//! E16a upkeep rate + E16b depth-attenuated per-chakra occupation — an
//! execution-side read pass over a frozen [`TensionGraph`].
//!
//! A maintained construct (THEOS-178) is a standing weave the caster holds taut
//! tick after tick. The one-time assembly cost `E_base` (E9) is paid **once** at
//! spawn; from then on the construct bleeds an **upkeep rate**:
//!
//! ```text
//! U   = λ · (1/S)^μ · ρ_med · ρ_env · W        E16a — per-tick hold cost
//! u_c = exp(−α_p · D_c) · U                     E16b — per-chakra occupation
//! ```
//!
//! `W` is the frozen authoring work ([`TensionGraph::authoring_work`]); `S` is the
//! execution-side stability scalar ([`TensionGraph::stability`]); `ρ_med`/`ρ_env`
//! are live world state ([`EnvResistance`]); `D_c` is the E11 node depth
//! ([`super::Node::depth`]); `λ`/`μ`/`α_p` are [`WeaveTuning`] constants
//! (THEOS-179). Nothing here re-solves or mutates the graph — the same
//! authoring/execution seam as E9/E13/E14.
//!
//! **Convergence:** the single rate `U` both drains the caster's Energy pool `E`
//! *and* is the construct's Channel-Power `P` occupation — one rate, two roles.
//! Applying either (the pool drain, the `P` ceiling) is lifecycle/ECS work and
//! lives elsewhere; this module only derives the numbers.
//!
//! See `plans/THEOS-180.md` and the Equations of the Weave (E16a, E16b).

use super::TensionGraph;
use crate::tuning::DEFAULT_THROUGHPUT_ALPHA_P;
use crate::{Chakra, EnvResistance, WeaveTuning};

/// E16b throughput depth-attenuation `Ξ_p(D) = exp(−α_p · D)`.
///
/// The load-reach sibling of E11's [`super::injection_attenuation`] (`Ξ = exp(−α·D)`):
/// identical shape, but `α_p` governs how far a maintained construct's *load*
/// spreads into the body, not how far its *effect* reaches.
///
/// * `depth` — the E11 path length `D_c = Σ dᵢⱼ` from the injection root along the
///   oriented root→node path (`0` at the root).
/// * `alpha_p` — the attenuation constant `α_p`, or `None` for the canon default
///   [`DEFAULT_THROUGHPUT_ALPHA_P`].
///
/// Returns a factor in `(0, 1]`: exactly `1` at the root (`depth = 0`), strictly
/// decreasing as depth grows, and multiplicatively composable
/// (`exp(−α_p·Σd) = Π exp(−α_p·dᵢⱼ)`).
pub fn throughput_attenuation(depth: f32, alpha_p: Option<f32>) -> f32 {
    let alpha = alpha_p.unwrap_or(DEFAULT_THROUGHPUT_ALPHA_P);
    (-alpha * depth).exp()
}

/// The E16 upkeep of a maintained construct for one tick: the scalar rate `U`
/// (E16a) and its depth-attenuated distribution over the anchoring chakras (E16b).
#[derive(Clone, Debug, PartialEq)]
pub struct Upkeep {
    /// E16a `U = λ·(1/S)^μ·ρ_med·ρ_env·W` — the per-tick cost of holding the
    /// construct taut. Both drains the Energy pool `E` and is the construct's
    /// Channel-Power `P` occupation (one rate, two roles). Excludes the one-time
    /// spawn cost `E_base`.
    pub total: f32,
    /// E16b `u_c = exp(−α_p·D_c)·U` for each body-anchored chakra the graph runs
    /// through. **Per-conduit load, not conserved** — the injection-root chakra
    /// (`D_c = 0`) bears the full [`Self::total`], deeper anchors bear less, and
    /// `Σ u_c` may exceed [`Self::total`]. Non-body nodes (`SpatialAnchor`) do not
    /// appear.
    pub per_chakra: Vec<(Chakra, f32)>,
}

