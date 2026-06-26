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

impl TensionGraph {
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
        let used_chakras: Vec<Chakra> = self.arena.iter().filter_map(|n| n.chakra).collect();
        let mut min_channel_power = f32::INFINITY;
        for chakra in used_chakras {
            let channel_power = chakras[chakra].channel_power;
            if channel_power < min_channel_power {
                min_channel_power = channel_power;
            }
        }
        if min_channel_power <= 0.0 {
            return Err(CannotChannel);
        }
        Ok(self
            .cast_time()
            .max(self.energy_cost(env, tuning) / min_channel_power))
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
}
