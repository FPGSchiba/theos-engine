//! Cast-time energy-throughput floor (E14) — the execution-side layer over the
//! frozen settling delay.
//!
//! E13 ([`TensionGraph::cast_time`]) gives the geometry+filter settling delay,
//! frozen at assembly. E14 adds the floor that a caster cannot drive energy through
//! the weave faster than their conduit conducts it:
//!
//! ```text
//! energy_time         = E_cost / P
//! effective_cast_time = max( cast_time (E13 settling), energy_time )
//! ```
//!
//! `E_cost` is the execution energy cost (E9, [`TensionGraph::energy_cost`]); `P` is
//! the caster's **Channel Power** — the bottleneck `min` of [`ChakraState::channel_power`]
//! over the chakras the graph anchors through. This composes *over* the frozen graph
//! (the same authoring/execution seam as E9/E13); the `TensionGraph` is untouched.
//! See `plans/THEOS-173.md` and the Equations of the Weave (E14).
//!
//! [`ChakraState::channel_power`]: crate::ChakraState::channel_power

use super::TensionGraph;
use crate::{Chakra, Chakras, EnvResistance, WeaveTuning};
use thiserror::Error;

/// The caster cannot channel this spell: effective Channel Power `P ≤ 0` — an
/// untrained conduit on one of the anchoring chakras (E14, DP4). A lore beat (an
/// untrained caster simply cannot cast), surfaced as a typed error rather than an
/// infinite cast-time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("caster cannot channel this spell: channel power P ≤ 0 on an anchoring chakra")]
pub struct CannotChannel;

/// The outcome of an overdraw cast (E14 B2): the (shortened) effective cast-time and
/// every anchoring conduit that snapped.
#[derive(Debug, Clone, PartialEq)]
pub struct OverdrawOutcome {
    /// `max(cast_time, E_cost/(o·P))` under the chosen overdraw factor `o`.
    pub effective_cast_time: f32,
    /// Every anchoring chakra that snapped (E8 — each with `o·Aᵢ > Cᵢ`). A snap is a
    /// *local* event, so several can occur at once; each routes its own backlash
    /// downstream. Empty when every anchor stayed within Capacity.
    pub snap: Vec<Chakra>,
}

impl TensionGraph {
    fn min_channel_power(&self, chakras: &Chakras) -> f32 {
        let used_chakras: Vec<Chakra> = self.arena.iter().filter_map(|n| n.chakra).collect();
        let mut min_channel_power = f32::INFINITY;
        for chakra in used_chakras {
            let channel_power = chakras[chakra].channel_power;
            if channel_power < min_channel_power {
                min_channel_power = channel_power;
            }
        }
        min_channel_power
    }

    /// E14 — the effective cast-time: `max(cast_time, E_cost / P)`.
    ///
    /// Execution-side and caster-specific. `P` is the bottleneck `min`
    /// `channel_power` over the chakras this graph anchors through (its `Node`s'
    /// `chakra` values); `E_cost` is [`Self::energy_cost`] under `env`/`tuning`.
    /// Reads the frozen [`Self::cast_time`] but never mutates the graph — the same
    /// spell cast by two casters yields the same settling but a different effective
    /// time. `Err(CannotChannel)` when `P ≤ 0`.
    pub fn effective_cast_time(
        &self,
        chakras: &Chakras,
        env: EnvResistance,
        tuning: &WeaveTuning,
    ) -> Result<f32, CannotChannel> {
        let min_channel_power = self.min_channel_power(chakras);
        if min_channel_power <= 0.0 {
            return Err(CannotChannel);
        }
        Ok(self
            .cast_time()
            .max(self.energy_cost(env, tuning) / min_channel_power))
    }

