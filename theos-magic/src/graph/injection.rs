//! E11 injection-depth attenuation + the `KinematicVector` source primitive.
//!
//! A `KinematicVector` is not a node ([`crate::HarmonicToken::KinematicVector`]);
//! it is an impulse whose momentum `m·v` is injected as a *source amplitude* into
//! the node its links resolve to — or the injection root, as a fallback. Injecting
//! deeper into the graph delivers less: the source is scaled by the E11 depth
//! attenuation factor before it enters the E4 superposition. See the Equations of
//! the Weave (E4, E9, E11):
//!
//! ```text
//! Ξ(n) = exp(−α · D(n))           depth attenuation, exactly 1 at the root
//! D(n) = Σ dᵢⱼ over edges r→n      accumulated path length (the E2 distances)
//! A_src(n) ⟶ Ξ(n) · A_src(n)       applied before the E4 sum
//! ```

/// Default E11 attenuation constant `α`, in inverse world-length units.
///
/// Canon default (Equations of the Weave, E11); a larger `α` punishes injection
/// depth harder. Callers may pass any `α` to the assembler.
pub const DEFAULT_INJECTION_ATTENUATION: f32 = 0.15;

/// E11 injection-depth attenuation `Ξ = exp(−α·D)`.
///
/// * `depth` — accumulated path length `D(n) = Σ dᵢⱼ` from the injection root
///   along the oriented root→`n` path (`0` at the root itself).
/// * `alpha` — the attenuation constant `α`, or `None` to use the canon default
///   [`DEFAULT_INJECTION_ATTENUATION`].
///
/// Returns a factor in `(0, 1]`: exactly `1` at the root (`depth = 0`) and strictly
/// decreasing as depth grows. Composes multiplicatively
/// (`exp(−α·Σd) = Π exp(−α·dᵢⱼ)`), mirroring E9's per-edge resistance.
pub fn injection_attenuation(depth: f32, alpha: Option<f32>) -> f32 {
    let alpha = alpha.unwrap_or(DEFAULT_INJECTION_ATTENUATION);
    (-alpha * depth).exp()
}

/// The scalar source amplitude a [`crate::HarmonicToken::KinematicVector`] injects.
///
/// The impulse contributes its momentum magnitude `m·v` (mass × scalar speed) as a
/// source amplitude (D6/D4). `direction` is reserved for the future Direction
/// grammar (lore: "terminal kinetic vector") and plays no part in the amplitude.
pub fn kinematic_source_amplitude(velocity: f32, mass: f32) -> f32 {
    mass * velocity
}

#[cfg(test)]
mod tests {
    use crate::{injection_attenuation, kinematic_source_amplitude};

    #[test]
    fn injection_attenuation_is_one_at_zero_depth() {
        assert_eq!(injection_attenuation(0.0, None), 1.0);
    }

    #[test]
    fn injection_attenuation_strictly_decreases_with_depth() {
        let mut previous_attenuation = 1.0;
        for depth in 1..10 {
            let attenuation = injection_attenuation(depth as f32, None);
            assert!(attenuation < previous_attenuation);
            previous_attenuation = attenuation;
        }
    }

    #[test]
    fn injection_attenuation_composes_multiplicatively() {
        let depth_one = 2.0;
        let depth_two = 3.0;
        let attenuation_one = injection_attenuation(depth_one, None);
        let attenuation_two = injection_attenuation(depth_two, None);
        let lhs = injection_attenuation(depth_one + depth_two, None);
        let rhs = attenuation_one * attenuation_two;
        assert!((lhs - rhs).abs() < 1e-6) // To be numerically safe
    }

    #[test]
    fn kinematic_source_amplitude_is_mass_times_velocity() {
        let mass = 3.0;
        let velocity = 2.0;
        assert_eq!(kinematic_source_amplitude(velocity, mass), mass * velocity);
    }
}
