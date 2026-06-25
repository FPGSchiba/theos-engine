//! Compressed-sparse-row (CSR) adjacency (AC3).
//!
//! Edges are stored oriented (outward from the root) in one flat `edges` vector,
//! with an `offsets` index marking where each node's slice begins. Back-edges are
//! kept and flagged rather than dropped — the M3 stability solver needs the cycle
//! structure (E6 loop gain).

use super::arena::NodeId;

/// A single oriented edge in an assembled graph.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Edge {
    /// The node this edge points at (oriented outward from the root).
    pub target: NodeId,
    /// `true` if orienting this edge re-entered the traversal tree — i.e. it
    /// induces a cycle. Flagged, not removed.
    pub is_back_edge: bool,
    /// Tension coefficient `kᵢⱼ` (E3/E6) — string stiffness. Defaults to
    /// `WeaveTuning::stiffness` at assembly until a token supplies its own (D1/D2).
    pub stiffness: f32,
    /// Natural (rest) length `Lᵢⱼ` (E3). Defaults to the edge's spatial length
    /// `dᵢⱼ` at assembly ⇒ tension `T ≈ 0` at rest until a token stretches it (D2).
    pub rest_length: f32,
    /// Frequency tolerance `σ_f` (E5) — Gaussian coherence width. Defaults to
    /// `WeaveTuning::sigma_f`; provenance (authentic vs synthetic) deferred (D3).
    pub sigma_f: f32,
}

/// Adjacency stored in compressed-sparse-row form.
///
/// `offsets` has length `node_count + 1`; the outgoing edges of the node at slot
/// index `n` are `edges[offsets[n]..offsets[n + 1]]`.
#[derive(Clone, Debug, Default)]
pub struct CsrAdjacency {
    pub(crate) offsets: Vec<u32>,
    pub(crate) edges: Vec<Edge>,
}

impl CsrAdjacency {
    /// The outgoing oriented edges of `node` as a contiguous slice.
    pub fn neighbors(&self, node: NodeId) -> &[Edge] {
        let start = self.offsets[node.index() as usize] as usize;
        let end = self.offsets[node.index() as usize + 1] as usize;
        &self.edges[start..end]
    }

    /// The total number of edges stored (including flagged back-edges).
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Arena;

    /// A 3-node CSR: node 0 → {1, 2}, node 1 is a sink, node 2 → {0} (back-edge).
    fn sample() -> (CsrAdjacency, NodeId, NodeId, NodeId) {
        let mut arena: Arena<()> = Arena::new();
        let (n0, n1, n2) = (arena.insert(()), arena.insert(()), arena.insert(()));
        let csr = CsrAdjacency {
            offsets: vec![0, 2, 2, 3],
            edges: vec![
                Edge {
                    target: n1,
                    is_back_edge: false,
                    stiffness: 1.0,
                    rest_length: 1.0,
                    sigma_f: 1.0,
                },
                Edge {
                    target: n2,
                    is_back_edge: false,
                    stiffness: 1.0,
                    rest_length: 1.0,
                    sigma_f: 1.0,
                },
                Edge {
                    target: n0,
                    is_back_edge: true,
                    stiffness: 1.0,
                    rest_length: 1.0,
                    sigma_f: 1.0,
                },
            ],
        };
        (csr, n0, n1, n2)
    }

    #[test]
    fn neighbors_returns_a_nodes_outgoing_edges() {
        let (csr, n0, n1, n2) = sample();
        let out = csr.neighbors(n0);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].target, n1);
        assert_eq!(out[1].target, n2);
        assert_eq!(csr.edge_count(), 3);
    }

    #[test]
    fn neighbors_is_empty_for_a_sink_node() {
        let (csr, _n0, n1, _n2) = sample();
        assert!(csr.neighbors(n1).is_empty());
    }

    #[test]
    fn offsets_length_is_node_count_plus_one() {
        let (csr, _n0, _n1, n2) = sample();
        // 3 nodes ⇒ 4 offsets, and the back-edge survives in the flat array.
        assert_eq!(csr.offsets.len(), 4);
        assert!(csr.neighbors(n2)[0].is_back_edge);
    }
}