impl TensionGraph {
    /// E16a — the per-tick upkeep rate `U` needed to hold this construct.
    ///
    /// `U = λ·(1/S)^μ·ρ_med·ρ_env·W`. `W` ([`Self::authoring_work`]) and the
    /// spectral radius behind `S` are frozen at assembly; `ρ_med`/`ρ_env` (`env`)
    /// and `ω` (`world_entropy`, via [`Self::stability`]) are live world state. The
    /// one-time spawn cost `E_base` is **not** included — this is the hold cost only.
    /// The frozen graph is never mutated.
    ///
    /// **Precondition: `S > 0`.** A Volatile construct (`S ≤ 0` is reachable) cannot
    /// be maintained; enforcing that is the lifecycle story (THEOS-182). Outside the
    /// domain the result may be non-finite.
    pub fn upkeep_rate(&self, env: EnvResistance, world_entropy: f32, tuning: &WeaveTuning) -> f32 {
        let (stability, _) = self.stability(world_entropy, tuning);
        tuning.upkeep_lambda
            * (1.0 / stability).powf(tuning.upkeep_mu)
            * env.medium
            * env.environment
            * self.authoring_work()
    }

    /// E16a + E16b — the upkeep rate and its per-chakra occupation.
    ///
    /// [`Upkeep::total`] equals [`Self::upkeep_rate`] for the same inputs.
    /// [`Upkeep::per_chakra`] holds `u_c = exp(−α_p·D_c)·U` for every chakra the
    /// graph's body anchors run through, where `D_c` is the E11 depth of the
    /// **shallowest** node on that chakra (heaviest load wins when a chakra is
    /// anchored more than once). Execution-side; the frozen graph is untouched.
    /// The `S > 0` precondition of [`Self::upkeep_rate`] applies.
    pub fn upkeep(&self, env: EnvResistance, world_entropy: f32, tuning: &WeaveTuning) -> Upkeep {
        let total = self.upkeep_rate(env, world_entropy, tuning);

        // One entry per anchored chakra, in canonical order; `D_c` is the shallowest
        // depth among the nodes on that chakra, and `u_c` is derived from it once.
        let per_chakra = Chakra::ALL
            .into_iter()
            .filter_map(|chakra| {
                let d_c = self
                    .arena
                    .iter()
                    .filter(|node| node.chakra == Some(chakra))
                    .map(|node| node.depth)
                    .reduce(f32::min)?;
                let u_c = throughput_attenuation(d_c, Some(tuning.throughput_alpha_p)) * total;
                Some((chakra, u_c))
            })
            .collect();

        Upkeep { total, per_chakra }
    }
}

#[cfg(test)]
mod tests {
    use super::{Upkeep, throughput_attenuation};
    use crate::{
        Chakra, CompilerContext, DEFAULT_INJECTION_ATTENUATION, EnvResistance, GraphAssembler,
        HarmonicToken, TensionGraph, TokenId, WeaveTuning, WovenToken,
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

    /// Wave speed 1.0 so phase = 2π·d; integer edge lengths ⇒ coherence 1 on every
    /// edge (mirrors the stability-module fixtures).
    fn assemble(tokens: &[WovenToken]) -> TensionGraph {
        GraphAssembler::assemble(
            tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .expect("test graph assembles")
    }

    /// Root → spatial leaf, unit edge. `ρ(G) = 1`, `W = 1·1 = 1`. Anchored at Root
    /// only.
    fn two_node() -> TensionGraph {
        assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ])
    }

    /// `Θ₀ = 3` keeps `S` comfortably positive on `ρ(G) = 1` graphs: at `ω = 0`,
    /// `S = 1 − 1/3 = 2/3`. Defaults leave `λ = μ = 1`.
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

    /// The E16b occupation recorded for a given chakra.
    fn occ(up: &Upkeep, chakra: Chakra) -> f32 {
        up.per_chakra
            .iter()
            .find(|(c, _)| *c == chakra)
            .map(|(_, u)| *u)
            .unwrap_or_else(|| panic!("no upkeep entry for {chakra:?}"))
    }

    // --- throughput_attenuation (E16b Ξ_p) — mirrors the injection_attenuation tests ---

    #[test]
    fn throughput_attenuation_is_one_at_zero_depth() {
        assert_eq!(throughput_attenuation(0.0, None), 1.0);
    }

