//! # theos-magic
//!
//! M1 foundation of the **Harmonic Strings** magic system (THEOS-148).
//!
//! This crate establishes the stable seam that all downstream magic compilation
//! (M2 TensionGraph, M3 solving) and G2 AI prompt work build against, **before**
//! any of that machinery exists:
//!
//! - [`HarmonicToken`] — the universal token vocabulary every casting interface
//!   emits and the Core consumes.
//! - [`MagicInterface`] — the object-safe trait each castable modality (Runes,
//!   Martial Arts, Chanting, …) implements to parse its raw input into tokens.
//! - [`CompilerContext`] — the per-cast context (caster, world entropy, energy,
//!   and the caster's seven [`Chakra`] points) handed to a modality during parsing.
//!
//! Math uses [`glam`] directly — the engine's canonical, SIMD-accelerated math
//! vocabulary (decision 2026-06-12; `theos-math` is retired). See the plan in
//! `plans/THEOS-148.md`.
//!
//! **Scope:** the M1 seam plus the M2 [`graph`] layer (TensionGraph assembly and
//! E2 phase derivation). No solving (M3), no concrete modalities, and no gameplay
//! behaviour (attunement growth, polarity dynamics, capacity progression) yet.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod chakra;
pub mod context;
pub mod element;
pub mod error;
pub mod graph;
pub mod interface;
pub mod token;
pub mod tuning;

pub use chakra::{Attunement, Chakra, ChakraState, Chakras, Polarity};
pub use context::CompilerContext;
pub use element::{ElementField, ElementKind};
pub use error::CompilerError;
pub use graph::{
    Arena, CsrAdjacency, DEFAULT_INJECTION_ATTENUATION, DEFAULT_WAVE_SPEED, Edge, GraphAssembler,
    Node, NodeId, TemporalTransfer, TensionGraph, band_pass_response, clamp_tau_seconds,
    edge_delay, edge_phase_shift, filter_bank_response, injection_attenuation,
    kinematic_source_amplitude, sigma_from_duration,
};
pub use interface::MagicInterface;
pub use token::{HarmonicToken, TokenId, WovenToken};
pub use tuning::WeaveTuning;
