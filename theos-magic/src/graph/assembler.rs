//! Graph assembly (AC2): woven tokens → an oriented, immutable [`TensionGraph`].

use super::TensionGraph;
use super::phase::DEFAULT_WAVE_SPEED;
use crate::error::CompilerError;
use crate::token::WovenToken;
use crate::{
    Arena, CsrAdjacency, Edge, HarmonicToken, Node, NodeId, TokenId, edge_delay, edge_phase_shift,
};
use std::collections::{HashSet, VecDeque};

/// Assembles a slice of [`WovenToken`]s into a [`TensionGraph`].
///
/// The assembly pipeline:
/// 1. **Materialize nodes by provenance.** Not every token becomes a
///    [`super::Node`] — materialization follows where a token derives its
///    *position* from. E2 derives phase from *delay* (`τ = d/c`); distance is
///    merely how a spatial token encodes that delay, so a positionless token is
///    not phase-less, it has a different position *provenance*:
///    - `SpatialAnchor` — **spatial provenance**: position authored directly ⇒
///      node.
///    - `BodyAnchor` — **spatial provenance**: position *derived* as
///      `body_position + chakra.body_offset()` ⇒ node; its `chakra` also drives
///      injection-root selection (lowest canonical index).
///    - `KinematicVector` — **no provenance**: an impulse (direction/amplitude/
///      mass) injected *into* an existing rooted node, not an oscillator ⇒
///      **never** materialized as a node, in any milestone.
///    - `TemporalFrequency` — **temporal provenance**: carries delay (duration/
///      ordering), not coordinates; a node position is *synthesized* from timing
///      relative to the caster transform so that `d = delay·c`, reproducing the
///      exact E2 phase ⇒ node *once synthesis is defined* (deferred to its own
///      story).
///
///    THEOS-149 ships exactly the two spatial-provenance paths.
/// 2. Choose the injection root: the lowest canonical-index chakra among the body
///    anchors (errors with [`CompilerError::NoInjectionRoot`] if there is none).
/// 3. Traverse from the root, orienting each undirected link outward and flagging
///    any link that re-enters the traversal tree as a back-edge.
/// 4. Build CSR adjacency and derive phase + critical-path delay (E2).
pub struct GraphAssembler;