    #[test]
    fn throughput_attenuation_strictly_decreases_with_depth() {
        let mut previous = 1.0;
        for depth in 1..10 {
            let attenuation = throughput_attenuation(depth as f32, None);
            assert!(attenuation < previous, "must decrease at depth {depth}");
            previous = attenuation;
        }
    }

    #[test]
    fn throughput_attenuation_composes_multiplicatively() {
        let a = 2.0;
        let b = 3.0;
        let lhs = throughput_attenuation(a + b, None);
        let rhs = throughput_attenuation(a, None) * throughput_attenuation(b, None);
        assert!((lhs - rhs).abs() < 1e-6);
    }

    // --- E16a: upkeep_rate (AC1, AC2) ---

    #[test]
    fn upkeep_rate_matches_manual_formula() {
        // AC1: reconstruct U from the published sub-terms, independent of the
        // method's internal wiring. Closed form: λ(1)·(1/(2/3))^1·1·1·W(1) = 1.5.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let (s, _) = g.stability(0.0, &tuning);
        let expected = tuning.upkeep_lambda
            * (1.0 / s).powf(tuning.upkeep_mu)
            * env.medium
            * env.environment
            * g.authoring_work();
        approx(g.upkeep_rate(env, 0.0, &tuning), expected);
        assert!((g.upkeep_rate(env, 0.0, &tuning) - 1.5).abs() < 1e-4);
    }

    #[test]
    fn upkeep_rate_excludes_the_e_base_spawn_cost() {
        // AC1: E_base is a spawn cost, absent from U — so inflating it moves
        // energy_cost but leaves upkeep_rate untouched.
        let g = two_node();
        let env = EnvResistance::default();
        let small = WeaveTuning {
            e_base: 1.0,
            ..stable_tuning()
        };
        let large = WeaveTuning {
            e_base: 1_000.0,
            ..stable_tuning()
        };
        approx(
            g.upkeep_rate(env, 0.0, &large),
            g.upkeep_rate(env, 0.0, &small),
        );
        assert!(
            g.energy_cost(env, &large) > g.energy_cost(env, &small),
            "sanity: E_base does move energy_cost",
        );
    }

