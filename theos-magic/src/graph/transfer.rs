//! E12 temporal transfer response: a per-element Gaussian band-pass + delay, and the
//! parallel-filterbank (union) composition of a stack of elements on one node.
//!
//! A `TemporalFrequency` token does **not** become a node — it lowers to a
//! [`TemporalTransfer`] attached to the positional node its links resolve to, and
//! filters that node's *incoming* feeder contributions (throughput-only) before the
//! E4 phasor sum. Stacking is allowed: several elements on one node form a parallel
//! filterbank, combined by the per-frequency maximum. See the Equations of the Weave
//! (E12):
//!
//! ```text
//! H(f)       = exp(−(f − f₀)² / (2·σ²))        per-element band-pass, H ∈ (0, 1]
//! τ          = clamp(duration_ms/1000, τ_min, τ_max)
//! σ          = 1 / (k · τ)                      Gabor width, default k = 2π
//! Δφ(f)      = 2π · f · τ                        delay phase (E2 temporal form)
//! H_total(f) = maxₑ Hₑ(f)                        union of a stack (parallel filterbank)
//! τ(f)       = τ of the band maximizing Hₑ(f)    dominant-band delay
//! ```

use crate::tuning::WeaveTuning;

/// One frequency-selective transfer element (E12): a Gaussian band-pass plus a
/// propagation delay, attached to a positional node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemporalTransfer {
    /// Band-pass centre frequency `f₀` (Hz) — the token's `frequency_hz`.
    pub center_hz: f32,
    /// Gaussian spectral width `σ` (Hz), derived from the token's duration.
    pub sigma_hz: f32,
    /// Propagation delay `τ` (seconds) imparted to a wave crossing the element.
    pub delay_s: f32,
}

impl TemporalTransfer {
    /// Build a transfer element from a `TemporalFrequency` token's fields under the
    /// given [`WeaveTuning`]: `center_hz = frequency_hz`, with `sigma_hz` and
    /// `delay_s` derived from `duration_ms` via E12.
    pub fn from_token(frequency_hz: f32, duration_ms: u32, tuning: &WeaveTuning) -> Self {
        Self {
            center_hz: frequency_hz,
            sigma_hz: sigma_from_duration(duration_ms, tuning),
            delay_s: clamp_tau_seconds(duration_ms, tuning),
        }
    }
}

/// E12 per-element band-pass response `H(f) = exp(−(f − f₀)² / (2σ²))`, in `(0, 1]`.
///
/// `1` exactly at `f == center_hz`, decreasing as `freq` moves off centre.
pub fn band_pass_response(freq: f32, center_hz: f32, sigma_hz: f32) -> f32 {
    (-(freq - center_hz).powi(2) / (2.0 * sigma_hz.powi(2))).exp()
}

/// The duration → seconds conversion with the E12 clamp:
/// `τ = clamp(duration_ms / 1000, τ_min, τ_max)`.
///
/// Guards the `τ→0` (all-pass) and `τ→∞` (numerical delta) extremes via
/// [`WeaveTuning::tau_min_s`] / [`WeaveTuning::tau_max_s`].
pub fn clamp_tau_seconds(duration_ms: u32, tuning: &WeaveTuning) -> f32 {
    (duration_ms as f32 / 1000.0).clamp(tuning.tau_min_s, tuning.tau_max_s)
}

/// E12 Gabor spectral width `σ = 1 / (k · τ)`, with `τ` clamped per
/// [`clamp_tau_seconds`] and `k = tuning.bandwidth_k`.
///
/// Width is reciprocal to duration: a longer tone is sharper (smaller `σ`).
pub fn sigma_from_duration(duration_ms: u32, tuning: &WeaveTuning) -> f32 {
    1.0 / (tuning.bandwidth_k * clamp_tau_seconds(duration_ms, tuning))
}

