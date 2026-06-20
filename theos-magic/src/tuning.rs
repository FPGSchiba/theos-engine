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
}

impl Default for WeaveTuning {
    fn default() -> Self {
        Self {
            bandwidth_k: DEFAULT_BANDWIDTH_K,
            tau_min_s: DEFAULT_TAU_MIN_S,
            tau_max_s: DEFAULT_TAU_MAX_S,
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
