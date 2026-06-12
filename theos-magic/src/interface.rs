//! The [`MagicInterface`] trait — the plug-in contract for casting modalities.

use crate::context::CompilerContext;
use crate::error::CompilerError;
use crate::token::HarmonicToken;

/// A castable magic interface (a *modality*: Runes, Martial Arts, Chanting, …).
///
/// Each modality parses its own raw input format into the universal
/// [`HarmonicToken`] stream; the Core then consumes those tokens blindly and runs
/// all physics. The trait is deliberately **object-safe** so modalities can be
/// held and dispatched as `dyn MagicInterface`.
///
/// No implementations live in this crate at the M1 seam stage — concrete
/// modalities are future modality work.
pub trait MagicInterface {
    /// A short, stable, human-readable name for this modality (e.g. `"martial-arts"`).
    fn modality_name(&self) -> &'static str;

    /// Parse `raw` modality input into harmonic tokens, appending them to `out`.
    ///
    /// `ctx` supplies the caster/world state the modality may read while parsing.
    /// Returns [`CompilerError`] if the input cannot be parsed.
    fn parse_input(
        &self,
        raw: &[u8],
        ctx: &CompilerContext,
        out: &mut Vec<HarmonicToken>,
    ) -> Result<(), CompilerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyModality;

    impl MagicInterface for DummyModality {
        fn modality_name(&self) -> &'static str {
            "dummy"
        }

        fn parse_input(
            &self,
            _raw: &[u8],
            _ctx: &CompilerContext,
            _out: &mut Vec<HarmonicToken>,
        ) -> Result<(), CompilerError> {
            Ok(())
        }
    }

    #[test]
    fn magic_interface_dispatches_as_a_trait_object() {
        // Coercing to `&dyn` proves object-safety; calling through it exercises dispatch.
        let modality: &dyn MagicInterface = &DummyModality;
        assert_eq!(modality.modality_name(), "dummy");

        let ctx = CompilerContext::default();
        let mut out = Vec::new();
        assert!(modality.parse_input(b"", &ctx, &mut out).is_ok());
        assert!(out.is_empty());
    }
}
