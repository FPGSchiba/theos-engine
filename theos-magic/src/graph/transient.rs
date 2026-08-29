//! E16c construction transient + per-chakra throughput meter — the time-evolving
//! execution layer over the frozen [`TensionGraph`].
//!
//! E16a/b ([`super::upkeep`]) give a maintained construct's **steady** per-chakra
//! occupation `u_∞,c = Ξ_p(D_c)·U`. E16c adds what happens *while manifesting* one:
//! the act spikes a chakra's occupation above that floor, and the spike relaxes
//! back exponentially.
//!
//! ```text
//! Δ_c     = Ξ_Δ(D_c) · ζ · E_cost         construction spike, injected once at spawn
//! Ξ_Δ(D)  = exp(−α_Δ · D)                  spike depth-attenuation (its own α_Δ)
//! du_c/dt = −(u_c − u_∞,c) / τ_p           exponential relaxation back to the floor
//! τ_p     = κ · E_cost                     recovery time constant
//! ```
//!
//! `ζ`/`α_Δ`/`κ` are [`WeaveTuning`] constants
//! ([`WeaveTuning::transient_spike_gain`], [`WeaveTuning::transient_spike_alpha`],
//! [`WeaveTuning::recovery_kappa`]); `E_cost` is E9
//! ([`TensionGraph::energy_cost`]); `U` and `u_∞,c` are E16a/b
//! ([`TensionGraph::upkeep`]); `D_c` is the E11 node depth. The Equations of the
//! Weave state the spike only as `Δ ∝ E_cost` — `ζ` and `α_Δ` are the constants
//! this engine pins (canon writeback pending). `α_Δ = 0` makes the spike flat
//! across every anchor; `α_Δ = α_p` makes it spread exactly like the floor.
//!
//! **Integration.** `du_c/dt = −(u_c − u_∞,c)/τ_p` is solved in closed form —
//! `(u_c − u_∞,c)` after `dt` is `(u_c − u_∞,c)·exp(−dt/τ_p)`. That step is exact
//! and unconditionally stable for any `dt ≥ 0`, unlike an explicit Euler step which
//! oscillates once `dt` approaches `2·τ_p` (and a cheap construct's `τ_p` is tiny).
//!
//! [`ThroughputMeter`] is the per-caster state that carries this: it sums floor +
//! live transient over every live construct and raises an **overdraw signal**
//! ([`ThroughputMeter::overdrawn`]) when `floor + transient > P_c` on any anchor.
//! Routing that signal into the existing E14/E8 capacity snap — and deciding when a
//! construct collapses or is dismissed — is the lifecycle story (THEOS-182); E16c
//! raises **no new failure mode**. The meter holds only scalars: never a
//! `TensionGraph`, an ECS entity, or a collapse verdict. The floor is snapshot at
//! [`ThroughputMeter::spawn`] under the then-current world; refreshing it as `ρ`/`ω`
//! drift on a held construct is THEOS-182.
//!
//! See `plans/THEOS-181.md` and the Equations of the Weave (E16c).

use crate::{Chakra, Chakras, EnvResistance, WeaveTuning, throughput_attenuation};
use std::collections::HashMap;

use super::TensionGraph;

/// E16c construction-transient magnitude `Δ = ζ · E_cost` — the spike a manifesting
/// construct adds to a chakra's occupation above the steady floor, **before** the
/// per-chakra depth attenuation `Ξ_Δ(D_c)`.
///
/// `ζ` is [`WeaveTuning::transient_spike_gain`]. Linear in `E_cost`, so a costlier
/// construct spikes harder — the same `E_cost` that lengthens its recovery
/// ([`recovery_tau`]).
pub fn transient_spike(e_cost: f32, tuning: &WeaveTuning) -> f32 {
    e_cost * tuning.transient_spike_gain
}

/// E16c recovery time constant `τ_p = κ · E_cost` — the e-folding time of the
/// construction transient's exponential relaxation to the floor.
///
/// `κ` is [`WeaveTuning::recovery_kappa`]. A cheaper construct has a shorter `τ_p`
/// and so recovers sooner.
pub fn recovery_tau(e_cost: f32, tuning: &WeaveTuning) -> f32 {
    e_cost * tuning.recovery_kappa
}

