//! E9 energy cost and E10 efficiency η — a read pass over an assembled
//! [`TensionGraph`].
//!
//! The intrinsic, world-independent part of the cost (`W = Σ dᵢⱼ·Aᵢ`) is frozen at
//! assembly ([`TensionGraph::authoring_work`]); this module applies the
//! execution-side environmental factors (`ρ_med`, `ρ_env`) and the base floor
//! `E_base` to produce E9, then divides effector output by it for E10. No new
//! physics — see `plans/THEOS-159.md` and the Equations of the Weave (E9, E10).

use super::{NodeId, TensionGraph};
use crate::WeaveTuning;

/// Execution-time environmental resistance factors for E9 (`ρ_med`, `ρ_env`).
///
/// World/medium state at cast time, distinct from the frozen graph. `Default` is a
/// resistance-free cast (`1.0`/`1.0`), so `energy_cost == E_base + W`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EnvResistance {
    /// Medium resistance `ρ_med`.
    pub medium: f32,
    /// Environmental resistance `ρ_env`.
    pub environment: f32,
}

impl Default for EnvResistance {
    fn default() -> Self {
        Self {
            medium: 1.0,
            environment: 1.0,
        }
    }
}

impl TensionGraph {
    /// E9 per-node energy contribution — the cost attributable to `id` as an
    /// emitter, under the given environment. A diagnostic read; the graph total is
    /// [`Self::energy_cost`]. `None` if the handle is stale.
    pub fn node_energy(&self, id: NodeId, env: EnvResistance) -> Option<f32> {
        self.arena
            .get(id)
            .map(|this| env.environment * env.medium * this.node_work)
    }

    /// E9 total energy cost `E_cost = E_base + ρ_med·ρ_env·W` for this cast.
    ///
    /// `W` is the frozen [`Self::authoring_work`]; `E_base` and the `ρ` factors are
    /// applied here at execution. `E_base > 0` keeps the cost strictly positive.
    pub fn energy_cost(&self, env: EnvResistance, weave_tuning: &WeaveTuning) -> f32 {
        weave_tuning.e_base + env.medium * env.environment * self.authoring_work
    }

    /// E10 construct efficiency `η = (Σ effector output amplitudes) / E_cost`.
    ///
    /// Numerator sums [`NodeKind::Effector`] nodes' output amplitudes; denominator
    /// is [`Self::energy_cost`]. Always defined because `E_cost > 0`.
    pub fn efficiency(&self, env: EnvResistance, weave_tuning: &WeaveTuning) -> f32 {
        let total_output: f32 = self
            .arena
            .iter()
            .filter_map(|node| {
                if node.kind == super::NodeKind::Effector {
                    Some(node.output_amplitude)
                } else {
                    None
                }
            })
            .sum();
        total_output / self.energy_cost(env, weave_tuning)
    }
}

#[cfg(test)]
mod tests {
    use super::EnvResistance;
    use crate::{
        Chakra, CompilerContext, DEFAULT_INJECTION_ATTENUATION, GraphAssembler, HarmonicToken,
        TensionGraph, TokenId, WeaveTuning, WovenToken,
    };
    use glam::Vec3;

