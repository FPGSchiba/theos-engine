//! Engine-physics tuning constants for weave assembly.
//!
//! [`WeaveTuning`] is **engine-physics configuration**, distinct in kind from the
//! caster/world *state* carried by [`crate::CompilerContext`]; it is carried as one
//! named field there. It collects the knobs the Equations of the Weave leave open —
//! the E12 temporal-filter constants, the E9 energy floor, the E5–E7 stability
//! coefficients, the E15 gain steepness, and the E16 maintained-construct constants;
//! more tunables may join it over time.

use serde::{Deserialize, Serialize};

/// Default E12 bandwidth constant `k` in `σ = 1/(k·τ)`.
///
/// `2π` sits a factor of two above the Gabor uncertainty floor `1/(4π)` — see the
/// Equations of the Weave, E12.
pub const DEFAULT_BANDWIDTH_K: f32 = std::f32::consts::TAU;

/// Default lower clamp on the temporal duration `τ` (seconds).
///
/// Guards the `τ→0` extreme, which would send `σ→∞` (an all-pass `H ≡ 1`, i.e. no
/// filtering at all). E12.
pub const DEFAULT_TAU_MIN_S: f32 = 1.0e-3;

/// Default upper clamp on the temporal duration `τ` (seconds).
///
/// Guards the `τ→∞` extreme, which would send `σ→0` (a numerical delta /
/// divide-by-near-zero). E12.
pub const DEFAULT_TAU_MAX_S: f32 = 60.0;

/// Default base energy floor `E_base` (E9, D5).
///
/// The minimum cost of any cast, so `E_cost > 0` always and efficiency η (E10) is
/// always defined. Invariant: **`> 0`**.
// TODO(THEOS-159): `1.0` is a placeholder — confirm the tuned value (a physics/balance call).
pub const DEFAULT_E_BASE: f32 = 1.0;

/// Default edge tension coefficient `kᵢⱼ` (E3/E6) — string stiffness.
// TODO(THEOS-150): `1.0` is a placeholder — confirm the tuned value (a physics/balance call).
pub const DEFAULT_STIFFNESS: f32 = 1.0;

/// Default reference stiffness `k_ref` (E6) — the normalizer in `Gᵢⱼ = (kᵢⱼ/k_ref)·Iᵢⱼ`.
/// Equal to [`DEFAULT_STIFFNESS`] so `kᵢⱼ/k_ref = 1` at rest ⇒ `G ≡` coherence matrix (D2).
pub const DEFAULT_K_REF: f32 = DEFAULT_STIFFNESS;

/// Default frequency tolerance `σ_f` (E5) — Gaussian coherence width.
// TODO(THEOS-150): `1.0` is a placeholder — confirm the tuned value.
pub const DEFAULT_SIGMA_F: f32 = 1.0;

/// Default base stability ceiling `Θ₀` (E7) — `Θ_env = Θ₀·(1 − ω)`.
// TODO(THEOS-150): `1.0` is a placeholder — confirm the tuned value.
pub const DEFAULT_THETA_0: f32 = 1.0;

/// Default stability threshold `S_min` (E7) — stable iff `S ≥ S_min`, else volatile.
/// Canon default `0.5`.
pub const DEFAULT_S_MIN: f32 = 0.5;

/// Default resonance-gain steepness `γ` (E15) — exponent in `Q = 1/Sᵞ`. `> 0`;
/// larger γ ramps power harder as the stability margin thins.
// TODO(THEOS-176): `2.0` is a placeholder — confirm the tuned value.
pub const DEFAULT_GAMMA: f32 = 2.0;

/// Default E16a upkeep leak-rate `λ` — magnitude of the per-tick hold cost in
/// `U = λ·(1/S)^μ·ρ_med·ρ_env·W`. Invariant: **`> 0`**.
// TODO(THEOS-179): `1.0` is a placeholder — confirm the tuned value (a physics/balance call).
pub const DEFAULT_UPKEEP_LAMBDA: f32 = 1.0;

/// Default E16a upkeep power-tax exponent `μ` — how much harder a thin-margin
/// construct bleeds to hold. `≥ 0`, and **independent of the E15 reward exponent `γ`**;
/// `1.0` starts it linear and below the [`DEFAULT_GAMMA`] of `2.0`.
// TODO(THEOS-179): `1.0` is a placeholder — confirm the tuned value (a physics/balance call).
pub const DEFAULT_UPKEEP_MU: f32 = 1.0;

/// Default E16b throughput depth-attenuation `α_p` in `Ξ_p(D) = exp(−α_p·D)` — how far
/// a maintained construct's *load* spreads into the body. `> 0`, and **distinct from
/// E11's amplitude constant `α`**; mirrors that constant's `0.15` per world length
/// unit as a starting point.
// TODO(THEOS-179): `0.15` mirrors E11 `α` — confirm the tuned value (a physics/balance call).
pub const DEFAULT_THROUGHPUT_ALPHA_P: f32 = 0.15;