/// One analytic relaxation step of `du/dt = −(u − u_∞)/τ_p`: given the current
/// excess over the floor `excess = u − u_∞` and an elapsed `dt` seconds, returns the
/// excess afterwards, `excess · exp(−dt / τ_p)`.
///
/// The closed-form solution of the E16c ODE — exact for any `dt`, monotone toward
/// `0`, and never negative or oscillating (contrast an explicit Euler step).
pub fn relax_excess(excess: f32, tau_p: f32, dt: f32) -> f32 {
    excess * (-dt / tau_p).exp()
}

/// An opaque handle to one live maintained construct inside a [`ThroughputMeter`].
///
/// Returned by [`ThroughputMeter::spawn`] and consumed by
/// [`ThroughputMeter::release`]. Unique for the lifetime of a meter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstructId(u32);

/// One anchored chakra's contribution from a single live construct: the E16b steady
/// floor `u_c` and the current construction-transient excess `(u_c − u_∞,c)` that
/// [`ThroughputMeter::tick`] decays toward `0`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Lane {
    chakra: Chakra,
    floor: f32,
    transient: f32,
}

/// A live construct's full contribution to the meter, keyed by its [`ConstructId`].
#[derive(Debug, Clone, PartialEq)]
struct Construct {
    id: ConstructId,
    /// `τ_p = κ · E_cost` — this construct's transient decay time constant.
    tau_p: f32,
    /// One [`Lane`] per anchored chakra, in canonical [`Chakra::ALL`] order.
    lanes: Vec<Lane>,
}

/// Per-caster throughput meter (E16c): the standing floor `u_∞,c` plus the decaying
/// construction transient, summed over every live maintained construct, per chakra.
///
/// The genuinely time-evolving layer E16 adds over the frozen graph. Holds only
/// scalars keyed by [`ConstructId`] — never a [`TensionGraph`], an ECS entity, or a
/// collapse verdict (those are THEOS-182). Embed one per caster, as [`Chakras`] is
/// embedded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThroughputMeter {
    constructs: Vec<Construct>,
    next_id: u32,
}

impl ThroughputMeter {
    /// An empty meter — no live constructs, so no floor and no transient.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a newly manifested construct and return its handle.
    ///
    /// Raises the floor by the construct's E16b per-chakra occupation `Ξ_p(D_c)·U`
    /// ([`TensionGraph::upkeep`]) **and** injects the construction transient
    /// `Δ_c = Ξ_Δ(D_c) · ζ · E_cost` on top of it, with decay time constant
    /// `τ_p = κ · E_cost` ([`recovery_tau`]). `D_c` is the shallowest anchoring
    /// node's E11 depth on that chakra — the same rule [`Upkeep`] uses.
    ///
    /// `env` / `world_entropy` are the live world at spawn; the floor is snapshot
    /// here (see the module docs on `ρ`/`ω` drift). Reads the frozen `graph` once
    /// and keeps no reference to it.
    ///
    /// [`Upkeep`]: super::Upkeep
    pub fn spawn(
        &mut self,
        graph: &TensionGraph,
        env: EnvResistance,
        world_entropy: f32,
        tuning: &WeaveTuning,
    ) -> ConstructId {
        let construct_id: ConstructId = ConstructId(self.next_id);
        self.next_id += 1;
        let e_cost = graph.energy_cost(env, tuning);
        let tau_p = recovery_tau(e_cost, tuning);
        let upkeep_rate = graph.upkeep_rate(env, world_entropy, tuning);
        let spike = transient_spike(e_cost, tuning);
        let lanes: Vec<Lane> = Chakra::ALL
            .into_iter()
            .filter_map(|chakra| {
                let d_c = graph
                    .arena
                    .iter()
                    .filter(|node| node.chakra == Some(chakra))
                    .map(|node| node.depth)
                    .reduce(f32::min)?;

                Some(Lane {
                    chakra,
                    floor: throughput_attenuation(d_c, Some(tuning.throughput_alpha_p))
                        * upkeep_rate,
                    transient: throughput_attenuation(d_c, Some(tuning.transient_spike_alpha))
                        * spike,
                })
            })
            .collect();
        let construct = Construct {
            id: construct_id,
            tau_p,
            lanes,
        };
        self.constructs.push(construct);
        construct_id
    }

