//! The **TensionGraph** (M2): an oriented graph assembled from woven tokens, with
//! geometry-derived phase (E2).
//!
//! A spell is a graph of oscillator [`Node`]s joined by tensioned [`Edge`]s. The
//! [`GraphAssembler`] parses a slice of [`crate::WovenToken`]s into an immutable
//! [`TensionGraph`]: it builds nodes, picks the injection root, orients edges
//! outward from it (flagging cycle-inducing edges as back-edges), stores adjacency
//! in compressed-sparse-row form, and derives each node's phase from geometry. It
//! also folds `KinematicVector` impulses into depth-attenuated source amplitudes
//! (E11) and derives each node's output amplitude by phasor superposition (E4).
//!
//! See `plans/THEOS-149.md`, `plans/THEOS-158.md`, and the Equations of the Weave
//! (E1, E2, E4, E11).

mod arena;
mod assembler;
mod csr;
mod injection;
mod phase;

pub use arena::{Arena, NodeId};
pub use assembler::GraphAssembler;
pub use csr::{CsrAdjacency, Edge};
pub use injection::{
    DEFAULT_INJECTION_ATTENUATION, injection_attenuation, kinematic_source_amplitude,
};
pub use phase::{DEFAULT_WAVE_SPEED, edge_delay, edge_phase_shift};

use glam::Vec3;

/// A single oscillator node (E1 fields).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Node {
    /// World-space position `xᵢ`. For a `BodyAnchor`, this is the derived
    /// `body_position + chakra.body_offset()`.
    pub position: Vec3,
    /// Oscillation frequency `fᵢ` (Hz).
    pub frequency: f32,
    /// Authored oscillation amplitude `Aᵢ` (E1) — the node's intrinsic source
    /// amplitude, as emitted by its anchor token. Distinct from [`Self::output_amplitude`].
    pub amplitude: f32,
    /// Phase `φᵢ`, derived from geometry per E2. Read-only after assembly.
    pub phase: f32,
    /// Depth `D(n)` — the accumulated path length `Σ dᵢⱼ` from the injection root
    /// to this node along the oriented root→`n` path (`0.0` at the root). Derived
    /// from geometry during assembly, read-only after; this is the depth that
    /// scales an injected source amplitude via E11 (THEOS-158).
    pub depth: f32,
    /// Output amplitude `Aⱼ` (E4) — the magnitude of the phasor superposition of
    /// the contributions converging on this node, with depth-attenuated (E11)
    /// `KinematicVector` injections folded in at their source. Derived, read-only
    /// after assembly; `0.0` until the interference pass runs (THEOS-158).
    pub output_amplitude: f32,
}

/// An assembled, immutable tension graph.
///
/// Produced by [`GraphAssembler::assemble`]. It exposes only shared (`&`) access,
/// so derived quantities ([`Node::phase`], [`TensionGraph::critical_path_delay`])
/// are read-only once the graph exists — the value itself is the proof that
/// assembly ran (AC4).
#[derive(Clone, Debug)]
pub struct TensionGraph {
    pub(crate) arena: Arena<Node>,
    pub(crate) adjacency: CsrAdjacency,
    pub(crate) root: NodeId,
    pub(crate) critical_path_delay: f32,
}

impl TensionGraph {
    /// The injection-root node — the lowest canonical-index chakra among the body
    /// anchors.
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Borrow a node by handle, or `None` if the handle is stale.
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.arena.get(id)
    }

    /// The outgoing oriented edges of `id` as a CSR slice.
    pub fn neighbors(&self, id: NodeId) -> &[Edge] {
        self.adjacency.neighbors(id)
    }

    /// The number of live nodes in the graph.
    pub fn node_count(&self) -> usize {
        self.arena.len()
    }

    /// Critical-path propagation delay: `max over leaves(Σ dᵢⱼ on root→leaf) / c`.
    ///
    /// Derived in the same pass as phase; a read-only substrate for cast-time
    /// (THEOS-156). Data only — no cast-time mechanic lives here.
    pub fn critical_path_delay(&self) -> f32 {
        self.critical_path_delay
    }
}
