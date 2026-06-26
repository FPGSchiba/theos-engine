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
mod cast;
mod csr;
mod energy;
mod injection;
mod phase;
mod stability;
mod transfer;

pub use arena::{Arena, NodeId};
pub use assembler::GraphAssembler;
pub use cast::CannotChannel;
pub use csr::{CsrAdjacency, Edge};
pub use energy::EnvResistance;
pub use injection::{
    DEFAULT_INJECTION_ATTENUATION, injection_attenuation, kinematic_source_amplitude,
};
pub use phase::{DEFAULT_WAVE_SPEED, edge_delay, edge_phase_shift};
pub use stability::{Stability, edge_coherence, edge_tension, spectral_radius};
pub use transfer::{
    TemporalTransfer, band_pass_response, clamp_tau_seconds, filter_bank_response,
    sigma_from_duration,
};

use crate::Chakra;
use glam::Vec3;

/// The structural role a node plays in an assembled graph (D3, THEOS-159).
///
/// Derived at assembly from the oriented edge structure, read-only after. E10's
/// efficiency η sums output amplitudes over [`NodeKind::Effector`] nodes only.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A wave origin: no feeders (no incoming forward edge). Seeds its own authored
    /// amplitude `Aᵢ` (E1). Canon's "Anchor/Source".
    Source,
    /// An interior node: has both feeders and at least one downstream consumer.
    Router,
    /// A sink: has feeders but no outgoing forward edge. The E10 η numerator sums
    /// effector output amplitudes.
    Effector,
}

/// A single oscillator node (E1 fields).
///
/// Not `Copy`: a node owns a `Vec` of attached [`TemporalTransfer`]s (E12).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// World-space position `xᵢ`. For a `BodyAnchor`, this is the derived
    /// `body_position + chakra.body_offset()`.
    pub position: Vec3,
    /// Oscillation frequency `fᵢ` (Hz).
    pub frequency: f32,
    /// Authored oscillation amplitude `Aᵢ` (E1) — the node's intrinsic source
    /// amplitude, as emitted by its anchor token. Distinct from [`Self::output_amplitude`].
    pub amplitude: f32,
    /// The body [`Chakra`] this node anchors, or `None` for a non-body node (a
    /// `SpatialAnchor`). Set at materialization (Stage 1). The set of `Some` values
    /// across a graph is the chakras it "anchors through" — the conduits whose
    /// Channel Power `P` bounds the cast (E14, THEOS-173); also the per-chakra hook
    /// future stories (E16/E18 occupation) will read.
    pub chakra: Option<Chakra>,
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
    /// Structural role (E10) — derived at assembly from the oriented edge
    /// structure, read-only after. Selects whether this node feeds the η numerator.
    pub kind: NodeKind,
    /// Authoring-side energy work this node *emits* (E9, D1): `Σ over edges this
    /// node feeds of dᵢⱼ·Aᵢ`, with `Aᵢ` this node's `output_amplitude`. Derived at
    /// assembly, read-only after; `Σ` over all nodes equals
    /// [`TensionGraph::authoring_work`]. Read by [`TensionGraph::node_energy`].
    pub node_work: f32,
    /// Frequency-selective transfer elements (E12) attached here by
    /// `TemporalFrequency` tokens — a parallel filterbank that band-pass-filters and
    /// delays this node's *incoming* feeder contributions before the E4 sum
    /// (throughput-only). Empty on a plain positional node; stacking is allowed
    /// (Decision 2026-06-14, THEOS-157).
    pub transfers: Vec<TemporalTransfer>,
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
    /// Cast-time (E13) — geometry + temporal-transfer settling delay.
    /// See [`TensionGraph::cast_time`].
    pub(crate) cast_time: f32,
    /// Authoring-side energy work term `W = Σ dᵢⱼ·Aᵢ` (E9, D1) — the intrinsic
    /// geometry×amplitude part of the energy cost, frozen at assembly like
    /// [`Self::cast_time`]. See [`TensionGraph::authoring_work`].
    pub(crate) authoring_work: f32,
    /// Spectral radius `ρ(G)` (E6) of the coupling matrix — the intrinsic loop
    /// gain, frozen at assembly (geometry + stiffness + coherence are all
    /// compile-time). The execution-side stability `S(ω)` reads this; see
    /// [`TensionGraph::rho_g`] and [`TensionGraph::stability`]. (D4.)
    pub(crate) rho_g: f32,
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

    /// Cast-time (E13): the critical-path **settling** delay — the longest root→leaf
    /// path where each edge contributes its geometric delay (`τ = d/c`, E2) and each
    /// crossed node contributes its dominant-band temporal-transfer delay (E12), keyed
    /// at the feeder frequency of the crossing wave.
    ///
    /// Derived at assembly, read-only after — the same lifecycle as
    /// [`Self::critical_path_delay`] and [`Node::phase`]. Caster-independent and
    /// intrinsic to the locked graph; the energy-throughput floor (Option B,
    /// THEOS-173) is deliberately **not** folded in here.
    pub fn cast_time(&self) -> f32 {
        self.cast_time
    }

    /// Authoring-side energy work term `W = Σ dᵢⱼ·Aᵢ` (E9, D1) — the frozen,
    /// world-independent input to [`Self::energy_cost`]. `Aᵢ` is the emitting
    /// (upstream) endpoint's output amplitude; world-dependent `ρ`/`E_base` factors
    /// are applied at execution, not here (the same authoring/execution split as
    /// [`Self::cast_time`] vs the THEOS-173 floor).
    pub fn authoring_work(&self) -> f32 {
        self.authoring_work
    }

    /// Spectral radius `ρ(G)` (E6) — the frozen, world-independent loop gain that
    /// feeds the execution-side stability `S(ω)` ([`Self::stability`]). Geometry +
    /// stiffness + coherence are all compile-time, so this is frozen at assembly.
    pub fn rho_g(&self) -> f32 {
        self.rho_g
    }
}
