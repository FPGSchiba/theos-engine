//! Resonance stability (E3/E5/E6/E7) — the M3 spectral-radius solver over an
//! assembled [`TensionGraph`].
//!
//! The intrinsic loop gain `ρ(G)` (E6) is frozen at assembly — geometry, stiffness,
//! and coherence are all compile-time. The world-dependent stability scalar `S(ω)`
//! and the Stable/Volatile classification (E7) are computed at execution, the same
//! authoring/execution seam THEOS-159 used for energy. See `plans/THEOS-150.md` and
//! the Equations of the Weave (E3, E5, E6, E7).

use super::TensionGraph;
use crate::WeaveTuning;

const TOL: f32 = 1e-6;

/// E7 classification: a cast is `Stable` when `S ≥ S_min`, else `Volatile`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Stability {
    /// `S ≥ S_min` — a calm, well-damped graph.
    Stable,
    /// `S < S_min` — loop gain past the safe margin; runs away (E15/backlash).
    Volatile,
}

/// E3 — structural tension stored in one edge: `Tᵢⱼ = ½·kᵢⱼ·(dᵢⱼ − Lᵢⱼ)²`.
pub fn edge_tension(stiffness: f32, distance: f32, rest_length: f32) -> f32 {
    0.5 * stiffness * (distance - rest_length).powi(2)
}

/// E5 — edge coherence: `Iᵢⱼ = cos(Δφᵢⱼ)·exp(−(Δfᵢⱼ/σ_f)²)`, in `[−1, 1]`.
pub fn edge_coherence(delta_phi: f32, delta_freq: f32, sigma_f: f32) -> f32 {
    delta_phi.cos() * (-(delta_freq / sigma_f).powi(2)).exp()
}