impl GraphAssembler {
    /// Assemble at an explicit wave speed `c`.
    ///
    /// Returns a [`CompilerError`] if the stream has no injection root, a link
    /// references an unknown token, or two tokens share an id.
    pub fn assemble(tokens: &[WovenToken], wave_speed: f32) -> Result<TensionGraph, CompilerError> {
        // Stage 0 — Index & validate. Establish each token's identity so links can
        // be resolved later; a repeated id is a `DuplicateTokenId`.
        if tokens.is_empty() {
            return Err(CompilerError::NoTokens);
        }

        if let Some(id) = tokens
            .iter()
            .find(|token| tokens.iter().filter(|t| t.id == token.id).count() > 1)
            .map(|token| token.id)
        {
            return Err(CompilerError::DuplicateTokenId(id));
        }

        let max_token_id = tokens
            .iter()
            .max_by(|a, b| a.id.0.cmp(&b.id.0))
            .map(|token| token.id)
            .unwrap_or(TokenId::ZERO);
        if max_token_id.0 != tokens.len() as u32 - 1 {
            return Err(CompilerError::NonDenseTokenIds);
        }

        // Stage 1 — Materialize nodes by provenance (spatial only this story).
        // `SpatialAnchor` keeps its position; `BodyAnchor` derives it via
        // `body_position + chakra.body_offset()`. `KinematicVector` and
        // `TemporalFrequency` are not nodes here.
        let mut arena: Arena<Node> = Arena::new();
        let mut token_map: Vec<Option<NodeId>> = vec![None; tokens.len()];

        for token in tokens {
            match token.token {
                HarmonicToken::BodyAnchor {
                    chakra,
                    body_position,
                    frequency,
                    amplitude,
                } => {
                    let position = body_position + chakra.body_offset();
                    let node = Node {
                        position,
                        frequency,
                        amplitude,
                        phase: 0.0, // placeholder; derived in Stage 6
                    };
                    let node_id = arena.insert(node);
                    token_map[token.id.0 as usize] = Some(node_id);
                }
                HarmonicToken::SpatialAnchor {
                    position,
                    frequency,
                    amplitude,
                } => {
                    let node = Node {
                        position,
                        frequency,
                        amplitude,
                        phase: 0.0, // placeholder; derived in Stage 6
                    };
                    let node_id = arena.insert(node);
                    token_map[token.id.0 as usize] = Some(node_id);
                }
                _ => {} // Skip other tokens for now
            }
        }

        // Stage 2 — Choose the injection root: the body anchor with the lowest
        // `chakra.index()`. None present ⇒ `NoInjectionRoot`.
        let root = tokens
            .iter()
            .filter(|token| matches!(token.token, HarmonicToken::BodyAnchor { .. }))
            .min_by_key(|token| {
                if let HarmonicToken::BodyAnchor { chakra, .. } = token.token {
                    chakra.index()
                } else {
                    usize::MAX // non-body anchors sort last
                }
            })
            .ok_or(CompilerError::NoInjectionRoot)
            .and_then(|root_token| {
                token_map[root_token.id.0 as usize].ok_or(CompilerError::NoInjectionRoot)
            })?;

        // Stage 3 — Resolve links into undirected adjacency, keyed by node slot.
        // A link is a mutual tension, so each one is recorded in *both* endpoints'
        // neighbour lists. A link id absent from the stream is a `DanglingLink`.
        let mut neighbors: Vec<Vec<NodeId>> = vec![Vec::new(); arena.len()];

        for token in tokens {
            // Only node-bearing tokens contribute edges; non-node tokens are skipped.
            let Some(u) = token_map[token.id.0 as usize] else {
                continue;
            };
            for &link_id in &token.links {
                match token_map.get(link_id.0 as usize) {
                    // In range and a node: record the tension in both directions.
                    Some(&Some(v)) => {
                        neighbors[u.index() as usize].push(v);
                        neighbors[v.index() as usize].push(u);
                    }
                    // In range but a non-node token (e.g. KinematicVector): an
                    // injection binding, not a graph edge — skipped per provenance.
                    Some(&None) => {}
                    // Out of range: the id isn't in the stream at all.
                    None => {
                        return Err(CompilerError::DanglingLink {
                            from: token.id,
                            to: link_id,
                        });
                    }
                }
            }
        }

        // Pushing both directions (and a link possibly declared from both ends)
        // can duplicate a neighbour; sort by slot for a deterministic order, then
        // drop the adjacent duplicates.
        for row in &mut neighbors {
            row.sort_unstable_by_key(|n| n.index());
            row.dedup();
        }

        // Stage 4 — Orient from the root and flag back-edges. Tree edges point
        // outward from the root; a link reaching an already-seen node (other than
        // the one you arrived from) is a back-edge — kept and flagged. Must
        // terminate on cycles.
        let mut full_edges: Vec<Vec<Edge>> = vec![Vec::new(); arena.len()];
        let mut visited: Vec<bool> = vec![false; arena.len()];
        let mut queue: VecDeque<(NodeId, Option<NodeId>)> = VecDeque::new();
        let mut seen_backedges: HashSet<(u32, u32)> = HashSet::new();

        visited[root.index() as usize] = true;
        queue.push_back((root, None));

        while let Some((u, parent)) = queue.pop_front() {
            for v in neighbors[u.index() as usize].iter() {
                if Some(*v) != parent {
                    if !visited[v.index() as usize] {
                        visited[v.index() as usize] = true;
                        full_edges[u.index() as usize].push(Edge {
                            target: *v,
                            is_back_edge: false,
                        });
                        queue.push_back((*v, Some(u)));
                    } else {
                        let pair = (u.index().min(v.index()), u.index().max(v.index()));
                        if seen_backedges.insert(pair) {
                            full_edges[u.index() as usize].push(Edge {
                                target: *v,
                                is_back_edge: true,
                            });
                        }
                    }
                }
            }
        }

        // Stage 5 — Flatten the per-node oriented edges into CSR (offsets + edges).
        let mut edges: Vec<Edge> = Vec::new();
        let mut offsets: Vec<u32> = Vec::with_capacity(arena.len() + 1);
        let mut running_offset: u32 = 0;

        for nested_edges in full_edges {
            offsets.push(running_offset);
            running_offset += nested_edges.len() as u32;
            for edge in nested_edges {
                edges.push(edge);
            }
        }
        offsets.push(running_offset);

        let csr = CsrAdjacency { offsets, edges };

        // Stage 6 — Derive phase + critical-path delay (E2). Accumulate Δφ along the
        // tree edges from the root (root phase = 0); carry a running delay and keep
        // the maximum for `critical_path_delay`.
        let mut max_delay: f32 = 0.0;
        let mut queue: VecDeque<(NodeId, f32 /*phase so far*/, f32 /*delay so far*/)> =
            VecDeque::new();

        queue.push_back((root, 0.0, 0.0));

        while let Some((id_u, phase_u, delay_u)) = queue.pop_front() {
            let (pos_u, freq_u) = {
                // invariant: ids in the queue / CSR are always materialized nodes
                let u = arena
                    .get(id_u)
                    .expect("queued ids are valid during assembly");
                (u.position, u.frequency) // Vec3 + f32 are Copy
            };

            for edge in csr.neighbors(id_u) {
                if edge.is_back_edge {
                    continue;
                }
                // invariant: ids in the queue / CSR are always materialized nodes
                let pos_v = arena
                    .get(edge.target)
                    .expect("edge targets are valid")
                    .position;
                let d = (pos_u - pos_v).length();

                let phase_v = phase_u + edge_phase_shift(freq_u, d, wave_speed);
                let delay_v = delay_u + edge_delay(d, wave_speed);

                // invariant: ids in the queue / CSR are always materialized nodes
                arena
                    .get_mut(edge.target)
                    .expect("edge targets are valid")
                    .phase = phase_v;

                max_delay = max_delay.max(delay_v);
                queue.push_back((edge.target, phase_v, delay_v));
            }
        }

        // Stage 7 — Freeze: arena + CSR + root + critical_path_delay → TensionGraph.
        // The immutable graph is the proof assembly ran; only `&` accessors escape.
        Ok(TensionGraph {
            arena,
            adjacency: csr,
            root,
            critical_path_delay: max_delay,
        })
    }