    /// Drop a construct — collapse or dismiss. Its floor contribution and its
    /// transient vanish **instantly** (E16c AC2). An unknown or already-released
    /// `id` is a no-op. Deciding *when* to call this — the collapse verdict — is the
    /// lifecycle story (THEOS-182).
    pub fn release(&mut self, id: ConstructId) {
        self.constructs.retain(|c| c.id != id);
    }

    /// Advance every live construct's transient by `dt` seconds:
    /// `(u_c − u_∞,c) *= exp(−dt / τ_p)` per construct ([`relax_excess`]). The floor
    /// is untouched. `dt = 0` and an empty meter are both no-ops.
    pub fn tick(&mut self, dt: f32) {
        for construct in self.constructs.iter_mut() {
            construct.lanes.iter_mut().for_each(|lane| {
                lane.transient = relax_excess(lane.transient, construct.tau_p, dt);
            })
        }
    }

    /// The steady floor `u_∞,c = Σ (over live constructs) Ξ_p(D_c)·U`, per anchored
    /// chakra, in canonical [`Chakra::ALL`] order. Empty when no construct is live.
    pub fn floor(&self) -> Vec<(Chakra, f32)> {
        let mut map: HashMap<Chakra, f32> = HashMap::new();
        for construct in self.constructs.iter() {
            construct.lanes.iter().for_each(|lane| {
                map.entry(lane.chakra)
                    .and_modify(|f| *f += lane.floor)
                    .or_insert(lane.floor);
            })
        }
        Chakra::ALL
            .into_iter()
            .filter_map(|chakra| map.get(&chakra).map(|total| (chakra, *total)))
            .collect()
    }

    /// The live construction transient `Σ (over live constructs) (u_c − u_∞,c)`, per
    /// anchored chakra, canonical order — the excess [`Self::tick`] decays toward
    /// `0`. Empty when no construct is live.
    pub fn transient(&self) -> Vec<(Chakra, f32)> {
        let mut map: HashMap<Chakra, f32> = HashMap::new();
        for construct in self.constructs.iter() {
            construct.lanes.iter().for_each(|lane| {
                map.entry(lane.chakra)
                    .and_modify(|f| *f += lane.transient)
                    .or_insert(lane.transient);
            })
        }
        Chakra::ALL
            .into_iter()
            .filter_map(|chakra| map.get(&chakra).map(|total| (chakra, *total)))
            .collect()
    }

    /// The meter reading: [`Self::floor`] + [`Self::transient`] per anchored chakra,
    /// canonical order. Empty when no construct is live.
    pub fn occupation(&self) -> Vec<(Chakra, f32)> {
        let mut map: HashMap<Chakra, f32> = HashMap::new();
        for construct in self.constructs.iter() {
            construct.lanes.iter().for_each(|lane| {
                map.entry(lane.chakra)
                    .and_modify(|f| *f += lane.floor + lane.transient)
                    .or_insert(lane.floor + lane.transient);
            })
        }
        Chakra::ALL
            .into_iter()
            .filter_map(|chakra| map.get(&chakra).map(|total| (chakra, *total)))
            .collect()
    }

    /// The E16c overdraw signal: every anchoring chakra whose live
    /// [`Self::occupation`] exceeds its Channel Power `P_c`
    /// ([`ChakraState::channel_power`]), in canonical [`Chakra::ALL`] order. Empty
    /// ⇒ every anchor is within `P_c`.
    ///
    /// The lifecycle layer (THEOS-182) routes these anchors into the **existing**
    /// E14/E8 capacity snap / backlash — E16c adds no new failure mode. Casting
    /// costly constructs in quick succession stacks transients past `P_c` and trips
    /// this; it clears again once the transients relax.
    ///
    /// [`ChakraState::channel_power`]: crate::ChakraState::channel_power
    pub fn overdrawn(&self, chakras: &Chakras) -> Vec<Chakra> {
        // `occupation()` is already canonical-order and anchored-only, so filtering
        // it against each anchor's Channel Power keeps that order.
        self.occupation()
            .into_iter()
            .filter(|&(chakra, occ)| occ > chakras[chakra].channel_power)
            .map(|(chakra, _)| chakra)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{ThroughputMeter, recovery_tau, relax_excess, transient_spike};
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

    /// Wave speed 1.0; integer edge lengths ⇒ coherence 1 on every edge ⇒ `ρ(G) = 1`
    /// on these fixtures (mirrors the `upkeep.rs` / `cast.rs` fixtures).
    fn assemble(tokens: &[WovenToken]) -> TensionGraph {
        GraphAssembler::assemble(
            tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .expect("test graph assembles")
    }

    /// Root → spatial leaf, unit edge. `ρ(G) = 1`, `W = 1`. Anchored at Root only.
    /// Under [`stable_tuning`]: `S = 2/3`, `U = 1.5`, `E_cost = 2.0`.
    fn two_node() -> TensionGraph {
        assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ])
    }

