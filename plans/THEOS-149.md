# THEOS-149 — [M2][magic] TensionGraph CSR adjacency & assembly

**Jira:** [THEOS-149](https://firephoenixgames.atlassian.net/browse/THEOS-149) · Story · `M2` `magic` · 5 SP · Sprint *G1.5 B02* · Blocks M3 (THEOS-150), M7 (THEOS-153)
**Spawned follow-up:** [THEOS-156](https://firephoenixgames.atlassian.net/browse/THEOS-156) — cast-time / E11 (depends on this story)

> 🎯 **Goal:** Parse a slice of woven `HarmonicToken`s into an oriented `TensionGraph` backed by a hand-rolled generational arena and CSR adjacency, and derive each node's phase φ from geometry per **E2** — phase read-only after assembly.

Math source of truth: [Shared Formalism: Equations of the Weave](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/170164225) — relevant equations **E1** (node = position xᵢ, frequency fᵢ, amplitude Aᵢ, phase φᵢ) and **E2** (phase derivation):

```
τᵢⱼ = dᵢⱼ / c                    propagation delay along edge i→j
Δφᵢⱼ = 2π·fᵢ·dᵢⱼ / c             phase shift across the edge
φⱼ = φᵣ + Σ Δφ  (edges on root→j path)   phase accumulates from the injection root r
dᵢⱼ = |xⱼ − xᵢ|                  Euclidean edge length
```

---

## Locked decisions (design session 2026-06-13)

### Token layer (M1 seam, in `token.rs`)
- **D-T1 — Connectivity is interface-declared and undirected.** The casting interface (Runes, Martial Arts, …) says *which* tokens are tensioned together; the assembler derives *direction*. A link is a mutual tension, not an arrow.
- **D-T2 — Identity via explicit `TokenId(u32)`** (not slice index). A small helper vends/increments ids. Robust to emit order.
- **D-T3 — Wrapper struct owns the wiring.** `HarmonicToken` stays the pure, `Copy`, M1 payload (untouched). A new `WovenToken { id, token, links: Vec<TokenId> }` carries topology. `Copy` is lost only on the wrapper, where the cost belongs — the payload seam stays cheap and uncoupled from graph assembly.
- **D-T4 — New `BodyAnchor` variant.** `BodyAnchor { chakra: Chakra, body_position: Vec3, frequency: f32, amplitude: f32 }`. Distinct from `SpatialAnchor` (arbitrary world point). The node's effective position is **derived**, not authored:
  `node_position = body_position + chakra.body_offset()`.

### Chakra layer (`chakra.rs`)
- **D-C1 — `Chakra::body_offset() -> Vec3`**, an index-derived vertical stack: `(0, 0, index * CHAKRA_SPACING)`. Reuses the existing canonical Root→Crown ordering. `CHAKRA_SPACING` is a single tunable constant; swap to explicit per-chakra values later without touching call sites.
- Stacking axis is **z** (flag for engine-wide up-axis consistency; not resolved here).

### Token→node provenance (assembly materialization rule)
- **D-P1 — Materialization is provenance-dependent, not one-node-per-token.** A token becomes a `Node` based on where it derives its *position* from. The load-bearing idea: E2 derives phase from *delay* (`τ = d/c`); distance is just how a *spatial* token encodes that delay, so a positionless token is not phase-less — it has a different position **provenance**. The four-way rule:
  - **Spatial provenance ⇒ node.**
    - `SpatialAnchor` — position authored directly.
    - `BodyAnchor` — position **derived**: `body_position + chakra.body_offset()`; also drives injection-root selection (lowest chakra index). THEOS-149 ships exactly these two paths.
  - **No provenance ⇒ NOT a node.**
    - `KinematicVector` — an impulse (direction/velocity/mass) injected **into** an existing rooted node, not an oscillator. Never materialized, in any milestone.
  - **Temporal provenance ⇒ node, once synthesis is defined.**
    - `TemporalFrequency` — carries delay (duration/ordering), not coordinates; node position is **synthesized** from timing relative to the caster transform, laid so `d = delay·c` to reproduce the exact phase. **Deferred** (its own story).
- **D-P2 — Deferral: `TemporalFrequency` → node.** Blocked on the timing→position synthesis rule and `WorldAccess`/THEOS-147. Tracked as a follow-up story.
- **D-P3 — Deferral: `KinematicVector` injection.** Materializing the impulse as a per-node injection (and E4 interference amplitude `A_j`) is out of scope for THEOS-149; tracked as a follow-up story.

### Graph layer (`graph/`)
- **D-G1 — Hand-rolled generic `Arena<T>`** (`arena.rs`). `NodeId { index: u32, generation: u32 }`, private fields, `Copy`. `get`/`get_mut` return `None` on generation mismatch (AC1, ABA guard). Generic for isolated generational tests.
- **D-G2 — Typestate enforces phase read-only (AC4).** Mutable `GraphAssembler` builds; `assemble(...) -> TensionGraph` returns an immutable graph exposing only `&` accessors. The `TensionGraph` value *is* the proof assembly ran.
- **D-G3 — CSR adjacency** (`csr.rs`): `offsets: Vec<u32>` (len = nodes + 1) + `edges: Vec<Edge>` where `Edge { target: NodeId, is_back_edge: bool }`. Oriented; back-edges are **flagged, not removed** (M3 solver needs the cycle structure for E6 loop gain).
- **D-G4 — Root = lowest canonical-index chakra among `BodyAnchor`s.** No new ordering infrastructure — uses `Chakra::index()`. No `BodyAnchor` ⇒ `NoInjectionRoot` error.
- **D-G5 — Wave speed `c` is a parameter:** `assemble(tokens, wave_speed: f32)`, with `DEFAULT_WAVE_SPEED` constant. Variable from day one (environment may dictate it) without coupling the assembler to the whole `CompilerContext`.
- **D-G6 — `critical_path_delay` derived read-out.** `max over leaves( Σ dᵢⱼ on root→leaf ) / c`, computed in the same pass as phase. Read-only substrate for THEOS-156 cast-time. *Data only — no cast-time mechanic in this story.*
- **D-G7 — `Node { position, frequency, amplitude, phase }`**; `phase` derived (E2).

### Seam ripple (`interface.rs`, `error.rs`)
- **D-S1 — `MagicInterface::parse_input` emits `Vec<WovenToken>`** (was `Vec<HarmonicToken>`). No concrete modalities exist yet, so the seam change is free and keeps connectivity with its emitter.
- **D-S2 — New `CompilerError` variants:** `NoInjectionRoot`, `DanglingLink { from, to }`, `DuplicateTokenId(TokenId)`.

---

## AC → file gap table

| AC | Demand | Lands in | Logic owner |
|----|--------|----------|-------------|
| AC1 | Generational slot lookup fails on mismatch | `graph/arena.rs` | **user** (insert/get/remove generational logic) |
| AC2 | Root-find, orient edges, flag back-edges, no infinite loop | `graph/assembler.rs` | **user** (BFS/DFS + cycle detection) |
| AC3 | CSR offset adjacency, traversable | `graph/csr.rs` | **user** (build + neighbors slice) |
| AC4 | Phase φⱼ per E2 along root paths, read-only | `graph/phase.rs` + D-G2 typestate | **user** (E2 accumulation) |
| AC5 | 5-node acyclic + cyclic tests, ≤1e-5 vs manual | `graph/*` test modules | **user** (fill arrange/act/assert) |
| AC6 | fmt/clippy/test green; rustdoc on every pub item | all | scaffold provides rustdoc; gates verify |
| — | `critical_path_delay` read-out (D-G6) | `graph/phase.rs` + `mod.rs` | **user** (accumulate τ, max over leaves) |

## Scaffolding contract
- Signatures, types, rustdoc, module wiring, and empty-named tests are scaffolded.
- **Every logic-bearing body is `todo!()`** and stays that way until the user implements it — this is where the learning lives (generational indices, graph orientation, phase math).
- Empty test bodies pass vacuously; names encode the scenario. Fill arrange/act/assert during implementation.

## Deferred / out of scope
- Stability solving (M3, THEOS-150); optimization passes (M4).
- Cast-time mechanic + E11 canon (THEOS-156).
- Chakra body-position resolution from a real body model (BodyAnchor carries `body_position` explicitly for now).
- EventBus compiler-event payloads (story mentions THEOS-74; wire if cheap, else note as follow-up).

## Coverage target
Per-crate ≥ 85% region (target 90%). The arena, CSR, and phase math are all unit-testable without winit/IO, so the bar is reachable; the assembler's error paths (D-S2) must be exercised.
