//! Components used across the AI layer.
//!
//! These are G2 seam component types (THEOS-147): they let AI-layer work
//! (ContextBuilder THEOS-68, ValidationGate THEOS-80, BehaviorTree runtime
//! THEOS-83) refer to AI-driven entities and their in-flight request state
//! before the owning systems exist. They are deliberately minimal — richer
//! structure (the behaviour-tree definition, latency/priority) is added by the
//! tickets that own it (THEOS-83 / THEOS-88 / THEOS-91).

use theos_shared::Component;

/// Marks an entity as AI-driven and names the behaviour that governs it.
///
/// An entity carrying an `AiBrain` is steered by the AI layer: it emits LLM
/// requests and falls back to a behaviour tree during inference (spec §10.1).
/// At the seam stage it carries only the behaviour identifier; the concrete
/// behaviour-tree definition is added by THEOS-83 / THEOS-88.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiBrain {
    /// Identifier of the AI behaviour / persona that governs this entity.
    pub behavior: String,
}

impl Component for AiBrain {}

/// Marker placed on an entity while an LLM request for it is in flight.
///
/// Its presence is the signal: the behaviour-tree runtime (THEOS-83) runs the
/// fallback while it is attached, and the request dispatcher uses it to avoid
/// firing a second request for the same entity. It is removed once the response
/// is applied at the next frame sync point. Deadline and priority fields belong
/// to the AI Latency Budget system (THEOS-91), not here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingAiResponse;

impl Component for PendingAiResponse {}