/// Default E16c recovery-rate constant `κ` — seconds of construction-transient
/// recovery per unit cost, in `τ_p = κ·E_cost`. Invariant: **`> 0`**.
// TODO(THEOS-179): `1.0` is a placeholder — confirm the tuned value (a physics/balance call).
pub const DEFAULT_RECOVERY_KAPPA: f32 = 1.0;

/// Default E16c construction-transient gain `ζ` — the spike magnitude in
/// `Δ = ζ·E_cost` a manifesting construct adds to a chakra's occupation above the
/// steady floor, before depth attenuation (E16c, THEOS-181). Invariant: **`> 0`**.
///
/// The Equations of the Weave state the spike only as `Δ ∝ E_cost`; `ζ` is the
/// proportionality constant this engine pins (canon writeback pending).
// TODO(THEOS-179): `1.0` is a placeholder — confirm the tuned value (a physics/balance call).
pub const DEFAULT_TRANSIENT_SPIKE_GAIN: f32 = 1.0;

/// Default E16c construction-transient depth-attenuation `α_Δ` in
/// `Ξ_Δ(D) = exp(−α_Δ·D)` — how far the manifestation shock spreads into the body
/// from the injection root.
///
/// `≥ 0`, and **independent of E16b's load constant `α_p`**
/// ([`DEFAULT_THROUGHPUT_ALPHA_P`]) — the same independence discipline that keeps
/// E11 `α` and E16b `α_p` apart: `α_Δ = 0` ⇒ the spike is flat across every anchor,
/// `α_Δ = α_p` ⇒ it spreads exactly like the steady floor, `α_Δ > α_p` ⇒ the shock
/// is more root-localized than the standing load. Mirrors `α_p`'s `0.15` as a
/// starting point.
// TODO(THEOS-179): `0.15` mirrors α_p — confirm the tuned value (a physics/balance call).
pub const DEFAULT_TRANSIENT_SPIKE_ALPHA: f32 = 0.15;

/// Engine-physics tuning for weave assembly — currently the E12 temporal-filter knobs.
///
/// Distinct in kind from the caster/world *state* on [`crate::CompilerContext`]:
/// this is configuration, not a per-cast world snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeaveTuning {
    /// Band-pass width constant `k` in `σ = 1/(k·τ)` (E12). Default `2π`.
    pub bandwidth_k: f32,
    /// Lower clamp on `τ` (seconds) — guards the `τ→0` all-pass extreme (E12).
    pub tau_min_s: f32,
    /// Upper clamp on `τ` (seconds) — guards the `τ→∞` numerical-delta extreme (E12).
    pub tau_max_s: f32,
    /// Base energy floor `E_base` (E9) — keeps `E_cost > 0` so η is always defined.
    /// Invariant: `> 0`.
    pub e_base: f32,
    /// Default edge stiffness `kᵢⱼ` (E3/E6) applied to edges at assembly.
    pub stiffness: f32,
    /// Reference stiffness `k_ref` (E6) — the `Gᵢⱼ = (kᵢⱼ/k_ref)·Iᵢⱼ` normalizer.
    pub k_ref: f32,
    /// Default edge frequency tolerance `σ_f` (E5).
    pub sigma_f: f32,
    /// Base stability ceiling `Θ₀` (E7) — `Θ_env = Θ₀·(1 − ω)`.
    pub theta_0: f32,
    /// Stability threshold `S_min` (E7) — stable iff `S ≥ S_min`. Default `0.5`.
    pub s_min: f32,
    /// Resonance-gain steepness `γ` (E15) — exponent in `Q = 1/Sᵞ`. `> 0`.
    pub gamma: f32,
    /// E16a upkeep leak-rate `λ` in `U = λ·(1/S)^μ·ρ_med·ρ_env·W` — the magnitude of
    /// holding a maintained construct taut for one tick. Invariant: `> 0`.
    pub upkeep_lambda: f32,
    /// E16a upkeep power-tax exponent `μ` — how much harder a thin-margin (Dark,
    /// `S → S_min`) construct bleeds to hold. `≥ 0`; **independent of the E15 reward
    /// exponent `γ`** (`μ = 0` margin-blind, `μ = γ` thirst-tracks-power, `μ > γ`
    /// self-limiting).
    pub upkeep_mu: f32,
    /// E16b throughput depth-attenuation `α_p` in `Ξ_p(D) = exp(−α_p·D)` — how far a
    /// maintained construct's *load* spreads into the body from the injection root.
    /// `> 0`; **distinct from E11's amplitude constant `α`** (load reach ≠ effect reach).
    pub throughput_alpha_p: f32,
    /// E16c recovery-rate constant `κ` in `τ_p = κ·E_cost` — seconds of construction-
    /// transient recovery per unit cost; a costlier construct recovers more slowly.
    /// Invariant: `> 0`.
    pub recovery_kappa: f32,
    /// E16c construction-transient gain `ζ` in `Δ = ζ·E_cost` — the spike a
    /// manifesting construct injects onto a chakra's occupation above the steady
    /// floor, before depth attenuation. Invariant: `> 0`.
    pub transient_spike_gain: f32,
    /// E16c construction-transient depth-attenuation `α_Δ` in `Ξ_Δ(D) = exp(−α_Δ·D)`
    /// — how far the manifestation shock spreads from the injection root. `≥ 0`;
    /// **independent of E16b's load constant `α_p`** (`α_Δ = 0` ⇒ flat across every
    /// anchor, `α_Δ = α_p` ⇒ spreads like the floor).
    pub transient_spike_alpha: f32,
}

