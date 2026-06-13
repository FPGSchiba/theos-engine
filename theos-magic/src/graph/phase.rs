//! E2 phase primitives: per-edge phase shift and propagation delay.
//!
//! These are the building blocks the assembler accumulates along oriented
//! root→node paths to derive each node's phase `φⱼ` and the graph's critical-path
//! delay. See E2 on the Equations of the Weave page:
//!
//! ```text
//! τᵢⱼ  = dᵢⱼ / c
//! Δφᵢⱼ = 2π·fᵢ·dᵢⱼ / c
//! ```

use std::f32::consts::PI;

/// Default wave propagation speed `c`, in world units per second.
///
/// A tunable placeholder; callers may pass any `c` to
/// [`crate::GraphAssembler::assemble`] (e.g. an environment-dependent value).
pub const DEFAULT_WAVE_SPEED: f32 = 1.0;

/// E2 phase shift across an edge: `Δφ = 2π·f·d / c`.
///
/// * `frequency` — the source node's frequency `fᵢ` (Hz).
/// * `distance` — the edge length `dᵢⱼ = |xⱼ − xᵢ|`.
/// * `wave_speed` — propagation speed `c`.
pub fn edge_phase_shift(frequency: f32, distance: f32, wave_speed: f32) -> f32 {
    (2.0 * PI * frequency * distance) / wave_speed
}

/// E2 propagation delay across an edge: `τ = d / c`.
///
/// * `distance` — the edge length `dᵢⱼ`.
/// * `wave_speed` — propagation speed `c`.
pub fn edge_delay(distance: f32, wave_speed: f32) -> f32 {
    distance / wave_speed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn edge_phase_shift_matches_e2_closed_form() {
        // Δφ = 2π·f·d / c
        let expected = 2.0 * PI * 2.0 * 3.0 / 4.0;
        assert!((edge_phase_shift(2.0, 3.0, 4.0) - expected).abs() < 1e-6);
    }

    #[test]
    fn edge_delay_is_distance_over_wave_speed() {
        assert!((edge_delay(6.0, 2.0) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn zero_distance_yields_zero_phase_shift() {
        assert_eq!(edge_phase_shift(5.0, 0.0, 1.0), 0.0);
        assert_eq!(edge_delay(0.0, DEFAULT_WAVE_SPEED), 0.0);
    }
}
