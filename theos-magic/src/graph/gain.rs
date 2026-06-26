//! Resonance gain (E15) — the power a graph's output earns from its stability margin.
//!
//! `Q = 1/Sᵞ` (`= Θ_env/(Θ_env − ρ(G))` at γ=1) multiplies output amplitude: a
//! well-damped graph (`S → 1`) gains `Q ≈ 1`, an under-damped one (`S → S_min`) rings
//! up a large — but finite — `Q`, and at `S ≤ 0` the gain diverges (the canon
//! Runaway / detonation). Execution-side, like [`TensionGraph::stability`]: `S`
//! depends on `ω = world_entropy`, so the same spell yields a different `Q` in a
//! harsher environment. The frozen graph is untouched. See `plans/THEOS-176.md` and
//! the Equations of the Weave (E15).

use super::TensionGraph;
use crate::WeaveTuning;
use thiserror::Error;

/// The resonance gain diverges: stability `S ≤ 0` (`ρ(G) ≥ Θ_env`), so `Q = 1/Sᵞ`
/// is unbounded — the cast has crossed into the canon **Runaway** / detonation
/// regime (E7/E15). Surfaced as a typed error rather than a non-finite `Q`; the
/// `Stability::Volatile` verdict (E7) is the gameplay failure signal. **Not** a new
/// failure mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("resonance gain runaway: stability S ≤ 0 (ρ(G) ≥ Θ_env) — the cast detonates")]
pub struct ResonanceRunaway;

impl TensionGraph {
    /// E15 — resonance gain `Q = 1/Sᵞ` for this cast.
    ///
    /// Reads `S` from [`Self::stability`] (so `ω = world_entropy` is execution
    /// state) and `γ` from `tuning`. Graph-wide (`S` is one value per graph);
    /// callers multiply output amplitude by `Q` at the point of effect. Finite and
    /// `≥ 1` for `0 < S ≤ 1` (including the volatile band — the power payoff);
    /// `Err(ResonanceRunaway)` once `S ≤ 0`.
    pub fn resonance_gain(
        &self,
        world_entropy: f32,
        tuning: &WeaveTuning,
    ) -> Result<f32, ResonanceRunaway> {
        let stability = self.stability(world_entropy, tuning).0;
        if stability <= 0.0 {
            Err(ResonanceRunaway)
        } else {
            Ok(stability.powf(-tuning.gamma))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ResonanceRunaway;
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

    /// Root → leaf, unit edge ⇒ frozen `ρ(G) = 1` (see THEOS-150 tests). `S` is then
    /// steered purely by the tuning's `Θ₀`/`ω` passed to `resonance_gain`.
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

    /// Default tuning (`γ = 2`, `S_min = 0.5`) with an explicit stability ceiling `Θ₀`.
    fn tuning_with_theta0(theta_0: f32) -> WeaveTuning {
        WeaveTuning {
            theta_0,
            ..WeaveTuning::default()
        }
    }

    #[test]
    fn gain_near_one_for_well_damped_graph() {
        // Θ₀ = 100, ω = 0 ⇒ S = 1 − 1/100 = 0.99 ⇒ Q = 0.99⁻² ≈ 1.02.
        let q = two_node()
            .resonance_gain(0.0, &tuning_with_theta0(100.0))
            .unwrap();
        assert!((q - 1.0).abs() < 0.05, "well-damped ⇒ Q ≈ 1, got {q}");
    }

    #[test]
    fn gain_matches_manual_q_for_known_s_and_gamma() {
        // Θ₀ = 2, ω = 0 ⇒ S = 1 − 1/2 = 0.5; γ = 2 ⇒ Q = 0.5⁻² = 4.0.
        let q = two_node()
            .resonance_gain(0.0, &tuning_with_theta0(2.0))
            .unwrap();
        assert!((q - 4.0).abs() < 1e-3, "expected Q ≈ 4.0, got {q}");
    }

    #[test]
    fn gain_rises_as_stability_margin_thins() {
        let g = two_node();
        // Wide margin: Θ₀ = 10 ⇒ S = 0.9 ⇒ Q ≈ 1.23.
        let wide = g.resonance_gain(0.0, &tuning_with_theta0(10.0)).unwrap();
        // Thin (volatile) margin: Θ₀ = 1.5 ⇒ S = 0.333 (< S_min) ⇒ Q = 9.0, finite.
        let thin = g.resonance_gain(0.0, &tuning_with_theta0(1.5)).unwrap();
        assert!(thin > wide, "thinner margin must give more gain");
        assert!(thin.is_finite(), "volatile band still yields a finite Q");
    }

    #[test]
    fn gain_differs_with_environment_on_same_graph() {
        let g = two_node();
        let tuning = tuning_with_theta0(4.0);
        // ω = 0   ⇒ Θ_env = 4, S = 0.75, Q ≈ 1.78.
        let calm = g.resonance_gain(0.0, &tuning).unwrap();
        // ω = 0.5 ⇒ Θ_env = 2, S = 0.5,  Q = 4.0 — harsher world, more gain.
        let harsh = g.resonance_gain(0.5, &tuning).unwrap();
        assert!(harsh > calm, "higher entropy thins the margin ⇒ more gain");
        // The frozen graph is untouched by either read.
        approx(g.rho_g(), 1.0);
    }

    #[test]
    fn runaway_when_stability_nonpositive() {
        // Θ₀ = 0.5, ω = 0 ⇒ Θ_env = 0.5, S = 1 − 1/0.5 = −1 ≤ 0 ⇒ runaway.
        let result = two_node().resonance_gain(0.0, &tuning_with_theta0(0.5));
        assert_eq!(result, Err(ResonanceRunaway));
    }

    fn approx(got: f32, expected: f32) {
        assert!(
            (got - expected).abs() < 1e-4,
            "expected {expected}, got {got}"
        );
    }
}
