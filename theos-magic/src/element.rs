//! The five elemental phases (Wu Xing) and ambient elemental fields.

use core::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

/// One of the five elemental phases (Wu Xing).
///
/// The generating/controlling cycles between elements are intentionally left to
/// M2 (the TensionGraph), which is where element interactions become mechanics.
/// The discriminant doubles as the storage index (see [`ElementKind::index`] and
/// [`ElementField`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementKind {
    /// 木 — Wood.
    Wood,
    /// 火 — Fire.
    Fire,
    /// 土 — Earth.
    Earth,
    /// 金 — Metal.
    Metal,
    /// 水 — Water.
    Water,
}

impl ElementKind {
    /// The number of elemental phases.
    pub const COUNT: usize = 5;

    /// All elements in canonical (Wu Xing generating-cycle) order.
    pub const ALL: [ElementKind; Self::COUNT] = [
        ElementKind::Wood,
        ElementKind::Fire,
        ElementKind::Earth,
        ElementKind::Metal,
        ElementKind::Water,
    ];

    /// This element's index into an [`ElementField`] (its position in [`ElementKind::ALL`]).
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// An ambient per-element intensity field.
///
/// Models the elemental "mix" of an environment (or any per-element weighting):
/// each [`ElementKind`] carries a scalar intensity. A modality reads this during
/// compilation to strengthen tokens whose element or frequency matches the
/// locale — e.g. Fire weaving near a volcano. How intensities bias output is M2+
/// logic; the seam only fixes the shape.
#[derive(Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct ElementField([f32; ElementKind::COUNT]);

impl ElementField {
    /// Build an [`ElementField`] from an explicit array in [`ElementKind::ALL`] order.
    pub const fn from_array(intensities: [f32; ElementKind::COUNT]) -> Self {
        Self(intensities)
    }

    /// An [`ElementField`] with the same intensity for every element.
    pub const fn uniform(intensity: f32) -> Self {
        Self([intensity; ElementKind::COUNT])
    }

    /// Iterate over every element paired with its intensity, in canonical order.
    pub fn iter(&self) -> impl Iterator<Item = (ElementKind, f32)> {
        ElementKind::ALL.into_iter().zip(self.0.iter().copied())
    }
}

impl Index<ElementKind> for ElementField {
    type Output = f32;

    fn index(&self, element: ElementKind) -> &f32 {
        &self.0[element.index()]
    }
}

impl IndexMut<ElementKind> for ElementField {
    fn index_mut(&mut self, element: ElementKind) -> &mut f32 {
        &mut self.0[element.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_all_length_matches_count() {
        assert_eq!(ElementKind::ALL.len(), ElementKind::COUNT);
    }

    #[test]
    fn element_index_matches_position_in_all() {
        for (position, element) in ElementKind::ALL.into_iter().enumerate() {
            assert_eq!(element.index(), position);
        }
    }

    #[test]
    fn default_field_is_all_zero() {
        let field = ElementField::default();
        assert!(field.iter().all(|(_, intensity)| intensity == 0.0));
    }

    #[test]
    fn uniform_sets_every_element() {
        let field = ElementField::uniform(0.5);
        assert!(field.iter().all(|(_, intensity)| intensity == 0.5));
    }

    #[test]
    fn indexing_reads_and_writes_per_element() {
        let mut field = ElementField::default();
        field[ElementKind::Fire] = 2.0;
        assert_eq!(field[ElementKind::Fire], 2.0);
        assert_eq!(field[ElementKind::Water], 0.0);
    }

    #[test]
    fn element_field_round_trips_through_json() {
        let field = ElementField::from_array([0.1, 0.2, 0.3, 0.4, 0.5]);
        let json = serde_json::to_string(&field).unwrap();
        let back: ElementField = serde_json::from_str(&json).unwrap();
        assert_eq!(field, back);
    }
}