    fn body(id: u32, chakra: Chakra, pos: Vec3, amplitude: f32, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::BodyAnchor {
                chakra,
                body_position: pos,
                frequency: 1.0,
                amplitude,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    fn spatial(id: u32, pos: Vec3, amplitude: f32, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::SpatialAnchor {
                position: pos,
                frequency: 1.0,
                amplitude,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    fn assemble(tokens: &[WovenToken]) -> TensionGraph {
        GraphAssembler::assemble(
            tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .expect("test graph assembles")
    }

    fn approx(got: f32, expected: f32) {
        assert!(
            (got - expected).abs() < 1e-5,
            "expected {expected}, got {got}"
        );
    }

    /// A root feeding two leaves at distances 1 and 2. Every edge's emitter is the
    /// root (output amplitude 1.0), so `W = 1·1 + 2·1 = 3` with no downstream
    /// phasor sums to reason about; each leaf is a single-fed effector ⇒ output 1.0.
    fn star() -> TensionGraph {
        assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, 1.0, vec![1, 2]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), 1.0, vec![]),
            spatial(2, Vec3::new(0.0, 2.0, 0.0), 1.0, vec![]),
        ])
    }

    #[test]
    fn graph_energy_matches_manual_sum_over_edges() {
        let g = star();
        let env = EnvResistance::default(); // ρ_med = ρ_env = 1.0
        let tuning = WeaveTuning::default(); // e_base = 1.0
        // W = Σ dᵢⱼ·Aᵢ = 1·1 + 2·1 = 3; E_cost = e_base + ρ·ρ·W = 1 + 3 = 4.
        approx(g.energy_cost(env, &tuning), 4.0);
    }

    #[test]
    fn node_energy_scales_with_edge_length_and_amplitude() {
        let env = EnvResistance::default();
        // Baseline: one unit edge, unit source amplitude.
        let base = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, 1.0, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), 1.0, vec![]),
        ]);
        // Longer edge (length 3), same amplitude.
        let longer = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, 1.0, vec![1]),
            spatial(1, Vec3::new(3.0, 0.0, 0.0), 1.0, vec![]),
        ]);
        // Same edge, louder source (amplitude 2).
        let louder = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, 2.0, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), 1.0, vec![]),
        ]);

        let e_base = base.node_energy(base.root(), env).unwrap();
        let e_longer = longer.node_energy(longer.root(), env).unwrap();
        let e_louder = louder.node_energy(louder.root(), env).unwrap();

        assert!(e_longer > e_base, "a longer edge must cost more energy");
        assert!(e_louder > e_base, "a louder source must cost more energy");
        // node_energy = ρ·ρ·(Σ dᵢⱼ·Aᵢ over edges the node emits).
        approx(e_base, 1.0); // 1·1
        approx(e_longer, 3.0); // 3·1
        approx(e_louder, 2.0); // 1·2
    }

    #[test]
    fn efficiency_matches_manual_eta_on_constructive_graph() {
        let g = star();
        let env = EnvResistance::default();
        let tuning = WeaveTuning::default();
        // Each leaf is fed by the root alone ⇒ output amplitude 1.0.
        let leaf = g.neighbors(g.root())[0].target;
        approx(g.node(leaf).unwrap().output_amplitude, 1.0);
        // η = Σ effector output / E_cost = (1 + 1) / 4 = 0.5.
        approx(g.efficiency(env, &tuning), 0.5);
    }

    #[test]
    fn efficiency_on_zero_energy_graph_is_well_defined() {
        // A lone node has no edges ⇒ W = 0, so E_cost = e_base exactly and η stays
        // defined (the floor is what keeps the denominator off zero).
        let g = assemble(&[body(0, Chakra::Root, Vec3::ZERO, 1.0, vec![])]);
        let env = EnvResistance::default();
        let tuning = WeaveTuning::default();
        approx(g.energy_cost(env, &tuning), tuning.e_base);
        let eta = g.efficiency(env, &tuning);
        assert!(eta.is_finite(), "η must be defined when W = 0");
        // The lone node is its own effector: 1.0 / e_base(1.0) = 1.0.
        approx(eta, 1.0);
    }

    #[test]
    fn full_transfer_graph_efficiency_near_one() {
        // One unit edge, unit amplitude, negligible base floor: the cost is almost
        // entirely the single transfer (ρ·ρ·d·A = 1), which equals the effector
        // output (1.0), so η → 1.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, 1.0, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), 1.0, vec![]),
        ]);
        let env = EnvResistance::default();
        let tuning = WeaveTuning {
            e_base: 1.0e-4,
            ..WeaveTuning::default()
        };
        let eta = g.efficiency(env, &tuning);
        assert!((eta - 1.0).abs() < 1.0e-3, "expected η ≈ 1, got {eta}");
    }
}