/// E12 parallel-filterbank (union) composition of a stack of elements at frequency
/// `freq`.
///
/// Returns `(H_total, delay)` where `H_total = maxₑ Hₑ(freq)` (a feeder passes if it
/// lies in **any** stacked band) and `delay` is the dominant band's `τ` (the element
/// maximizing `Hₑ`). An empty stack is pass-through: `(1.0, 0.0)`.
pub fn filter_bank_response(transfers: &[TemporalTransfer], freq: f32) -> (f32, f32) {
    if transfers.is_empty() {
        return (1.0, 0.0);
    }

    let mut h_total: f32 = 0.0;
    let mut delay: f32 = 0.0;

    for transfer in transfers {
        let h = band_pass_response(freq, transfer.center_hz, transfer.sigma_hz);
        if h > h_total {
            h_total = h;
            delay = transfer.delay_s;
        }
    }

    (h_total, delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(got: f32, expected: f32) {
        assert!(
            (got - expected).abs() < 1e-5,
            "expected {expected}, got {got}"
        );
    }

    #[test]
    fn band_pass_response_is_one_at_center() {
        approx(band_pass_response(440.0, 440.0, 5.0), 1.0);
    }

    #[test]
    fn band_pass_response_attenuates_off_center() {
        let (center, sigma) = (440.0_f32, 5.0_f32);
        let at_center = band_pass_response(center, center, sigma);
        let one_sigma = band_pass_response(center + sigma, center, sigma);
        // Strictly attenuated away from centre…
        assert!(one_sigma < at_center);
        // …and exactly the Gaussian closed form: H(f₀+σ) = exp(−σ²/(2σ²)) = exp(−0.5).
        approx(one_sigma, (-0.5_f32).exp());
    }

    #[test]
    fn band_pass_response_never_exceeds_one() {
        // H is mathematically in (0, 1], but far off-centre it f32-underflows to a
        // clean 0.0 — so the meaningful invariants are: never negative, never > 1.
        for &f in &[0.0, 100.0, 440.0, 441.0, 900.0, 5000.0] {
            let h = band_pass_response(f, 440.0, 5.0);
            assert!(h >= 0.0, "H({f}) = {h} must not go negative");
            assert!(h <= 1.0, "H({f}) = {h} must not exceed 1");
        }
    }

    #[test]
    fn clamp_tau_pins_zero_duration_to_min() {
        let tuning = WeaveTuning::default();
        approx(clamp_tau_seconds(0, &tuning), tuning.tau_min_s);
    }

    #[test]
    fn clamp_tau_pins_huge_duration_to_max() {
        let tuning = WeaveTuning::default();
        approx(clamp_tau_seconds(u32::MAX, &tuning), tuning.tau_max_s);
    }

    #[test]
    fn sigma_is_reciprocal_to_duration() {
        let tuning = WeaveTuning::default();
        // 1000 ms → τ = 1.0 s (inside the clamp window), so σ = 1/(k·1).
        approx(sigma_from_duration(1000, &tuning), 1.0 / tuning.bandwidth_k);
    }

    #[test]
    fn longer_duration_gives_a_narrower_band() {
        let tuning = WeaveTuning::default();
        let short = sigma_from_duration(500, &tuning);
        let long = sigma_from_duration(2000, &tuning);
        assert!(long < short, "longer τ should narrow σ: {long} !< {short}");
    }

    #[test]
    fn from_token_centers_on_frequency_hz() {
        let tuning = WeaveTuning::default();
        let transfer = TemporalTransfer::from_token(440.0, 1000, &tuning);
        approx(transfer.center_hz, 440.0);
        approx(transfer.sigma_hz, sigma_from_duration(1000, &tuning));
        approx(transfer.delay_s, clamp_tau_seconds(1000, &tuning));
    }

    #[test]
    fn filter_bank_empty_stack_is_pass_through() {
        let (h, delay) = filter_bank_response(&[], 440.0);
        approx(h, 1.0);
        approx(delay, 0.0);
    }

    #[test]
    fn filter_bank_far_out_feeder_against_a_nonempty_bank_is_blocked() {
        // A feeder ~100σ outside the only band: every Hₑ f32-underflows to 0, so the
        // bank blocks it (≈0). It must NOT be mistaken for an empty (pass-through) bank.
        let band = TemporalTransfer {
            center_hz: 100.0,
            sigma_hz: 1.0,
            delay_s: 0.05,
        };
        let (h, _) = filter_bank_response(&[band], 200.0);
        assert!(
            h < 1e-6,
            "far-out feeder should be blocked, not passed: H = {h}"
        );
    }

    #[test]
    fn filter_bank_union_passes_a_feeder_in_any_stacked_band() {
        // Two narrow bands; a feeder sitting on the second still passes (union),
        // even though it is far outside the first.
        let low = TemporalTransfer {
            center_hz: 100.0,
            sigma_hz: 5.0,
            delay_s: 0.01,
        };
        let high = TemporalTransfer {
            center_hz: 400.0,
            sigma_hz: 5.0,
            delay_s: 0.02,
        };
        let (h, _) = filter_bank_response(&[low, high], 400.0);
        assert!(h > 0.99, "a feeder in the high band should pass: H = {h}");
    }

    #[test]
    fn filter_bank_total_never_exceeds_one() {
        let a = TemporalTransfer {
            center_hz: 100.0,
            sigma_hz: 50.0,
            delay_s: 0.01,
        };
        let b = TemporalTransfer {
            center_hz: 120.0,
            sigma_hz: 50.0,
            delay_s: 0.02,
        };
        for &f in &[0.0, 100.0, 110.0, 120.0, 500.0] {
            let (h, _) = filter_bank_response(&[a, b], f);
            assert!(
                h <= 1.0 + 1e-6,
                "stacking manufactured gain at {f}: H = {h}"
            );
        }
    }

    #[test]
    fn filter_bank_delay_is_the_dominant_band() {
        // A feeder on the high band's centre ⇒ that band dominates ⇒ its delay wins.
        let low = TemporalTransfer {
            center_hz: 100.0,
            sigma_hz: 5.0,
            delay_s: 0.01,
        };
        let high = TemporalTransfer {
            center_hz: 400.0,
            sigma_hz: 5.0,
            delay_s: 0.02,
        };
        let (_, delay) = filter_bank_response(&[low, high], 400.0);
        approx(delay, 0.02);
    }
}