    /// `Θ₀ = 3` keeps `S = 1 − 1/3 = 2/3` on `ρ(G) = 1` graphs (defaults leave
    /// `λ = μ = ζ = κ = 1`, `α_p = α_Δ = 0.15`). Mirrors `upkeep.rs`.
    fn stable_tuning() -> WeaveTuning {
        WeaveTuning {
            theta_0: 3.0,
            ..WeaveTuning::default()
        }
    }

    fn approx(got: f32, expected: f32) {
        assert!(
            (got - expected).abs() < 1e-5,
            "expected {expected}, got {got}"
        );
    }

    /// The value recorded for `chakra` in a `(Chakra, f32)` lane list.
    fn lane(pairs: &[(Chakra, f32)], chakra: Chakra) -> f32 {
        pairs
            .iter()
            .find(|(c, _)| *c == chakra)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("no lane for {chakra:?}"))
    }

    // --- free fns: transient_spike / recovery_tau / relax_excess ---

    #[test]
    fn transient_spike_is_gain_times_energy_cost() {
        let tuning = WeaveTuning {
            transient_spike_gain: 3.0,
            ..WeaveTuning::default()
        };
        approx(transient_spike(4.0, &tuning), 12.0);
    }

    #[test]
    fn transient_spike_scales_linearly_with_energy_cost() {
        let tuning = WeaveTuning::default();
        approx(
            transient_spike(10.0, &tuning),
            5.0 * transient_spike(2.0, &tuning),
        );
    }

    #[test]
    fn recovery_tau_is_kappa_times_energy_cost() {
        let tuning = WeaveTuning {
            recovery_kappa: 2.5,
            ..WeaveTuning::default()
        };
        approx(recovery_tau(4.0, &tuning), 10.0);
    }

    #[test]
    fn recovery_tau_is_shorter_for_a_cheaper_construct() {
        // AC1: τ_p = κ·E_cost, so a lower E_cost is a shorter time constant.
        let tuning = WeaveTuning::default();
        assert!(recovery_tau(1.0, &tuning) < recovery_tau(9.0, &tuning));
    }

    #[test]
    fn relax_excess_is_analytic_exponential_decay() {
        // excess(dt) = excess₀ · exp(−dt / τ_p); at dt = τ_p that is excess₀ / e.
        approx(relax_excess(8.0, 2.0, 2.0), 8.0 * (-1.0_f32).exp());
    }

    #[test]
    fn relax_excess_is_stable_when_dt_far_exceeds_tau() {
        // dt ≫ τ_p: the closed form decays smoothly to ~0 and never goes negative
        // (an explicit Euler step would overshoot and oscillate here).
        let out = relax_excess(5.0, 0.01, 1_000.0);
        assert!((0.0..1e-6).contains(&out), "decays to ~0, got {out}");
    }

    #[test]
    fn relax_excess_with_zero_dt_is_identity() {
        approx(relax_excess(7.5, 3.0, 0.0), 7.5);
    }

    // --- spawn + floor / occupation: E16b consumption + E16c spike (AC2, D3) ---

    #[test]
    fn spawn_raises_the_floor_by_the_construct_occupation() {
        // AC2: an empty meter has no floor; after spawn each lane equals the
        // construct's E16b per-chakra occupation u_c.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        assert!(meter.floor().is_empty());

        meter.spawn(&g, env, 0.0, &tuning);
        for (chakra, u_c) in g.upkeep(env, 0.0, &tuning).per_chakra {
            approx(lane(&meter.floor(), chakra), u_c);
        }
    }

    #[test]
    fn spawn_floor_matches_upkeep_per_chakra() {
        // Multi-anchor: the meter's floor lanes are exactly THEOS-180's Upkeep
        // per-chakra list — same chakras, same values.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let up = g.upkeep(env, 0.0, &tuning);
        assert_eq!(meter.floor().len(), up.per_chakra.len());
        for (chakra, u_c) in up.per_chakra {
            approx(lane(&meter.floor(), chakra), u_c);
        }
    }

    #[test]
    fn spawn_injects_the_construction_transient_on_top_of_the_floor() {
        // Right after spawn (no tick), occupation is exactly floor + transient,
        // lane by lane.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let floor = meter.floor();
        let transient = meter.transient();
        for (chakra, occ) in meter.occupation() {
            approx(occ, lane(&floor, chakra) + lane(&transient, chakra));
        }
        assert!(
            lane(&transient, Chakra::Root) > 0.0,
            "a fresh construct carries a live spike"
        );
    }

    #[test]
    fn spawn_transient_at_the_root_is_gain_times_energy_cost() {
        // D3: the injection-root chakra sits at D_c = 0 ⇒ Ξ_Δ(0) = 1 ⇒ the full
        // ζ · E_cost spike, unattenuated.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let spike = lane(&meter.occupation(), Chakra::Root) - lane(&meter.floor(), Chakra::Root);
        approx(
            spike,
            tuning.transient_spike_gain * g.energy_cost(env, &tuning),
        );
    }

    #[test]
    fn spawn_transient_attenuates_from_the_injection_root() {
        // D3: Δ_c = exp(−α_Δ·D_c)·ζ·E_cost, so a deeper anchor bears strictly less
        // of the construction spike than the root.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let spike = |c| lane(&meter.occupation(), c) - lane(&meter.floor(), c);
        assert!(
            spike(Chakra::Sacral) < spike(Chakra::Root),
            "deeper anchor bears less spike: {} !< {}",
            spike(Chakra::Sacral),
            spike(Chakra::Root)
        );
    }

    #[test]
    fn spawn_transient_is_flat_across_anchors_when_alpha_delta_is_zero() {
        // D3: α_Δ = 0 ⇒ Ξ_Δ(D) = exp(0) = 1 on every anchor ⇒ every lane gets the
        // same ζ · E_cost spike regardless of depth.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = WeaveTuning {
            transient_spike_alpha: 0.0,
            ..stable_tuning()
        };
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let expected = tuning.transient_spike_gain * g.energy_cost(env, &tuning);
        let spike = |c| lane(&meter.occupation(), c) - lane(&meter.floor(), c);
        approx(spike(Chakra::Root), expected);
        approx(spike(Chakra::Sacral), expected);
    }

    #[test]
    fn two_constructs_stack_their_floors() {
        // AC2: u_∞,c is the sum over live constructs — two of the same construct
        // double the root floor.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let u_root = g.upkeep(env, 0.0, &tuning).total; // root u_c == U

        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);
        meter.spawn(&g, env, 0.0, &tuning);
        approx(lane(&meter.floor(), Chakra::Root), 2.0 * u_root);
    }

    // --- release: collapse / dismiss drops the contribution instantly (AC2) ---

    #[test]
    fn release_drops_the_floor_instantly() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        let id = meter.spawn(&g, env, 0.0, &tuning);
        assert!(!meter.floor().is_empty());

        meter.release(id);
        assert!(meter.floor().is_empty(), "no live constructs ⇒ no floor");
    }

    #[test]
    fn release_drops_the_transient_instantly() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        let id = meter.spawn(&g, env, 0.0, &tuning);
        assert!(!meter.transient().is_empty());

        meter.release(id);
        assert!(meter.transient().is_empty());
    }

    #[test]
    fn release_one_of_two_constructs_leaves_the_other_intact() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let u_root = g.upkeep(env, 0.0, &tuning).total;

        let mut meter = ThroughputMeter::new();
        let first = meter.spawn(&g, env, 0.0, &tuning);
        meter.spawn(&g, env, 0.0, &tuning);
        meter.release(first);

        approx(lane(&meter.floor(), Chakra::Root), u_root);
    }

    #[test]
    fn release_of_an_unknown_id_is_a_noop() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        let id = meter.spawn(&g, env, 0.0, &tuning);

        meter.release(id);
        meter.release(id); // already gone — must not panic or resurrect a floor
        assert!(meter.floor().is_empty());
    }

    // --- tick: the E16c relaxation ODE (AC1) ---

    #[test]
    fn tick_relaxes_the_occupation_toward_the_floor() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let floor = lane(&meter.floor(), Chakra::Root);
        let start = lane(&meter.occupation(), Chakra::Root);
        assert!(start > floor);

        meter.tick(1.0);
        let mid = lane(&meter.occupation(), Chakra::Root);
        assert!(
            mid < start && mid > floor,
            "monotone approach, not yet arrived"
        );

        meter.tick(1_000.0);
        approx(lane(&meter.occupation(), Chakra::Root), floor);
    }

    #[test]
    fn tick_matches_a_sampled_manual_integration_within_1e_5() {
        // AC1 headline: the analytic tick must track a fine-grained numeric
        // integration of du_c/dt = −(u_c − u_∞,c)/τ_p to within 1e-5.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let floor = lane(&meter.floor(), Chakra::Root);
        let e_cost = g.energy_cost(env, &tuning);
        let tau_p = tuning.recovery_kappa * e_cost;
        let spike0 = tuning.transient_spike_gain * e_cost; // Ξ_Δ(0) = 1 at the root

        // Reference: explicit Euler at a tiny step, integrated to t = 1.0. Done in
        // f64 so the reference itself is far tighter than the 1e-5 gate — an f32
        // accumulation over this many steps drifts by ~1e-4 on rounding alone.
        let total_t = 1.0_f64;
        let steps = 200_000_u32;
        let h = total_t / f64::from(steps);
        let tau_p = f64::from(tau_p);
        let mut excess = f64::from(spike0);
        for _ in 0..steps {
            excess += -(excess / tau_p) * h;
        }
        let reference = f64::from(floor) + excess;

        meter.tick(total_t as f32);
        let got = f64::from(lane(&meter.occupation(), Chakra::Root));
        assert!(
            (got - reference).abs() < 1e-5,
            "analytic {got} vs sampled integration {reference}"
        );
    }

    #[test]
    fn tick_a_cheaper_construct_recovers_sooner() {
        // AC1: τ_p = κ·E_cost, so after the same elapsed time the cheaper
        // construct's transient has decayed to a smaller fraction of its own spike.
        let cheap = two_node(); // W = 1 ⇒ E_cost = 2
        let dear = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(5.0, 0.0, 0.0), vec![]),
        ]); // W = 5 ⇒ E_cost = 6
        let env = EnvResistance::default();
        let tuning = stable_tuning();

        let mut a = ThroughputMeter::new();
        a.spawn(&cheap, env, 0.0, &tuning);
        let mut b = ThroughputMeter::new();
        b.spawn(&dear, env, 0.0, &tuning);

        a.tick(1.0);
        b.tick(1.0);

        let fraction = |m: &ThroughputMeter, graph: &TensionGraph| {
            let spike0 = tuning.transient_spike_gain * graph.energy_cost(env, &tuning);
            lane(&m.transient(), Chakra::Root) / spike0
        };
        assert!(
            fraction(&a, &cheap) < fraction(&b, &dear),
            "cheaper construct recovers sooner: {} !< {}",
            fraction(&a, &cheap),
            fraction(&b, &dear)
        );
    }

    #[test]
    fn tick_with_no_live_constructs_is_a_noop() {
        let mut meter = ThroughputMeter::new();
        meter.tick(1.0);
        assert!(meter.floor().is_empty());
        assert!(meter.occupation().is_empty());
    }

    #[test]
    fn tick_never_overshoots_the_floor() {
        // The analytic decay is monotone: however large the step, the transient
        // stays ≥ 0 (an explicit Euler step overshoots once dt > 2·τ_p).
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        meter.tick(10_000.0); // dt ≫ τ_p
        let transient = lane(&meter.transient(), Chakra::Root);
        assert!(transient >= 0.0, "transient went negative: {transient}");
        approx(transient, 0.0);
    }

    // --- overdrawn: reuse of the E14/E8 cliff (AC3) ---

    #[test]
    fn overdrawn_is_empty_when_every_anchor_is_within_channel_power() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let mut chakras = Chakras::default();
        for (_, state) in chakras.iter_mut() {
            state.channel_power = 1.0e6;
        }
        assert!(meter.overdrawn(&chakras).is_empty());
    }

    #[test]
    fn overdrawn_flags_an_anchor_when_floor_plus_transient_exceeds_channel_power() {
        // AC3: P_c set below the live occupation ⇒ that anchor is overdrawn.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let occ_root = lane(&meter.occupation(), Chakra::Root);
        let mut chakras = Chakras::default();
        chakras.root_mut().channel_power = occ_root * 0.5;
        assert_eq!(meter.overdrawn(&chakras), vec![Chakra::Root]);
    }

    #[test]
    fn overdrawn_trips_on_a_succession_of_costly_constructs() {
        // AC3 headline: each construct stacks another Ξ_p(0)·U on the floor and
        // another ζ·E_cost transient on the root; enough of them with no tick
        // between pushes the root past P_c.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(3.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = stable_tuning();

        // One construct's root occupation = U + ζ·E_cost. Seat P_c above one but
        // below two.
        let one = g.upkeep(env, 0.0, &tuning).total
            + tuning.transient_spike_gain * g.energy_cost(env, &tuning);
        let mut chakras = Chakras::default();
        chakras.root_mut().channel_power = one * 1.5;

        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);
        assert!(
            meter.overdrawn(&chakras).is_empty(),
            "one costly construct stays within P_c"
        );

        meter.spawn(&g, env, 0.0, &tuning);
        assert_eq!(
            meter.overdrawn(&chakras),
            vec![Chakra::Root],
            "the second trips the overdraw signal"
        );
    }

    #[test]
    fn overdrawn_clears_after_the_transient_decays() {
        // AC3: overdrawn at spawn while the spike is fresh; once it relaxes, if the
        // steady floor alone sits under P_c, the signal clears.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(3.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = stable_tuning();

        let floor = g.upkeep(env, 0.0, &tuning).total;
        let spike = tuning.transient_spike_gain * g.energy_cost(env, &tuning);
        let mut chakras = Chakras::default();
        chakras.root_mut().channel_power = floor + spike * 0.25; // between floor and floor+spike

        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);
        assert_eq!(meter.overdrawn(&chakras), vec![Chakra::Root]);

        meter.tick(10.0 * tuning.recovery_kappa * g.energy_cost(env, &tuning));
        assert!(
            meter.overdrawn(&chakras).is_empty(),
            "transient decayed below P_c"
        );
    }

    #[test]
    fn overdrawn_reports_anchors_in_canonical_order() {
        // Two anchors, P_c = 0 everywhere ⇒ both overdrawn ⇒ Root before Sacral.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        assert_eq!(
            meter.overdrawn(&Chakras::default()),
            vec![Chakra::Root, Chakra::Sacral]
        );
    }

    #[test]
    fn overdrawn_root_strains_before_deeper_anchors() {
        // D3 consistency: root bears the most (Ξ_p(0) = Ξ_Δ(0) = 1), so with P_c
        // seated between the two anchors' occupations only the root trips.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();
        meter.spawn(&g, env, 0.0, &tuning);

        let root_occ = lane(&meter.occupation(), Chakra::Root);
        let sacral_occ = lane(&meter.occupation(), Chakra::Sacral);
        assert!(root_occ > sacral_occ);

        let mut chakras = Chakras::default();
        for (_, state) in chakras.iter_mut() {
            state.channel_power = (root_occ + sacral_occ) / 2.0;
        }
        assert_eq!(meter.overdrawn(&chakras), vec![Chakra::Root]);
    }

    // --- ConstructId ---

    #[test]
    fn construct_ids_are_unique_across_spawns() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let mut meter = ThroughputMeter::new();

        let a = meter.spawn(&g, env, 0.0, &tuning);
        let b = meter.spawn(&g, env, 0.0, &tuning);
        let c = meter.spawn(&g, env, 0.0, &tuning);
        assert!(a != b && b != c && a != c);
    }
}
