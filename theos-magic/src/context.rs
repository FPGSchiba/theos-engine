//! The [`CompilerContext`] handed to a [`crate::MagicInterface`] during parsing.

use theos_shared::EntityId;

use crate::chakra::Chakras;
use crate::element::ElementField;

/// Everything a casting modality needs to know about the caster and the world at
/// compile time, when turning raw input into [`crate::HarmonicToken`]s.
///
/// This is the read-side context for parsing; it does not own the TensionGraph or
/// any solver state (those are M2+). Serde is intentionally not derived here —
/// `EntityId` carries no serde impl, and persisting the context is not a seam
/// requirement.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CompilerContext {
    /// The entity performing the cast.
    pub caster: EntityId,
    /// Overall entropy of the surrounding world, influencing weave stability.
    pub world_entropy: f32,
    /// Ambient elemental intensities of the locale (see [`ElementField`]).
    ///
    /// Read during compilation to strengthen tokens whose element or frequency
    /// matches the surroundings — e.g. Fire weaving near a volcano.
    pub ambient_elements: ElementField,
    /// Energy currently available to spend on the cast.
    pub energy: f32,
    /// The caster's seven chakra points and their per-point state.
    pub chakras: Chakras,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chakra::Chakra;

    #[test]
    fn default_context_has_seven_chakras_zero_caster_and_empty_field() {
        let ctx = CompilerContext::default();
        assert_eq!(ctx.caster, EntityId::default());
        assert_eq!(ctx.chakras.iter().count(), Chakra::COUNT);
        assert!(
            ctx.ambient_elements
                .iter()
                .all(|(_, intensity)| intensity == 0.0)
        );
    }
}
