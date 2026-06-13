//! The [`HarmonicToken`] vocabulary — the universal seam between casting
//! interfaces (which produce tokens) and the Core (which consumes them blindly).
//!
//! Tokens are intentionally interface-agnostic: a `KinematicVector` means the same
//! thing whether it came from a Martial-Arts strike or a Rune. Positions and
//! directions use [`glam::Vec3`] (scalar storage — predictable serialized layout;
//! `Vec3A` is reserved for hot compute paths that do not exist at the seam stage).

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::chakra::Chakra;

/// A single instruction in the harmonic token stream a modality emits.
///
/// Each variant carries the minimal payload the Core needs during TensionGraph
/// assembly (M2). Materialization is **provenance-dependent**, *not* one-node-per-
/// token: spatial-provenance variants ([`Self::SpatialAnchor`], [`Self::BodyAnchor`])
/// become [`crate::Node`]s; [`Self::TemporalFrequency`] becomes a node only once
/// timing→position synthesis is defined (deferred); [`Self::KinematicVector`] is
/// **never** a node — it is an impulse injected into an existing rooted node. The
/// token set is closed at the Core level; interfaces map their domain concepts onto
/// these variants.
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
    ///
    /// **Not a node.** A `KinematicVector` has no position provenance — it is an
    /// impulse (direction/velocity/mass) injected *into* an existing rooted node
    /// via its links, never materialized as an oscillator node in any milestone.
    KinematicVector {
        /// Direction of the motion (not required to be normalized at the seam).
        direction: Vec3,
        /// Scalar speed along `direction`.
        velocity: f32,
        /// Mass the motion applies to, scaling its momentum.
        mass: f32,
    },
    /// A pure temporal oscillation with no spatial component.
    ///
    /// **Temporal provenance.** Carries delay (`duration_ms`/ordering), not
    /// coordinates; a node position is *synthesized* from timing relative to the
    /// caster transform so that `d = delay·c`, reproducing the exact E2 phase.
    /// Materialization is **deferred** to its own story (blocked on the synthesis
    /// rule + `WorldAccess`/THEOS-147).
    TemporalFrequency {
        /// Oscillation frequency in hertz.
        frequency_hz: f32,
        /// Duration of the oscillation in milliseconds.
        duration_ms: u32,
    },
    /// An anchor bound to one of the caster's chakra points.
    ///
    /// Unlike [`HarmonicToken::SpatialAnchor`] (an arbitrary world point), a
    /// `BodyAnchor`'s node position is **derived**, not authored:
    /// `body_position + chakra.body_offset()` (see [`crate::Chakra::body_offset`]).
    /// The `chakra` also drives injection-root selection during assembly.
    BodyAnchor {
        /// Which chakra point this anchor is bound to.
        chakra: Chakra,
        /// World-space position of the caster's body origin.
        body_position: Vec3,
        /// Oscillation frequency carried by the anchor.
        frequency: f32,
        /// Oscillation amplitude carried by the anchor.
        amplitude: f32,
    },
}

/// A stable identity for a token within a single woven stream.
///
/// Assigned by the emitting interface; [`WovenToken::links`] reference other tokens
/// by this id. Robust to emit order, unlike a bare slice index.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
pub struct TokenId(pub u32);

impl TokenId {
    /// The first id in a fresh stream.
    pub const ZERO: TokenId = TokenId(0);

    /// The next id in sequence — a small helper for vending a contiguous id space.
    pub const fn next(self) -> TokenId {
        TokenId(self.0 + 1)
    }
}

/// A [`HarmonicToken`] plus the topology the emitting interface declares around it.
///
/// `HarmonicToken` stays the pure, `Copy` payload seam; the wiring lives here.
/// `links` is **undirected** adjacency — a link is a mutual tension, and the
/// assembler derives edge *direction* by orienting outward from the injection root.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WovenToken {
    /// This token's stable identity within the stream.
    pub id: TokenId,
    /// The payload instruction.
    pub token: HarmonicToken,
    /// Ids of the tokens this one is tensioned to (undirected).
    pub links: Vec<TokenId>,
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

    #[test]
    fn body_anchor_round_trips_through_json() {
        let token = HarmonicToken::BodyAnchor {
            chakra: Chakra::Heart,
            body_position: Vec3::new(0.0, 0.0, 1.0),
            frequency: 220.0,
            amplitude: 0.5,
        };
        let json = serde_json::to_string(&token).unwrap();
        let back: HarmonicToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token, back);
    }

    #[test]
    fn token_id_next_increments_by_one() {
        assert_eq!(TokenId::ZERO.next(), TokenId(1));
    }

    #[test]
    fn woven_token_round_trips_through_json() {
        let woven = WovenToken {
            id: TokenId(0),
            token: HarmonicToken::TemporalFrequency {
                frequency_hz: 1.0,
                duration_ms: 10,
            },
            links: vec![TokenId(1), TokenId(2)],
        };
        let json = serde_json::to_string(&woven).unwrap();
        let back: WovenToken = serde_json::from_str(&json).unwrap();
        assert_eq!(woven, back);
    }
}