    /// E14 B2 — overdraw cast-time: channel at `o·P` for a faster cast at snap risk.
    ///
    /// `overdraw` (`o`) is clamped to `≥ 1` (`o = 1` is the safe floor). The cast
    /// speeds to `max(cast_time, E_cost/(o·P))`, but the effective anchor amplitude
    /// rises to `o·Aᵢ`, snapping any conduit where `o·Aᵢ > Cᵢ` (E8). Returns the
    /// faster time plus every snapped anchor (empty if all stay within Capacity).
    /// `Err(CannotChannel)` when `P ≤ 0`. Frozen graph untouched — execution-side,
    /// like [`Self::effective_cast_time`].
    pub fn overdraw_cast_time(
        &self,
        chakras: &Chakras,
        env: EnvResistance,
        tuning: &WeaveTuning,
        overdraw: f32,
    ) -> Result<OverdrawOutcome, CannotChannel> {
        let o = overdraw.max(1.0);
        let min_channel_power = self.min_channel_power(chakras);
        if min_channel_power <= 0.0 {
            return Err(CannotChannel);
        }
        let energy_time = self.energy_cost(env, tuning) / (o * min_channel_power);
        let effective_cast_time = self.cast_time().max(energy_time);
        let snap: Vec<Chakra> = self
            .arena
            .iter()
            .filter_map(|n| n.chakra.map(|c| (c, n.output_amplitude)))
            .filter(|(c, a)| o * a > chakras[*c].capacity)
            .map(|(c, _)| c)
            .collect();
        Ok(OverdrawOutcome {
            effective_cast_time,
            snap,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CannotChannel;
    use crate::{
        Chakra, Chakras, CompilerContext, DEFAULT_INJECTION_ATTENUATION, EnvResistance,
        GraphAssembler, HarmonicToken, TensionGraph, TokenId, WeaveTuning, WovenToken,
    };
    use glam::Vec3;

    fn body(id: u32, chakra: Chakra, pos: Vec3, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::BodyAnchor {
                chakra,
                body_position: pos,
                frequency: 1.0,
                amplitude: 1.0,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    fn spatial(id: u32, pos: Vec3, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::SpatialAnchor {
                position: pos,
                frequency: 1.0,
                amplitude: 1.0,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    /// Root → leaf, unit edge, wave speed 1. `cast_time = d/c = 1.0`; under default
    /// env/tuning, `E_cost = e_base(1) + 1·1·W(1) = 2.0`. Anchored at Root only.
    fn two_node() -> TensionGraph {
        GraphAssembler::assemble(
            &[
                body(0, Chakra::Root, Vec3::ZERO, vec![1]),
                spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            ],
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .expect("test graph assembles")
    }

    /// A caster with a single trained conduit: Channel Power `p` on the Root chakra.
    fn caster_with_root_power(p: f32) -> Chakras {
        let mut chakras = Chakras::default();
        chakras.root_mut().channel_power = p;
        chakras
    }

    fn approx(got: f32, expected: f32) {
        assert!(
            (got - expected).abs() < 1e-5,
            "expected {expected}, got {got}"
        );
    }

    #[test]
    fn effective_cast_time_is_settling_when_propagation_bound() {
        // P = 10 ⇒ energy_time = 2/10 = 0.2 < cast_time(1.0) ⇒ settling wins.
        let g = two_node();
        let t = g
            .effective_cast_time(
                &caster_with_root_power(10.0),
                EnvResistance::default(),
                &WeaveTuning::default(),
            )
            .unwrap();
        approx(t, g.cast_time());
        approx(t, 1.0);
    }

    #[test]
    fn effective_cast_time_is_energy_time_when_throughput_bound() {
        // P = 0.5 ⇒ energy_time = 2/0.5 = 4.0 > cast_time(1.0) ⇒ throughput wins.
        let g = two_node();
        let t = g
            .effective_cast_time(
                &caster_with_root_power(0.5),
                EnvResistance::default(),
                &WeaveTuning::default(),
            )
            .unwrap();
        approx(t, 4.0);
        assert!(t > g.cast_time());
    }

    #[test]
    fn same_graph_two_casters_differ_only_in_effective_time() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = WeaveTuning::default();
        let fast = g
            .effective_cast_time(&caster_with_root_power(10.0), env, &tuning)
            .unwrap();
        let slow = g
            .effective_cast_time(&caster_with_root_power(0.5), env, &tuning)
            .unwrap();
        // Same frozen settling for both casters; only the throughput floor differs.
        assert!(slow > fast, "a weaker conduit casts slower");
        approx(g.cast_time(), 1.0);
    }

    #[test]
    fn frozen_cast_time_unchanged_by_floor() {
        let g = two_node();
        let before = g.cast_time();
        let _ = g
            .effective_cast_time(
                &caster_with_root_power(0.5),
                EnvResistance::default(),
                &WeaveTuning::default(),
            )
            .unwrap();
        // The execution floor never mutates the frozen graph.
        approx(g.cast_time(), before);
    }

    #[test]
    fn zero_channel_power_caster_cannot_cast() {
        // Default Chakras have channel_power 0 ⇒ P ≤ 0 ⇒ cannot channel.
        let g = two_node();
        let result = g.effective_cast_time(
            &Chakras::default(),
            EnvResistance::default(),
            &WeaveTuning::default(),
        );
        assert_eq!(result, Err(CannotChannel));
    }

    // --- THEOS-177: B2 overdraw (reuses the two_node / caster fixtures above) ---

    /// A caster with Channel Power `power` and Capacity `capacity` on the Root chakra.
    fn caster(power: f32, capacity: f32) -> Chakras {
        let mut chakras = Chakras::default();
        let root = chakras.root_mut();
        root.channel_power = power;
        root.capacity = capacity;
        chakras
    }

    #[test]
    fn overdraw_shortens_cast_vs_safe_floor() {
        // Safe (P=0.5): max(1, 2/0.5) = 4.0. Overdraw o=2: 2/(2·0.5) = 2.0.
        let g = two_node();
        let chakras = caster(0.5, 10.0); // capacity high ⇒ no snap
        let env = EnvResistance::default();
        let tuning = WeaveTuning::default();
        let safe = g.effective_cast_time(&chakras, env, &tuning).unwrap();
        let out = g.overdraw_cast_time(&chakras, env, &tuning, 2.0).unwrap();
        approx(out.effective_cast_time, 2.0);
        assert!(
            out.effective_cast_time < safe,
            "overdraw must be faster than safe"
        );
        assert!(out.snap.is_empty(), "high capacity ⇒ no snap");
    }

    #[test]
    fn safe_factor_one_matches_throughput_floor() {
        // o = 1 ⇒ identical timing to the safe THEOS-173 floor.
        let g = two_node();
        let chakras = caster(0.5, 10.0);
        let env = EnvResistance::default();
        let tuning = WeaveTuning::default();
        let safe = g.effective_cast_time(&chakras, env, &tuning).unwrap();
        let out = g.overdraw_cast_time(&chakras, env, &tuning, 1.0).unwrap();
        approx(out.effective_cast_time, safe);
        assert!(out.snap.is_empty());
    }

    #[test]
    fn overdraw_within_capacity_does_not_snap() {
        // o=2, Aᵢ=1 ⇒ o·Aᵢ=2 ≤ capacity 5 ⇒ no snap.
        let out = two_node()
            .overdraw_cast_time(
                &caster(1.0, 5.0),
                EnvResistance::default(),
                &WeaveTuning::default(),
                2.0,
            )
            .unwrap();
        assert!(out.snap.is_empty());
    }

    #[test]
    fn overdraw_past_capacity_snaps_the_anchor() {
        // o=2, Aᵢ=1 ⇒ o·Aᵢ=2 > capacity 1.5 ⇒ the Root conduit snaps.
        let out = two_node()
            .overdraw_cast_time(
                &caster(1.0, 1.5),
                EnvResistance::default(),
                &WeaveTuning::default(),
                2.0,
            )
            .unwrap();
        assert_eq!(out.snap, vec![Chakra::Root]);
    }

    #[test]
    fn overdraw_snaps_multiple_anchors() {
        // Two body anchors (Root + Sacral), both over capacity ⇒ both snap locally.
        let g = GraphAssembler::assemble(
            &[
                body(0, Chakra::Root, Vec3::ZERO, vec![1]),
                body(1, Chakra::Sacral, Vec3::new(1.0, 0.0, 0.0), vec![]),
            ],
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .expect("assembles");
        let mut chakras = Chakras::default();
        chakras.root_mut().channel_power = 1.0;
        chakras.root_mut().capacity = 1.5;
        chakras.sacral_mut().channel_power = 1.0;
        chakras.sacral_mut().capacity = 1.5;
        let out = g
            .overdraw_cast_time(
                &chakras,
                EnvResistance::default(),
                &WeaveTuning::default(),
                2.0,
            )
            .unwrap();
        // Both exceed Cᵢ; snap lists them in node-slot order (Root, then Sacral).
        assert_eq!(out.snap, vec![Chakra::Root, Chakra::Sacral]);
    }

    #[test]
    fn zero_channel_power_cannot_overdraw() {
        // Default Chakras: P = 0 ⇒ cannot channel, even under overdraw.
        let result = two_node().overdraw_cast_time(
            &Chakras::default(),
            EnvResistance::default(),
            &WeaveTuning::default(),
            2.0,
        );
        assert_eq!(result, Err(CannotChannel));
    }
}
