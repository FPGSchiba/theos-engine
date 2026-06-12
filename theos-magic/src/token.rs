//! The [`HarmonicToken`] vocabulary — the universal seam between casting
//! interfaces (which produce tokens) and the Core (which consumes them blindly).
//!
//! Tokens are intentionally interface-agnostic: a `KinematicVector` means the same
//! thing whether it came from a Martial-Arts strike or a Rune. Positions and
//! directions use [`glam::Vec3`] (scalar storage — predictable serialized layout;
//! `Vec3A` is reserved for hot compute paths that do not exist at the seam stage).

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// A single instruction in the harmonic token stream a modality emits.
///
/// Each variant carries the minimal payload the Core needs to place a node or
/// edge into the TensionGraph (M2). The token set is closed at the Core level;
/// interfaces map their domain concepts onto these variants.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum HarmonicToken {
    /// A fixed point in space that anchors part of the weave.
    SpatialAnchor {
        /// World-space position of the anchor.
        position: Vec3,
        /// Oscillation frequency carried by the anchor.
        frequency: f32,
        /// Oscillation amplitude carried by the anchor.
        amplitude: f32,
    },
    /// A directed motion contribution (the primary Martial-Arts token).
    KinematicVector {
        /// Direction of the motion (not required to be normalized at the seam).
        direction: Vec3,
        /// Scalar speed along `direction`.
        velocity: f32,
        /// Mass the motion applies to, scaling its momentum.
        mass: f32,
    },
    /// A pure temporal oscillation with no spatial component.
    TemporalFrequency {
        /// Oscillation frequency in hertz.
        frequency_hz: f32,
        /// Duration of the oscillation in milliseconds.
        duration_ms: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spatial_anchor_round_trips_through_json() {
        let token = HarmonicToken::SpatialAnchor {
            position: Vec3::new(1.0, 2.0, 3.0),
            frequency: 440.0,
            amplitude: 0.8,
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: HarmonicToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
    }

    #[test]
    fn kinematic_vector_round_trips_through_json() {
        let token = HarmonicToken::KinematicVector {
            direction: Vec3::new(0.0, 1.0, 0.0),
            velocity: 12.5,
            mass: 70.0,
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: HarmonicToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
    }

    #[test]
    fn temporal_frequency_round_trips_through_json() {
        let token = HarmonicToken::TemporalFrequency {
            frequency_hz: 7.83,
            duration_ms: 1500,
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: HarmonicToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
    }
}
