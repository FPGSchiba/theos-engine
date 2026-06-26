//! Engine-physics tuning constants for weave assembly.
//!
//! [`WeaveTuning`] is **engine-physics configuration**, distinct in kind from the
//! caster/world *state* carried by [`crate::CompilerContext`]; it is carried as one
//! named field there. For now it holds the E12 temporal-filter knobs (the band-pass
//! width constant and the duration clamps); more tunables may join it over time.

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

/// Engine-physics tuning for weave assembly — currently the E12 temporal-filter knobs.
///
/// Distinct in kind from the caster/world *state* on [`crate::CompilerContext`]:
/// this is configuration, not a per-cast world snapshot.
#[derive(Clone, Copy, Debug, PartialEq)]
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
}