impl Default for WeaveTuning {
    fn default() -> Self {
        Self {
            bandwidth_k: DEFAULT_BANDWIDTH_K,
            tau_min_s: DEFAULT_TAU_MIN_S,
            tau_max_s: DEFAULT_TAU_MAX_S,
            e_base: DEFAULT_E_BASE,
            stiffness: DEFAULT_STIFFNESS,
            k_ref: DEFAULT_K_REF,
            sigma_f: DEFAULT_SIGMA_F,
            theta_0: DEFAULT_THETA_0,
            s_min: DEFAULT_S_MIN,
            gamma: DEFAULT_GAMMA,
            upkeep_lambda: DEFAULT_UPKEEP_LAMBDA,
            upkeep_mu: DEFAULT_UPKEEP_MU,
            throughput_alpha_p: DEFAULT_THROUGHPUT_ALPHA_P,
            recovery_kappa: DEFAULT_RECOVERY_KAPPA,
            transient_spike_gain: DEFAULT_TRANSIENT_SPIKE_GAIN,
            transient_spike_alpha: DEFAULT_TRANSIENT_SPIKE_ALPHA,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bandwidth_constant_is_tau() {
        assert_eq!(WeaveTuning::default().bandwidth_k, std::f32::consts::TAU);
    }

    #[test]
    fn default_tau_clamp_window_is_ordered() {
        let tuning = WeaveTuning::default();
        assert!(tuning.tau_min_s < tuning.tau_max_s);
    }

    #[test]
    fn default_e16_constants_are_within_their_domains() {
        let tuning = WeaveTuning::default();
        assert!(tuning.upkeep_lambda > 0.0);
        assert!(tuning.upkeep_mu >= 0.0);
        assert!(tuning.throughput_alpha_p > 0.0);
        assert!(tuning.recovery_kappa > 0.0);
        assert!(tuning.transient_spike_gain > 0.0);
        assert!(tuning.transient_spike_alpha >= 0.0);
    }

    #[test]
    fn default_transient_spike_alpha_is_independent_of_e16b_load_alpha() {
        // α_Δ (spike spread) and α_p (steady-load spread) are separate knobs: moving
        // one must not move the other, mirroring the E11 `α` / E16b `α_p` split.
        let tuning = WeaveTuning {
            transient_spike_alpha: DEFAULT_THROUGHPUT_ALPHA_P + 1.0,
            ..WeaveTuning::default()
        };
        assert_ne!(tuning.transient_spike_alpha, tuning.throughput_alpha_p);
        assert_eq!(
            WeaveTuning::default().transient_spike_alpha,
            DEFAULT_TRANSIENT_SPIKE_ALPHA
        );
        assert_eq!(
            WeaveTuning::default().throughput_alpha_p,
            DEFAULT_THROUGHPUT_ALPHA_P
        );
    }

    #[test]
    fn default_upkeep_power_tax_is_independent_of_e15_gain_steepness() {
        let tuning = WeaveTuning {
            upkeep_mu: 3.0,
            gamma: 7.0,
            ..WeaveTuning::default()
        };
        assert_eq!(tuning.upkeep_mu, 3.0);
        assert_eq!(tuning.gamma, 7.0);
        assert_eq!(WeaveTuning::default().upkeep_mu, DEFAULT_UPKEEP_MU);
    }

    #[test]
    fn default_throughput_attenuation_is_independent_of_e11_amplitude_alpha() {
        use crate::graph::DEFAULT_INJECTION_ATTENUATION;

        let tuning = WeaveTuning {
            throughput_alpha_p: DEFAULT_INJECTION_ATTENUATION + 1.0,
            ..WeaveTuning::default()
        };
        assert_ne!(tuning.throughput_alpha_p, DEFAULT_INJECTION_ATTENUATION);
        assert_eq!(
            WeaveTuning::default().throughput_alpha_p,
            DEFAULT_THROUGHPUT_ALPHA_P
        );
    }

    #[test]
    fn weave_tuning_round_trips_through_json() {
        let tuning = WeaveTuning {
            upkeep_lambda: 10.0,
            e_base: 11.0,
            stiffness: 12.0,
            k_ref: 13.0,
            sigma_f: 14.0,
            theta_0: 15.0,
            s_min: 16.0,
            gamma: 17.0,
            upkeep_mu: 18.0,
            throughput_alpha_p: 19.0,
            recovery_kappa: 20.0,
            transient_spike_gain: 21.0,
            transient_spike_alpha: 22.0,
            ..WeaveTuning::default()
        };
        let tuning_string = serde_json::to_string(&tuning).unwrap();
        let tuning_json: WeaveTuning = serde_json::from_str(tuning_string.as_str()).unwrap();
        assert_eq!(tuning_json, tuning);
    }
}
