//! Graph assembly (AC2): woven tokens → an oriented, immutable [`TensionGraph`].

use super::TensionGraph;
use super::phase::DEFAULT_WAVE_SPEED;
use crate::error::CompilerError;
use crate::token::WovenToken;
use crate::{
    Arena, CompilerContext, CsrAdjacency, DEFAULT_INJECTION_ATTENUATION, Edge, HarmonicToken, Node,
    NodeId, NodeKind, TemporalTransfer, TokenId, edge_delay, edge_phase_shift,
    filter_bank_response, injection_attenuation, kinematic_source_amplitude, spectral_radius,
};
use std::collections::{HashSet, VecDeque};
use std::f32::consts::PI;

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
    /// `ctx` supplies engine tuning — notably `ctx.weave_tuning`, the E12
    /// temporal-filter knobs read when lowering `TemporalFrequency` tokens.
    ///
    /// Returns a [`CompilerError`] if the stream has no injection root, a link
    /// references an unknown token, or two tokens share an id.
    pub fn assemble(
        tokens: &[WovenToken],
        ctx: &CompilerContext,
        wave_speed: f32,
        injection_attenuation_alpha: f32,
    ) -> Result<TensionGraph, CompilerError> {
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
                        chakra: Some(chakra), // a body anchor is a conduit (E14, THEOS-173)
                        phase: 0.0,           // placeholder; derived in Stage 6
                        depth: 0.0, // placeholder; path length from root, derived in Stage 6
                        output_amplitude: 0.0, // placeholder; derived by the E4 pass (THEOS-158)
                        kind: NodeKind::Router, // placeholder; derived in Stage 8.5 (THEOS-159)
                        node_work: 0.0, // placeholder; derived in Stage 8 loop (THEOS-159)
                        transfers: Vec::new(), // attached in Stage 2b (E12, THEOS-157)
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
                        chakra: None, // a spatial anchor is not a body conduit (E14)
                        phase: 0.0,   // placeholder; derived in Stage 6
                        depth: 0.0,   // placeholder; path length from root, derived in Stage 6
                        output_amplitude: 0.0, // placeholder; derived by the E4 pass (THEOS-158)
                        kind: NodeKind::Router, // placeholder; derived in Stage 8.5 (THEOS-159)
                        node_work: 0.0, // placeholder; derived in Stage 8 loop (THEOS-159)
                        transfers: Vec::new(), // attached in Stage 2b (E12, THEOS-157)
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

        // Stage 2b — Attach temporal transfer elements (E12, THEOS-157). A
        // `TemporalFrequency` is NOT a node; it lowers to a band-pass + delay
        // (`TemporalTransfer`) attached to the positional node(s) its links resolve to
        // — or the root, if none resolve (mirroring the `KinematicVector` fallback in
        // Stage 7). Stacking is allowed: transfers accumulate on a node's `transfers`.
        for token in tokens {
            let HarmonicToken::TemporalFrequency {
                frequency_hz,
                duration_ms,
            } = token.token
            else {
                continue;
            };
            let transfer =
                TemporalTransfer::from_token(frequency_hz, duration_ms, &ctx.weave_tuning);
            let mut pushes = 0;
            for &link_id in &token.links {
                match token_map.get(link_id.0 as usize) {
                    // Resolves to a positional node: attach the transfer there.
                    Some(&Some(v)) => {
                        arena
                            .get_mut(v)
                            .expect("resolves to a node")
                            .transfers
                            .push(transfer);
                        pushes += 1;
                    }
                    // In range but a non-node token (another temporal, or a
                    // KinematicVector): nothing to attach a filter to — skipped.
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
            if pushes == 0 {
                arena
                    .get_mut(root)
                    .expect("root is a node")
                    .transfers
                    .push(transfer);
            }
        }

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
                            stiffness: ctx.weave_tuning.stiffness,
                            rest_length: arena
                                .get(*v)
                                .expect("v needs a rest")
                                .position
                                .distance(arena.get(u).expect("u needs a rest").position),
                            sigma_f: ctx.weave_tuning.sigma_f,
                        });
                        queue.push_back((*v, Some(u)));
                    } else {
                        let pair = (u.index().min(v.index()), u.index().max(v.index()));
                        if seen_backedges.insert(pair) {
                            full_edges[u.index() as usize].push(Edge {
                                target: *v,
                                is_back_edge: true,
                                stiffness: ctx.weave_tuning.stiffness,
                                rest_length: arena
                                    .get(*v)
                                    .expect("v needs a rest")
                                    .position
                                    .distance(arena.get(u).expect("u needs a rest").position),
                                sigma_f: ctx.weave_tuning.sigma_f,
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
        let mut max_cast_time: f32 = 0.0;
        let mut queue: VecDeque<(
            NodeId,
            f32, /*phase so far*/
            f32, /*delay so far*/
            f32, /* depth */
            f32, /* cast delay so far */
        )> = VecDeque::new();

        queue.push_back((root, 0.0, 0.0, 0.0, 0.0));

        while let Some((id_u, phase_u, delay_u, depth_u, cast_delay_u)) = queue.pop_front() {
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

                let geometric_delay = edge_delay(d, wave_speed);
                // E13 cast-time: charge `v`'s dominant-band filter delay, keyed at the
                // feeder frequency `freq_u` (mirrors the E4 throughput application).
                // invariant: ids in the queue / CSR are always materialized nodes
                let filter_delay = filter_bank_response(
                    &arena
                        .get(edge.target)
                        .expect("edge targets are valid")
                        .transfers,
                    freq_u,
                )
                .1;
                let phase_v = phase_u + edge_phase_shift(freq_u, d, wave_speed);
                let delay_v = delay_u + geometric_delay;
                let depth_v = depth_u + d;
                let cast_delay_v = cast_delay_u + geometric_delay + filter_delay;

                // invariant: ids in the queue / CSR are always materialized nodes
                arena
                    .get_mut(edge.target)
                    .expect("edge targets are valid")
                    .phase = phase_v;

                // invariant: ids in the queue / CSR are always materialized nodes
                arena
                    .get_mut(edge.target)
                    .expect("edge targets are valid")
                    .depth = depth_v;

                max_delay = max_delay.max(delay_v);
                max_cast_time = max_cast_time.max(cast_delay_v);
                queue.push_back((edge.target, phase_v, delay_v, depth_v, cast_delay_v));
            }
        }

        // Stage 7 — Resolve injections (part a). Each KinematicVector splits its
        // momentum `m·v` evenly across its resolved node targets — or the root, if
        // none resolve (D6) — and each share is attenuated by the target's E11
        // depth, then summed per node. Splitting (rather than full `m·v` per
        // target) conserves total injected momentum: links redistribute the
        // impulse, they never multiply it.
        let mut injected_source: Vec<f32> = vec![0.0; arena.len()];

        for token in tokens {
            let HarmonicToken::KinematicVector { velocity, mass, .. } = token.token else {
                continue;
            };

            let mut targets: Vec<NodeId> = Vec::new();
            for &link_id in &token.links {
                match token_map.get(link_id.0 as usize) {
                    Some(&Some(node_id)) => targets.push(node_id),
                    Some(&None) => {} // non-node token — deferred, contributes no target
                    None => {
                        return Err(CompilerError::DanglingLink {
                            from: token.id,
                            to: link_id,
                        });
                    }
                }
            }

            // A link-less (or all-deferred) impulse falls back to the root.
            if targets.is_empty() {
                targets.push(root);
            }

            let share = kinematic_source_amplitude(velocity, mass) / targets.len() as f32;
            for target in targets {
                // invariant: a resolved target is always a materialized node
                let depth = arena
                    .get(target)
                    .expect("resolved target is a live node")
                    .depth;
                injected_source[target.index() as usize] +=
                    share * injection_attenuation(depth, Some(injection_attenuation_alpha));
            }
        }

        // Stage 8 — E4 output amplitude: Aⱼ = |Σ over feeders Aₖ·e^{iφₖ}|. Each
        // converging wave is a phasor `(Aₖ·cosφₖ, Aₖ·sinφₖ)`; a node's output
        // amplitude is the magnitude of their sum. Every edge is oriented by depth —
        // its lower-`(depth, slot)` endpoint feeds the higher — which is always a
        // DAG, so this is a single forward pass with nothing to exclude. Directed
        // feedback (runaway) is *not* modelled here; it is E6/E7's spectral radius
        // (THEOS-150, which reads the same edges). A Source node (no feeders) seeds
        // its own authored amplitude `Aᵢ` (E1; lore: "a Source injects a wave");
        // every node also seeds any injection it received (Stage 7); a
        // Router/Effector takes the superposition of its feeders.
        let node_count = arena.len();
        // invariant: every id below comes from `token_map` or the CSR, and assembly
        // never removes a node, so every `arena.get(..)` here resolves (Stage 8
        // `expect`s rely on this).
        let node_ids: Vec<NodeId> = token_map.iter().flatten().copied().collect();

        // Orient every edge by `(depth, slot)`: the lower endpoint feeds the higher.
        // `downstream[f]` lists the nodes `f` feeds; `has_feeder` marks non-Sources.
        let mut downstream: Vec<Vec<NodeId>> = vec![Vec::new(); node_count];
        let mut has_feeder: Vec<bool> = vec![false; node_count];
        for &u in &node_ids {
            let depth_u = arena.get(u).expect("live node").depth;
            for edge in csr.neighbors(u) {
                let v = edge.target;
                let depth_v = arena.get(v).expect("live node").depth;
                let u_feeds_v = match depth_u.total_cmp(&depth_v) {
                    std::cmp::Ordering::Less => true,
                    std::cmp::Ordering::Greater => false,
                    std::cmp::Ordering::Equal => u.index() < v.index(),
                };
                let (feeder, consumer) = if u_feeds_v { (u, v) } else { (v, u) };
                downstream[feeder.index() as usize].push(consumer);
                has_feeder[consumer.index() as usize] = true;
            }
        }

        // Seed each node's phasor with its source — its injection, plus its authored
        // `Aᵢ` iff it is a Source — entered at its own phase `φᵢ`.
        let mut phasor: Vec<(f32, f32)> = vec![(0.0, 0.0); node_count];
        for &id in &node_ids {
            let slot = id.index() as usize;
            let node = arena.get(id).expect("live node");
            let authored = if has_feeder[slot] {
                0.0
            } else {
                node.amplitude
            };
            let source = injected_source[slot] + authored;
            phasor[slot] = (source * node.phase.cos(), source * node.phase.sin());
        }

        // Visit feeders before consumers (ascending depth, then slot) and push each
        // finalised node's `Aᵤ·e^{iφᵤ}` into the nodes it feeds.
        let mut order = node_ids;
        order.sort_by(|a, b| {
            let depth_a = arena.get(*a).expect("live node").depth;
            let depth_b = arena.get(*b).expect("live node").depth;
            depth_a
                .total_cmp(&depth_b)
                .then_with(|| a.index().cmp(&b.index()))
        });
        let mut authoring_work = 0.0;
        for id in order {
            let slot = id.index() as usize;
            let (re, im) = phasor[slot];
            let amplitude = (re * re + im * im).sqrt();
            let (phase, frequency) = {
                let node = arena.get_mut(id).expect("live node");
                node.output_amplitude = amplitude;
                (node.phase, node.frequency)
            };
            let pos_s = arena.get(id).expect("live node").position;
            if !has_feeder[slot] {
                arena.get_mut(id).expect("live node").kind = NodeKind::Source;
            }
            if downstream[slot].is_empty() {
                arena.get_mut(id).expect("live node").kind = NodeKind::Effector;
                continue;
            }
            let mut current_authoring_work = 0.0;

            for &consumer in &downstream[slot] {
                let c = consumer.index() as usize;
                let filter = filter_bank_response(
                    &arena.get(consumer).expect("live node").transfers,
                    frequency,
                );
                let eff_phase = phase + PI * 2.0 * frequency * filter.1;
                let push = (amplitude * eff_phase.cos(), amplitude * eff_phase.sin());
                phasor[c].0 += push.0 * filter.0;
                phasor[c].1 += push.1 * filter.0;
                let pos_c = arena.get(consumer).expect("live node").position;
                let distance = pos_c.distance(pos_s);
                current_authoring_work += amplitude * distance;
            }
            arena.get_mut(id).expect("live node").node_work = current_authoring_work;
            authoring_work += current_authoring_work;
        }

        // Stage 9 — Freeze: arena + CSR + root + critical_path_delay → TensionGraph.
        let mut graph = TensionGraph {
            arena,
            adjacency: csr,
            root,
            critical_path_delay: max_delay,
            cast_time: max_cast_time,
            authoring_work,
            rho_g: 0.0, // placeholder; frozen via spectral_radius (E6, THEOS-150)
        };
        graph.rho_g = spectral_radius(&graph, &ctx.weave_tuning);

        Ok(graph)
    }

    /// Assemble at [`DEFAULT_WAVE_SPEED`].
    pub fn assemble_default(tokens: &[WovenToken]) -> Result<TensionGraph, CompilerError> {
        Self::assemble(
            tokens,
            &CompilerContext::default(),
            DEFAULT_WAVE_SPEED,
            DEFAULT_INJECTION_ATTENUATION,
        )
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

    fn kinematic(id: u32, velocity: f32, mass: f32, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::KinematicVector {
                direction: Vec3::X,
                velocity,
                mass,
            },
            links: links.into_iter().map(TokenId).collect(),
        }
    }

    fn temporal(id: u32, frequency_hz: f32, duration_ms: u32, links: Vec<u32>) -> WovenToken {
        WovenToken {
            id: TokenId(id),
            token: HarmonicToken::TemporalFrequency {
                frequency_hz,
                duration_ms,
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
            GraphAssembler::assemble(
                &tokens,
                &CompilerContext::default(),
                1.0,
                DEFAULT_INJECTION_ATTENUATION
            ),
            Err(CompilerError::NoInjectionRoot)
        ));
    }

    #[test]
    fn errors_on_empty_token_stream() {
        assert!(matches!(
            GraphAssembler::assemble(
                &[],
                &CompilerContext::default(),
                1.0,
                DEFAULT_INJECTION_ATTENUATION
            ),
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
            GraphAssembler::assemble(
                &tokens,
                &CompilerContext::default(),
                1.0,
                DEFAULT_INJECTION_ATTENUATION
            ),
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
            GraphAssembler::assemble(
                &tokens,
                &CompilerContext::default(),
                1.0,
                DEFAULT_INJECTION_ATTENUATION
            ),
            Err(CompilerError::NonDenseTokenIds)
        ));
    }

    #[test]
    fn errors_on_link_to_unknown_token_id() {
        let tokens = vec![body(0, Chakra::Root, Vec3::ZERO, vec![5])];
        assert!(matches!(
            GraphAssembler::assemble(
                &tokens,
                &CompilerContext::default(),
                1.0,
                DEFAULT_INJECTION_ATTENUATION
            ),
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
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        assert_eq!(g.root().index(), 1);
    }

    #[test]
    fn orients_edges_outward_from_the_root() {
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(2.0, 0.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
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
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
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
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();

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
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();

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
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            2.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
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

    // --- THEOS-158: injection (part a) + E4 interference amplitude (part b) ---

    #[test]
    fn kinematic_vector_injects_source_amplitude_into_its_linked_node() {
        // A KinematicVector linking to the root adds its momentum m·v = 6 to that
        // node's source (depth 0 ⇒ no attenuation): output = authored 1 + 6 = 7.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![]),
            kinematic(1, 3.0, 2.0, vec![0]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        approx(g.node(g.root()).unwrap().output_amplitude, 7.0);
    }

    #[test]
    fn link_less_kinematic_vector_falls_back_to_the_root() {
        // No links ⇒ the impulse injects into the injection root (D6): same 7.0.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![]),
            kinematic(1, 3.0, 2.0, vec![]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        approx(g.node(g.root()).unwrap().output_amplitude, 7.0);
    }

    #[test]
    fn multiple_kinematic_vectors_on_one_node_sum_their_injection() {
        // Two impulses on the root sum: m·v = 6 and 4 ⇒ output = 1 + 10 = 11.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![]),
            kinematic(1, 3.0, 2.0, vec![0]),
            kinematic(2, 1.0, 4.0, vec![0]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        approx(g.node(g.root()).unwrap().output_amplitude, 11.0);
    }

    #[test]
    fn kinematic_vector_into_an_effector_is_allowed() {
        // An Effector (only incoming edges) may receive an injection — it is not an
        // error, and it raises that node's amplitude above the un-injected baseline.
        let base = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ];
        let mut with_injection = base.clone();
        with_injection.push(kinematic(2, 3.0, 2.0, vec![1]));

        let ctx = CompilerContext::default();
        let baseline =
            GraphAssembler::assemble(&base, &ctx, 1.0, DEFAULT_INJECTION_ATTENUATION).unwrap();
        let injected =
            GraphAssembler::assemble(&with_injection, &ctx, 1.0, DEFAULT_INJECTION_ATTENUATION)
                .unwrap();

        let effector = |g: &TensionGraph| g.neighbors(g.root())[0].target;
        let before = baseline.node(effector(&baseline)).unwrap().output_amplitude;
        let after = injected.node(effector(&injected)).unwrap().output_amplitude;
        assert!(
            after > before,
            "injecting into the effector should raise its amplitude: {after} !> {before}"
        );
    }

    #[test]
    fn injection_attenuates_with_depth_from_the_root() {
        // Equal impulses into a shallow node (depth 1) and a deep node (depth 3):
        // the deeper one is attenuated more by E11, so it ends with less amplitude.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            spatial(2, Vec3::new(3.0, 0.0, 0.0), vec![]),
            kinematic(3, 3.0, 2.0, vec![1]),
            kinematic(4, 3.0, 2.0, vec![2]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        let neighbors = g.neighbors(g.root());
        let shallow = g.node(neighbors[0].target).unwrap().output_amplitude;
        let deep = g.node(neighbors[1].target).unwrap().output_amplitude;
        assert!(
            shallow > deep,
            "deeper injection should attenuate more: shallow {shallow} vs deep {deep}"
        );
    }

    #[test]
    fn convergent_node_amplitude_adds_when_in_phase() {
        // Diamond: R feeds A and B (each d = 1 ⇒ φ ≡ 0 mod 2π), both feed C. The
        // two unit waves reach C in phase, so they add to 2.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![3]),
            spatial(2, Vec3::new(0.0, 1.0, 0.0), vec![3]),
            spatial(3, Vec3::new(1.0, 1.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        let root = g.root();
        let a = g.neighbors(root)[0].target;
        let c = g.neighbors(a)[0].target;
        approx(g.node(c).unwrap().output_amplitude, 2.0);
    }

    #[test]
    fn convergent_node_amplitude_cancels_when_anti_phase() {
        // B sits a half-wavelength closer than A (φ_B = π vs φ_A = 2π), so the two
        // unit waves reach C in anti-phase and cancel to 0.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![3]),
            spatial(2, Vec3::new(0.5, 0.0, 0.0), vec![3]),
            spatial(3, Vec3::new(1.0, 1.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        let root = g.root();
        let a = g.neighbors(root)[0].target;
        let c = g.neighbors(a)[0].target;
        approx(g.node(c).unwrap().output_amplitude, 0.0);
    }

    #[test]
    fn two_wave_interference_matches_the_closed_form_within_tolerance() {
        // φ_A = 2π (≡ 0), φ_B = π/2 ⇒ Δφ = π/2; closed form
        // √(A₁² + A₂² + 2·A₁·A₂·cos Δφ) = √(1 + 1 + 0) = √2.
        let tokens = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![3]),
            spatial(2, Vec3::new(0.25, 0.0, 0.0), vec![3]),
            spatial(3, Vec3::new(1.0, 1.0, 0.0), vec![]),
        ];
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        let root = g.root();
        let a = g.neighbors(root)[0].target;
        let c = g.neighbors(a)[0].target;
        approx(g.node(c).unwrap().output_amplitude, 2.0_f32.sqrt());
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
        let g = GraphAssembler::assemble(
            &tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap();
        assert_eq!(g.node_count(), 1); // only the anchor became a node
        assert!(g.neighbors(g.root()).is_empty()); // link to the kinematic was skipped
    }

    // --- THEOS-157: TemporalFrequency → transfer element (E12) ---

    fn assemble(tokens: &[WovenToken]) -> TensionGraph {
        GraphAssembler::assemble(
            tokens,
            &CompilerContext::default(),
            1.0,
            DEFAULT_INJECTION_ATTENUATION,
        )
        .unwrap()
    }

    #[test]
    fn temporal_frequency_attaches_a_transfer_to_its_linked_node() {
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(2, 1.0, 1000, vec![1]),
        ]);
        let anchor = g.neighbors(g.root())[0].target;
        let transfers = &g.node(anchor).unwrap().transfers;
        assert_eq!(transfers.len(), 1);
        approx(transfers[0].center_hz, 1.0);
    }

    #[test]
    fn link_less_temporal_frequency_attaches_to_the_root() {
        // No link resolves to a node ⇒ the transfer falls back to the injection root.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![]),
            temporal(1, 1.0, 1000, vec![]),
        ]);
        assert_eq!(g.node(g.root()).unwrap().transfers.len(), 1);
    }

    #[test]
    fn temporal_frequency_does_not_increase_node_count() {
        // Only the body + spatial anchors are nodes; the temporal token is not.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(2, 1.0, 1000, vec![1]),
        ]);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn stacked_temporal_transfers_accumulate_on_one_node() {
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(2, 1.0, 1000, vec![1]),
            temporal(3, 5.0, 1000, vec![1]),
        ]);
        let anchor = g.neighbors(g.root())[0].target;
        assert_eq!(g.node(anchor).unwrap().transfers.len(), 2);
    }

    #[test]
    fn in_band_feeder_passes_largely_unattenuated() {
        // The root (f = 1.0) feeds C; a transfer centred on 1.0 sits on C, so the
        // feeder is in-band (H ≈ 1) and C's output ≈ the unfiltered feeder amplitude.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(2, 1.0, 1000, vec![1]),
        ]);
        let c = g.neighbors(g.root())[0].target;
        approx(g.node(c).unwrap().output_amplitude, 1.0);
    }

    #[test]
    fn out_of_band_feeder_is_attenuated() {
        // Same graph, but the transfer is centred far from the feeder's 1.0 Hz, so the
        // feeder is out-of-band (H ≈ 0) and C's output collapses toward zero.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(2, 1000.0, 1000, vec![1]),
        ]);
        let c = g.neighbors(g.root())[0].target;
        let out = g.node(c).unwrap().output_amplitude;
        assert!(out < 0.01, "out-of-band feeder should be suppressed: {out}");
    }

    #[test]
    fn transfer_adds_delay_phase_to_a_crossing_feeder() {
        // Root (f = 1.0) feeds C, and a KinematicVector injects a fixed-phase source
        // into C. A transfer centred on 1.0 (H ≈ 1, no attenuation) with a
        // half-wavelength delay (τ = 0.5 s ⇒ Δφ = 2π·1·0.5 = π) flips the feeder's
        // contribution relative to the injection, so the convergent amplitude drops
        // sharply vs the no-transfer baseline — isolating the delay, not the band-pass.
        let base = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            kinematic(2, 2.0, 1.0, vec![1]),
        ];
        let mut delayed = base.clone();
        delayed.push(temporal(3, 1.0, 500, vec![1]));

        let g_base = assemble(&base);
        let g_delayed = assemble(&delayed);
        let base_out = g_base
            .node(g_base.neighbors(g_base.root())[0].target)
            .unwrap()
            .output_amplitude;
        let delayed_out = g_delayed
            .node(g_delayed.neighbors(g_delayed.root())[0].target)
            .unwrap()
            .output_amplitude;

        assert!(
            delayed_out < base_out - 1.0,
            "the π delay should sharply cut the convergent amplitude: base {base_out}, delayed {delayed_out}"
        );
    }

    // --- THEOS-156: cast-time (E13) ---

    /// The dominant-band delay a single `temporal(_, 1.0, 1000, _)` charges a crossing
    /// feeder under default tuning: `clamp(1000/1000, τ_min, τ_max) = 1.0 s`.
    fn unit_transfer_delay() -> f32 {
        TemporalTransfer::from_token(1.0, 1000, &CompilerContext::default().weave_tuning).delay_s
    }

    #[test]
    fn cast_time_equals_critical_path_delay_when_no_transfers_present() {
        // With no transfers anywhere, the filter term is 0 on every node, so cast-time
        // collapses to the purely geometric critical-path delay (the THEOS-149 diamond).
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 3]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(1.0, 1.0, 0.0), vec![]),
            spatial(3, Vec3::new(0.0, 1.0, 0.0), vec![4]),
            spatial(4, Vec3::new(0.0, 2.0, 0.0), vec![]),
        ]);
        approx(g.cast_time(), g.critical_path_delay());
        approx(g.cast_time(), 2.0);
    }

    #[test]
    fn cast_time_on_acyclic_graph_matches_manual_calc() {
        // Linear chain root→1→2 (each edge d = 1, c = 1) with a transfer on the leaf.
        // Manual: into 1 = 1 + 0; into 2 = (1) + 1 (edge) + 1.0 (leaf filter) = 3.0.
        // Geometry alone is 2.0, so the filter term contributes the extra second.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(2.0, 0.0, 0.0), vec![]),
            temporal(3, 1.0, 1000, vec![2]),
        ]);
        approx(g.critical_path_delay(), 2.0);
        approx(g.cast_time(), 2.0 + unit_transfer_delay());
    }

    #[test]
    fn cast_time_on_cyclic_graph_matches_manual_calc() {
        // The THEOS-149 cyclic diamond plus a transfer on node 2 (on the tree path
        // 0→1→2). The 2→4 link is a back-edge and is skipped in the delay pass, so the
        // filter is charged exactly once along 0→1→2: cast = 2.0 geometry + 1.0 filter.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 3]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![2]),
            spatial(2, Vec3::new(1.0, 1.0, 0.0), vec![4]),
            spatial(3, Vec3::new(0.0, 1.0, 0.0), vec![4]),
            spatial(4, Vec3::new(0.0, 2.0, 0.0), vec![]),
            temporal(5, 1.0, 1000, vec![2]),
        ]);
        approx(g.critical_path_delay(), 2.0);
        approx(g.cast_time(), 2.0 + unit_transfer_delay());
    }

    #[test]
    fn transfer_on_critical_path_extends_cast_time_beyond_geometric() {
        // The centerpiece (AC2): the same graph with and without a transfer on the
        // critical path. Geometry is identical; cast-time grows by exactly the
        // transfer's dominant-band delay.
        let base = vec![
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ];
        let mut with_transfer = base.clone();
        with_transfer.push(temporal(2, 1.0, 1000, vec![1]));

        let g_base = assemble(&base);
        let g_transfer = assemble(&with_transfer);

        approx(g_base.cast_time(), g_base.critical_path_delay());
        approx(
            g_transfer.critical_path_delay(),
            g_base.critical_path_delay(),
        );
        approx(
            g_transfer.cast_time() - g_base.cast_time(),
            unit_transfer_delay(),
        );
        assert!(
            g_transfer.cast_time() > g_base.cast_time(),
            "a transfer on the critical path must extend cast-time: {} !> {}",
            g_transfer.cast_time(),
            g_base.cast_time()
        );
    }

    #[test]
    fn transfer_off_critical_path_does_not_extend_cast_time() {
        // Long branch 0→1 (d = 5) sets the critical path. The short branch 0→2 (d = 1)
        // carries a transfer, but 1.0 + 1.0 = 2.0 stays well under the long branch's
        // 5.0, so cast-time still tracks the geometric critical path unchanged.
        let g = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1, 2]),
            spatial(1, Vec3::new(5.0, 0.0, 0.0), vec![]),
            spatial(2, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(3, 1.0, 1000, vec![2]),
        ]);
        approx(g.critical_path_delay(), 5.0);
        approx(g.cast_time(), 5.0);
    }

    #[test]
    fn root_transfer_does_not_contribute_to_cast_time() {
        // A link-less temporal falls back onto the root (D2). The root is never crossed
        // into, so its transfer adds nothing — cast-time matches the transfer-free graph.
        let plain = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
        ]);
        let root_transfer = assemble(&[
            body(0, Chakra::Root, Vec3::ZERO, vec![1]),
            spatial(1, Vec3::new(1.0, 0.0, 0.0), vec![]),
            temporal(2, 1.0, 1000, vec![]),
        ]);
        // The transfer really did land on the root...
        assert_eq!(
            root_transfer
                .node(root_transfer.root())
                .unwrap()
                .transfers
                .len(),
            1
        );
        // ...yet it leaves cast-time untouched.
        approx(root_transfer.cast_time(), plain.cast_time());
    }
}