    #[test]
    fn upkeep_rate_scales_with_authoring_work() {
        // Same ρ(G) = 1 ⇒ same S; only W differs (1 vs 3) ⇒ U scales 3×.
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let short = two_node();
        let long = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(3.0, 0.0, 0.0), vec![]),
        ]);
        let u_short = short.upkeep_rate(env, 0.0, &tuning);
        let u_long = long.upkeep_rate(env, 0.0, &tuning);
        approx(u_long, 3.0 * u_short);
    }

    #[test]
    fn upkeep_rate_rises_with_medium_and_environment_resistance() {
        // AC2: U is linear in ρ_med·ρ_env — (2, 3) ⇒ 6× the resistance-free rate.
        let g = two_node();
        let tuning = stable_tuning();
        let free = g.upkeep_rate(EnvResistance::default(), 0.0, &tuning);
        let resisted = g.upkeep_rate(
            EnvResistance {
                medium: 2.0,
                environment: 3.0,
            },
            0.0,
            &tuning,
        );
        approx(resisted, 6.0 * free);
    }

    #[test]
    fn upkeep_rate_rises_as_world_entropy_thins_the_stability_margin() {
        // AC2: higher ω ⇒ smaller Θ_env ⇒ smaller S ⇒ larger (1/S)^μ ⇒ larger U.
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        let calm = g.upkeep_rate(env, 0.0, &tuning);
        let turbulent = g.upkeep_rate(env, 0.5, &tuning);
        assert!(
            turbulent > calm,
            "a thinner margin must cost more to hold: {turbulent} !> {calm}"
        );
    }

    #[test]
    fn same_construct_two_environments_differ_only_in_upkeep_rate() {
        // AC2: the frozen graph is untouched; only the derived rate moves.
        let g = two_node();
        let tuning = stable_tuning();
        let w = g.authoring_work();
        let rho = g.rho_g();
        let cast = g.cast_time();

        let a = g.upkeep_rate(EnvResistance::default(), 0.0, &tuning);
        let b = g.upkeep_rate(
            EnvResistance {
                medium: 4.0,
                environment: 1.0,
            },
            0.0,
            &tuning,
        );
        assert!(a != b, "different environments ⇒ different U");
        approx(g.authoring_work(), w);
        approx(g.rho_g(), rho);
        approx(g.cast_time(), cast);
    }

    #[test]
    fn upkeep_rate_with_mu_zero_is_margin_blind() {
        // μ = 0 ⇒ (1/S)^0 = 1 ⇒ U = λ·ρ·ρ·W, independent of S (hence of ω).
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = WeaveTuning {
            upkeep_mu: 0.0,
            ..stable_tuning()
        };
        approx(g.upkeep_rate(env, 0.0, &tuning), 1.0);
        approx(
            g.upkeep_rate(env, 0.0, &tuning),
            g.upkeep_rate(env, 0.5, &tuning),
        );
    }

    // --- E16b: per-chakra occupation (AC3 + D4) ---

    #[test]
    fn upkeep_total_equals_upkeep_rate() {
        let g = two_node();
        let env = EnvResistance::default();
        let tuning = stable_tuning();
        approx(
            g.upkeep(env, 0.0, &tuning).total,
            g.upkeep_rate(env, 0.0, &tuning),
        );
    }

    #[test]
    fn upkeep_root_chakra_bears_the_full_rate() {
        // AC3: the injection root sits at D_c = 0 ⇒ exp(0) = 1 ⇒ u_root = total.
        let g = two_node();
        let up = g.upkeep(EnvResistance::default(), 0.0, &stable_tuning());
        approx(occ(&up, Chakra::Root), up.total);
    }

    #[test]
    fn upkeep_deeper_anchor_bears_less_than_root() {
        // AC3: Root (D = 0) bears the full rate; a downstream body anchor bears
        // exp(−α_p·D_c)·total < total.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ]);
        let tuning = stable_tuning();
        let up = g.upkeep(EnvResistance::default(), 0.0, &tuning);

        let deep_id = g.neighbors(g.root())[0].target;
        let deep = g.node(deep_id).expect("leaf node");
        assert_eq!(deep.chakra, Some(Chakra::Sacral));
        let expected_deep =
            throughput_attenuation(deep.depth, Some(tuning.throughput_alpha_p)) * up.total;

        approx(occ(&up, Chakra::Root), up.total);
        approx(occ(&up, Chakra::Sacral), expected_deep);
        assert!(occ(&up, Chakra::Sacral) < occ(&up, Chakra::Root));
    }

    #[test]
    fn upkeep_per_chakra_sum_may_exceed_total() {
        // AC3: occupation is per-conduit load, not a conserved split.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Sacral, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ]);
        let up = g.upkeep(EnvResistance::default(), 0.0, &stable_tuning());
        let sum: f32 = up.per_chakra.iter().map(|(_, u)| u).sum();
        assert!(sum > up.total, "Σ u_c ({sum}) must exceed U ({})", up.total);
    }

    #[test]
    fn upkeep_per_chakra_uses_shallowest_node_when_a_chakra_repeats() {
        // D4: two nodes anchor Heart at different depths; the shallower one sets D_c.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            body(1, Chakra::Heart, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            body(2, Chakra::Heart, Vec3::new(3.0, 0.0, 0.0), vec![]),
        ]);
        let tuning = stable_tuning();
        let up = g.upkeep(EnvResistance::default(), 0.0, &tuning);

        let shallow_id = g.neighbors(g.root())[0].target;
        let shallow_depth = g.node(shallow_id).expect("first Heart node").depth;
        let expected =
            throughput_attenuation(shallow_depth, Some(tuning.throughput_alpha_p)) * up.total;

        assert_eq!(
            up.per_chakra
                .iter()
                .filter(|(c, _)| *c == Chakra::Heart)
                .count(),
            1,
            "a repeated chakra collapses to one entry",
        );
        approx(occ(&up, Chakra::Heart), expected);
    }

    #[test]
    fn upkeep_omits_non_body_nodes() {
        // The spatial leaf carries no chakra ⇒ only Root appears.
        let g = two_node();
        let up = g.upkeep(EnvResistance::default(), 0.0, &stable_tuning());
        assert_eq!(up.per_chakra.len(), 1);
        assert_eq!(up.per_chakra[0].0, Chakra::Root);
    }
}