fn norm(y: &[f32]) -> f32 {
    y.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn div(y: &[f32], d: f32) -> Vec<f32> {
    y.iter().map(|v| v / d).collect()
}

/// E6 — spectral radius `ρ(G) = |λ_max(G)|` of the coupling matrix
/// `Gᵢⱼ = (kᵢⱼ/k_ref)·Iᵢⱼ`, via sparse power iteration.
///
/// `G` is symmetric but the CSR is oriented (+ back-edges), so the matvec scatters
/// each edge's contribution to *both* endpoints (D5). `O(E)` per iteration, hard
/// cap 64 iterations, early-exit on convergence. Computed once at assembly and
/// frozen into [`TensionGraph::rho_g`].
pub fn spectral_radius(graph: &TensionGraph, tuning: &WeaveTuning) -> f32 {
    if graph.arena.is_empty() {
        return 0.0;
    }

    let n = graph.arena.len();
    let mut x = vec![1.0; n];
    let mut prev = 0.0;

    for _ in 0..64 {
        let mut y = vec![0.0; n];
        for (id, node) in graph.arena.iter_ids() {
            let i = id.index() as usize;
            for edge in graph.neighbors(id) {
                let j = edge.target.index() as usize;
                let target = graph
                    .arena
                    .get(edge.target)
                    .expect("CSR target node must exist");
                let w = (edge.stiffness / tuning.k_ref)
                    * edge_coherence(
                        node.phase - target.phase,
                        node.frequency - target.frequency,
                        edge.sigma_f,
                    );
                y[i] += w * x[j];
                y[j] += w * x[i];
            }
        }

        let norm = norm(&y);
        if norm < f32::EPSILON {
            return 0.0;
        }
        if (norm - prev).abs() < TOL {
            return norm;
        }
        x = div(&y, norm);
        prev = norm;
    }
    prev
}

impl TensionGraph {
    /// E7 — resonance stability for this cast: `S = 1 − ρ(G)/Θ_env` with
    /// `Θ_env = Θ₀·(1 − ω)`, plus the Stable/Volatile classification at `S_min`.
    ///
    /// Execution-side (`ω = world_entropy` is world state); reads the frozen
    /// [`Self::rho_g`]. Returns `(S, classification)`.
    pub fn stability(&self, world_entropy: f32, tuning: &WeaveTuning) -> (f32, Stability) {
        let theta_env = tuning.theta_0 * (1.0 - world_entropy);
        let s = 1.0 - self.rho_g / theta_env;
        let stability = if s >= tuning.s_min {
            Stability::Stable
        } else {
            Stability::Volatile
        };
        (s, stability)
    }
}

#[cfg(test)]
mod tests {
    use super::{Stability, edge_coherence, edge_tension};
    use crate::{
        Chakra, CompilerContext, DEFAULT_INJECTION_ATTENUATION, GraphAssembler, HarmonicToken,
        TensionGraph, TokenId, WeaveTuning, WovenToken,
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

    fn assemble(tokens: &[WovenToken]) -> TensionGraph {
        // Wave speed 1.0 so phase = 2π·d; with unit frequency, integer edge lengths
        // give Δφ = 2π·(integer) ⇒ cos = 1 ⇒ coherence I = 1 on every edge.
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
            (got - expected).abs() < 1e-4,
            "expected {expected}, got {got}"
        );
    }

    /// Root → leaf, unit edge. One edge ⇒ `G = [[0, w], [w, 0]]` with `w = 1`
    /// (k/k_ref = 1, coherence = 1), whose eigenvalues are ±1 ⇒ `ρ(G) = 1`.
    fn two_node_graph() -> TensionGraph {
        assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ])
    }

    /// A 3-4-5 right triangle (all edge lengths integer ⇒ every coherence = 1), so
    /// `G` is the complete graph `K₃` with unit weights ⇒ `ρ(G) = 2`. The 1-2 link
    /// closes the cycle (a back-edge), exercising loop gain.
    fn triangle_graph() -> TensionGraph {
        assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(3.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(0.0, 4.0, 0.0), vec![]),
        ])
    }

    #[test]
    fn edge_tension_zero_when_length_equals_rest() {
        // d = L ⇒ T = ½·k·0² = 0.
        approx(edge_tension(2.0, 5.0, 5.0), 0.0);
    }

    #[test]
    fn edge_tension_nonzero_when_stretched() {
        // ½·2·(5 − 3)² = 4.
        approx(edge_tension(2.0, 5.0, 3.0), 4.0);
    }

    #[test]
    fn coherence_high_for_in_phase_frequency_matched_edge() {
        // Δφ = 0, Δf = 0 ⇒ cos(0)·exp(0) = 1.
        approx(edge_coherence(0.0, 0.0, 1.0), 1.0);
    }

    #[test]
    fn coherence_low_for_divergent_frequencies_via_sigma_f() {
        let matched = edge_coherence(0.0, 0.0, 1.0);
        let divergent = edge_coherence(0.0, 10.0, 1.0);
        // The Gaussian crushes a far-off frequency toward zero, well below a match.
        assert!(divergent < matched, "divergent must be weaker than matched");
        assert!(divergent.abs() < 1e-3, "10σ off ⇒ ~0, got {divergent}");
    }

    #[test]
    fn spectral_radius_matches_dense_reference_on_small_graph() {
        // Dense reference: G = [[0,1],[1,0]] ⇒ λ = ±1 ⇒ ρ = 1.
        approx(two_node_graph().rho_g(), 1.0);
    }

    #[test]
    fn power_iteration_terminates_within_64_iters() {
        // A cyclic graph (where the iteration must actually converge, not trivially)
        // still returns a finite, correct ρ ⇒ it terminated. K₃ ⇒ ρ = 2.
        let rho = triangle_graph().rho_g();
        assert!(rho.is_finite(), "ρ(G) must be finite");
        approx(rho, 2.0);
    }

    #[test]
    fn constructive_graph_is_stable() {
        // Acyclic, fully constructive ⇒ ρ = 1. With Θ₀ = 3, ω = 0: Θ_env = 3,
        // S = 1 − 1/3 ≈ 0.667 ≥ S_min(0.5) ⇒ Stable.
        let tuning = WeaveTuning {
            theta_0: 3.0,
            ..WeaveTuning::default()
        };
        let (s, classification) = two_node_graph().stability(0.0, &tuning);
        approx(s, 1.0 - 1.0 / 3.0);
        assert_eq!(classification, Stability::Stable);
    }

    #[test]
    fn cyclic_feedback_runaway_is_volatile() {
        // The K₃ cycle's loop gain (ρ = 2) under the same ceiling: S = 1 − 2/3 ≈
        // 0.333 < S_min(0.5) ⇒ Volatile.
        let tuning = WeaveTuning {
            theta_0: 3.0,
            ..WeaveTuning::default()
        };
        let (s, classification) = triangle_graph().stability(0.0, &tuning);
        approx(s, 1.0 - 2.0 / 3.0);
        assert_eq!(classification, Stability::Volatile);
    }
}
