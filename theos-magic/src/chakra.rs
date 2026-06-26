//! The chakra model carried by every caster.
//!
//! Chakras are **universal** — every caster has the same seven points regardless
//! of casting method; the Martial-Arts interface simply leans on them hardest.
//! Each point has three independent pieces of state with distinct mutability:
//!
//! - **capacity** — how much energy the point holds; grows with cultivation.
//! - **attunement** — the point's *element* (one of the five [`ElementKind`]s) and
//!   how strongly it is attuned. Attuning is a slow process that grows `strength`;
//!   the element can only be **retuned** once the old `strength` decays to `0.0`.
//! - **polarity** — the point's *state* on the Yin↔Yang axis; dynamic runtime
//!   value that M2+ mutates (Yin and Yang are defined by being in flux).
//!
//! Gameplay behaviour (growth, retuning, polarity dynamics) is **not** implemented
//! at the seam — only the data shape is fixed here.

use core::ops::{Index, IndexMut};

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::element::ElementKind;

/// One of a caster's chakra points.
///
/// Backend names are placeholders (the lore may rename them); the canonical order
/// is Root → Crown, and the discriminant doubles as the storage index (see
/// [`Chakra::index`] and [`Chakras`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chakra {
    /// Base point.
    Root,
    /// Lower point.
    Sacral,
    /// Solar-plexus point.
    SolarPlexus,
    /// Central point.
    Heart,
    /// Upper-chest point.
    Throat,
    /// Brow point.
    Brow,
    /// Apex point.
    Crown,
}

impl Chakra {
    /// The number of chakra points every caster has.
    pub const COUNT: usize = 7;

    /// All chakra points in canonical (Root → Crown) order.
    pub const ALL: [Chakra; Self::COUNT] = [
        Chakra::Root,
        Chakra::Sacral,
        Chakra::SolarPlexus,
        Chakra::Heart,
        Chakra::Throat,
        Chakra::Brow,
        Chakra::Crown,
    ];

    /// This chakra's index into a [`Chakras`] table (its position in [`Chakra::ALL`]).
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Vertical spacing between adjacent chakra points along the body's z-axis, in
    /// world units. A single tunable constant; swap to per-chakra values later
    /// without touching call sites.
    pub const SPACING: f32 = 0.2;

    /// This chakra's offset from the body origin.
    ///
    /// Points stack along **z** in canonical order — Root lowest, Crown highest —
    /// so the offset is `(0, 0, index * SPACING)`. A `BodyAnchor`'s node position
    /// is `body_position + chakra.body_offset()`, making chakra positions implicit
    /// and reproducible rather than authored per anchor.
    pub fn body_offset(self) -> Vec3 {
        Vec3::new(0.0, 0.0, self.index() as f32 * Self::SPACING)
    }
}

/// A chakra's elemental attunement: which element, and how strongly.
///
/// `strength` is grown by the (slow) attunement process toward `1.0`. A point can
/// only be retuned to a different [`ElementKind`] once `strength` has decayed to
/// `0.0`; that decay/retune logic is M2+ and is not implemented here.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attunement {
    /// The element this point is attuned to.
    pub kind: ElementKind,
    /// Attunement strength, nominally in `0.0..=1.0`.
    pub strength: f32,
}

/// A chakra's Yin↔Yang polarity as a single signed value.
///
/// `-1.0` = full Yin, `0.0` = balanced, `+1.0` = full Yang. This is dynamic
/// runtime state; the seam only defines the field, and M2+ owns how it drifts,
/// oscillates, and biases output.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Polarity(pub f32);

impl Default for Polarity {
    fn default() -> Self {
        Polarity(0.0)
    }
}

/// The per-caster runtime state of a single chakra point.
///
/// The [`Default`] state is a newborn point: zero capacity, unattuned, balanced.
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ChakraState {
    /// Energy capacity of the point; grows with cultivation.
    pub capacity: f32,
    /// Channel Power `P` (E14) — the point's **conductance**, how freely it admits
    /// energy into the weave (energy per unit time). A *passive* flow property, not
    /// an act of forcing energy through; the flow-rate sibling of [`Self::capacity`]
    /// (a withstand limit). Grows with cultivation; `0.0` on a newborn point ⇒
    /// cannot channel. A spell's effective `P` is the bottleneck `min` over the
    /// chakras it anchors through.
    pub channel_power: f32,
    /// Elemental attunement, or `None` if the point is not yet attuned.
    pub attunement: Option<Attunement>,
    /// Dynamic Yin↔Yang polarity of the point.
    pub polarity: Polarity,
}

/// A caster's full set of seven chakra points.
///
/// Wraps `[ChakraState; 7]` so callers get both readable, type-safe access by
/// [`Chakra`] (`chakras[Chakra::Heart]`) **and** iteration for graph passes that
/// aggregate across all points (`chakras.iter()`), with the [`Chakra`] enum as the
/// single source of truth for the count.
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Chakras([ChakraState; Chakra::COUNT]);

impl Chakras {
    /// Build a [`Chakras`] from an explicit array in [`Chakra::ALL`] order.
    pub const fn from_array(states: [ChakraState; Chakra::COUNT]) -> Self {
        Self(states)
    }