    /// Assemble at [`DEFAULT_WAVE_SPEED`].
    pub fn assemble_default(tokens: &[WovenToken]) -> Result<TensionGraph, CompilerError> {
        Self::assemble(tokens, DEFAULT_WAVE_SPEED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Chakra;
    use glam::Vec3;
    use std::f32::consts::PI;

    fn body(id: u32, chakra: Chakra, body_position: Vec3, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::BodyAnchor {
                chakra,
                body_position,
                frequency: 1.0,
                amplitude: 1.0,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    fn spatial(id: u32, position: Vec3, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::SpatialAnchor {
                position,
                frequency: 1.0,
                amplitude: 1.0,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    fn approx(got: f32, expected: f32) {
        assert!(
            (got - expected).abs() < 1e-5,
            "expected {expected}, got {got}"
        );
    }

    #[test]
    fn errors_when_no_body_anchor_is_present() {
        let tokens = vec![spatial(0, Vec3::ZERO, vec![])];
        assert!(matches!(
            GraphAssembler::assemble(&tokens, 1.0),
            Err(CompilerError::NoInjectionRoot)
        ));
    }

    #[test]
    fn errors_on_empty_token_stream() {
        assert!(matches!(
            GraphAssembler::assemble(&[], 1.0),
            Err(CompilerError::NoTokens)
        ));
    }

    #[test]
    fn errors_on_duplicate_token_id() {
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![]),
            spatial(0, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ];
        assert!(matches!(
            GraphAssembler::assemble(&tokens, 1.0),
            Err(CompilerError::DuplicateTokenId(TokenId(0)))
        ));
    }

    #[test]
    fn errors_on_non_dense_token_ids() {
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![]),
            spatial(2, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ];
        assert!(matches!(
            GraphAssembler::assemble(&tokens, 1.0),
            Err(CompilerError::NonDenseTokenIds)
        ));
    }

    #[test]
    fn errors_on_link_to_unknown_token_id() {
        let tokens = vec![body(0, Chakra::Root, Vec3::ZERO, vec![5])];
        assert!(matches!(
            GraphAssembler::assemble(&tokens, 1.0),
            Err(CompilerError::DanglingLink {
                from: TokenId(0),
                to: TokenId(5),
            })
        ));
    }

    #[test]
    fn chooses_lowest_index_chakra_as_injection_root() {
        // Heart (index 3) is token 0; Root (index 0) is token 1 → Root must win.
        let tokens = vec![
            body(0, Chakra::Heart, Vec3::ZERO, vec![1]),
            body(1, Chakra::Root, Vec3::ZERO, vec![]),
        ];
        let g = GraphAssembler::assemble(&tokens, 1.0).unwrap();
        assert_eq!(g.root().index(), 1);
    }

    #[test]
    fn orients_edges_outward_from_the_root() {
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(&tokens, 1.0).unwrap();
        let root = g.root();
        let r = g.neighbors(root);
        assert_eq!(r.len(), 1);
        assert!(!r[0].is_back_edge);
        let n1 = r[0].target;
        let e1 = g.neighbors(n1);
        assert_eq!(e1.len(), 1);
        // The far node is a sink — the wave does not flow back toward the root.
        let n2 = e1[0].target;
        assert!(g.neighbors(n2).is_empty());
    }

    #[test]
    fn flags_cycle_inducing_edges_as_back_edges() {
        // Triangle 0-1-2-0: one edge must close the cycle as a back-edge.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(0.0, 1.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(&tokens, 1.0).unwrap();
        let back = g.adjacency.edges.iter().filter(|e| e.is_back_edge).count();
        assert_eq!(back, 1);
    }

    #[test]
    fn five_node_acyclic_phase_matches_manual_calc_within_tolerance() {
        // All edges unit length, all frequencies 1, c = 1 ⇒ Δφ = 2π per edge.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 3]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(1.0, 1.0, 0.0), vec![]),
            spatial(3, Vec3::new(0.0, 1.0, 0.0), vec![4]),
            spatial(4, Vec3::new(0.0, 2.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(&tokens, 1.0).unwrap();

        assert_eq!(g.node_count(), 5);
        assert_eq!(g.adjacency.offsets, vec![0, 2, 3, 3, 4, 4]);
        assert_eq!(g.adjacency.edges.len(), 4);
        assert!(g.adjacency.edges.iter().all(|e| !e.is_back_edge));

        let root = g.root();
        assert_eq!(root.index(), 0);
        approx(g.node(root).unwrap().phase, 0.0);

        let r = g.neighbors(root);
        assert_eq!(r.len(), 2);
        let (n1, n3) = (r[0].target, r[1].target);
        assert_eq!(n1.index(), 1);
        assert_eq!(n3.index(), 3);
        approx(g.node(n1).unwrap().phase, 2.0 * PI);
        approx(g.node(n3).unwrap().phase, 2.0 * PI);

        let n2 = g.neighbors(n1)[0].target;
        assert_eq!(n2.index(), 2);
        approx(g.node(n2).unwrap().phase, 4.0 * PI);

        let n4 = g.neighbors(n3)[0].target;
        assert_eq!(n4.index(), 4);
        approx(g.node(n4).unwrap().phase, 4.0 * PI);

        approx(g.critical_path_delay(), 2.0);
    }

    #[test]
    fn five_node_cyclic_terminates_and_phase_matches_within_tolerance() {
        // Same diamond plus a 2–4 link, closing a cycle.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 3]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(1.0, 1.0, 0.0), vec![4]),
            spatial(3, Vec3::new(0.0, 1.0, 0.0), vec![4]),
            spatial(4, Vec3::new(0.0, 2.0, 0.0), vec![]),
        ];
        // Returning at all proves the traversal terminates on the cycle.
        let g = GraphAssembler::assemble(&tokens, 1.0).unwrap();

        assert_eq!(g.node_count(), 5);
        assert_eq!(g.adjacency.offsets, vec![0, 2, 3, 4, 5, 5]);
        let back = g.adjacency.edges.iter().filter(|e| e.is_back_edge).count();
        assert_eq!(back, 1);

        // Phase follows the spanning tree only, so it's unchanged by the back-edge.
        let root = g.root();
        let r = g.neighbors(root);
        approx(g.node(r[0].target).unwrap().phase, 2.0 * PI);
        approx(g.node(r[1].target).unwrap().phase, 2.0 * PI);
        approx(g.critical_path_delay(), 2.0);
    }

    #[test]
    fn critical_path_delay_equals_longest_root_to_leaf_over_wave_speed() {
        // Long branch 0→1→2 (d = 4 then 3); short branch 0→3 (d = 1); c = 2.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 3]),
            spatial(1, Vec3::new(4.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(4.0, 3.0, 0.0), vec![]),
            spatial(3, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(&tokens, 2.0).unwrap();
        // Longest path = (4 + 3) / 2 = 3.5, beating the short branch's 0.5.
        approx(g.critical_path_delay(), 3.5);
    }

    #[test]
    fn assemble_default_uses_the_default_wave_speed() {
        let tokens = vec![body(0, Chakra::Root, Vec3::ZERO, vec![])];
        let g = GraphAssembler::assemble_default(&tokens).unwrap();
        assert_eq!(g.node_count(), 1);
        approx(g.critical_path_delay(), 0.0);
    }

    #[test]
    fn non_node_tokens_and_their_links_are_skipped() {
        // A KinematicVector never materializes; an anchor's link to it is an
        // injection binding — skipped, neither a graph edge nor a DanglingLink.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            WovenToken {
                id: TokenId(1),
                token: HarmonicToken::KinematicVector {
                    direction: Vec3::X,
                    velocity: 1.0,
                    mass: 1.0,
                },
                links: vec![],
            },
        ];
        let g = GraphAssembler::assemble(&tokens, 1.0).unwrap();
        assert_eq!(g.node_count(), 1); // only the anchor became a node
        assert!(g.neighbors(g.root()).is_empty()); // link to the kinematic was skipped
    }
}