    /// Iterate over every point paired with its [`Chakra`] key, in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (Chakra, &ChakraState)> {
        Chakra::ALL.into_iter().zip(self.0.iter())
    }

    /// Mutably iterate over every point paired with its [`Chakra`] key.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Chakra, &mut ChakraState)> {
        Chakra::ALL.into_iter().zip(self.0.iter_mut())
    }

    /// Shared access to the Root chakra.
    pub fn root(&self) -> &ChakraState {
        &self[Chakra::Root]
    }

    /// Mutable access to the Root chakra.
    pub fn root_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::Root]
    }

    /// Shared access to the Sacral chakra.
    pub fn sacral(&self) -> &ChakraState {
        &self[Chakra::Sacral]
    }

    /// Mutable access to the Sacral chakra.
    pub fn sacral_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::Sacral]
    }

    /// Shared access to the Solar-plexus chakra.
    pub fn solar_plexus(&self) -> &ChakraState {
        &self[Chakra::SolarPlexus]
    }

    /// Mutable access to the Solar-plexus chakra.
    pub fn solar_plexus_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::SolarPlexus]
    }

    /// Shared access to the Heart chakra.
    pub fn heart(&self) -> &ChakraState {
        &self[Chakra::Heart]
    }

    /// Mutable access to the Heart chakra.
    pub fn heart_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::Heart]
    }

    /// Shared access to the Throat chakra.
    pub fn throat(&self) -> &ChakraState {
        &self[Chakra::Throat]
    }

    /// Mutable access to the Throat chakra.
    pub fn throat_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::Throat]
    }

    /// Shared access to the Brow chakra.
    pub fn brow(&self) -> &ChakraState {
        &self[Chakra::Brow]
    }

    /// Mutable access to the Brow chakra.
    pub fn brow_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::Brow]
    }

    /// Shared access to the Crown chakra.
    pub fn crown(&self) -> &ChakraState {
        &self[Chakra::Crown]
    }

    /// Mutable access to the Crown chakra.
    pub fn crown_mut(&mut self) -> &mut ChakraState {
        &mut self[Chakra::Crown]
    }
}

impl Index<Chakra> for Chakras {
    type Output = ChakraState;

    fn index(&self, chakra: Chakra) -> &ChakraState {
        &self.0[chakra.index()]
    }
}

impl IndexMut<Chakra> for Chakras {
    fn index_mut(&mut self, chakra: Chakra) -> &mut ChakraState {
        &mut self.0[chakra.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chakra_all_length_matches_count() {
        assert_eq!(Chakra::ALL.len(), Chakra::COUNT);
    }

    #[test]
    fn chakra_index_matches_position_in_all() {
        for (position, chakra) in Chakra::ALL.into_iter().enumerate() {
            assert_eq!(chakra.index(), position);
        }
    }

    #[test]
    fn body_offset_stacks_along_z_by_canonical_index() {
        assert_eq!(Chakra::Root.body_offset(), Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(
            Chakra::Crown.body_offset(),
            Vec3::new(0.0, 0.0, 6.0 * Chakra::SPACING)
        );
    }

    #[test]
    fn chakras_default_is_unattuned_and_balanced() {
        let chakras = Chakras::default();
        for (_, state) in chakras.iter() {
            assert_eq!(state.capacity, 0.0);
            assert_eq!(state.attunement, None);
            assert_eq!(state.polarity, Polarity::default());
        }
    }

    #[test]
    fn indexing_and_iteration_observe_the_same_state() {
        let mut chakras = Chakras::default();
        chakras.heart_mut().capacity = 5.0;

        assert_eq!(chakras.heart().capacity, 5.0);
        let total: f32 = chakras.iter().map(|(_, state)| state.capacity).sum();
        assert_eq!(total, 5.0);
    }

    #[test]
    fn iter_mut_allows_mutation_of_every_point() {
        let mut chakras = Chakras::default();
        for (_, state) in chakras.iter_mut() {
            state.polarity = Polarity(1.0);
        }
        assert!(
            chakras
                .iter()
                .all(|(_, state)| state.polarity == Polarity(1.0))
        );
    }

    #[test]
    fn from_array_preserves_canonical_order() {
        let mut states = [ChakraState::default(); Chakra::COUNT];
        states[Chakra::Crown.index()].capacity = 9.0;

        let chakras = Chakras::from_array(states);
        assert_eq!(chakras.crown().capacity, 9.0);
    }

    #[test]
    fn chakra_state_round_trips_through_json() {
        let state = ChakraState {
            capacity: 3.0,
            channel_power: 1.5,
            attunement: Some(Attunement {
                kind: ElementKind::Fire,
                strength: 0.5,
            }),
            polarity: Polarity(0.25),
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: ChakraState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn named_accessors_reach_every_point() {
        let mut chakras = Chakras::default();
        chakras.root_mut().capacity = 1.0;
        chakras.sacral_mut().capacity = 2.0;
        chakras.solar_plexus_mut().capacity = 3.0;
        chakras.heart_mut().capacity = 4.0;
        chakras.throat_mut().capacity = 5.0;
        chakras.brow_mut().capacity = 6.0;
        chakras.crown_mut().capacity = 7.0;

        assert_eq!(chakras.root().capacity, 1.0);
        assert_eq!(chakras.sacral().capacity, 2.0);
        assert_eq!(chakras.solar_plexus().capacity, 3.0);
        assert_eq!(chakras.heart().capacity, 4.0);
        assert_eq!(chakras.throat().capacity, 5.0);
        assert_eq!(chakras.brow().capacity, 6.0);
        assert_eq!(chakras.crown().capacity, 7.0);
    }
}
