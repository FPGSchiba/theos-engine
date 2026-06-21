# THEOS-150 — [M3] resonance stability sparse solver (E3/E5/E6/E7)

**Status:** PLANNING (decisions open). Implementation deferred.
**Epic:** THEOS-95 · **Crate:** `theos-magic` · **Blocked by:** THEOS-149 (merged) ·
**Blocks:** THEOS-151, THEOS-152, THEOS-154, THEOS-162, THEOS-167 (5 downstream — highest leverage in sprint)

---

## Goal (one sentence)
Compute spell **stability** `S` (E7) from the assembled `TensionGraph` — per-edge tension (E3) and
coherence (E5), the coupling matrix `G` and its spectral radius `ρ(G)` (E6) via a sparse
power-iteration, and the master safety scalar (E7) — classifying the cast Stable vs Volatile.

## Canon (pinned from *Equations of the Weave*, page 170164225)
```
E3  Tᵢⱼ = ½·kᵢⱼ·(dᵢⱼ − Lᵢⱼ)²            T = Σ Tᵢⱼ
E5  Iᵢⱼ = cos(Δφᵢⱼ)·exp(−(Δfᵢⱼ/σ_f)²)   ∈ [−1, 1]
E6  Gᵢⱼ = (kᵢⱼ/k_ref)·Iᵢⱼ (connected) else 0   ρ(G) = |λ_max(G)|
E7  S = 1 − ρ(G)/Θ_env   Θ_env = Θ₀·(1 − ω)   Stable iff S ≥ S_min (def 0.5) else Volatile
```
- `dᵢⱼ`, `Δφᵢⱼ`, `Δfᵢⱼ` derive from endpoint node fields (`position`, `phase`, `frequency`) — **available**.
- `kᵢⱼ`, `Lᵢⱼ`, `σ_f`, `k_ref` are **not** in the data model (see D1/D2/D3).
- `ω` = `CompilerContext.world_entropy` (already exists, execution-side world state).

## Current code state (`theos-magic/src/graph`)
- `Edge` (`csr.rs:12`) = `{ target: NodeId, is_back_edge: bool }`, `Copy`. Back-edges kept & flagged
  precisely so the solver sees the **cycle structure** (E6 loop gain).
- CSR adjacency stores edges **oriented outward** from the root; `neighbors(id)` is a slice.
- `Node` has `position`, `frequency`, `phase` (so `I_ij` inputs are all reachable from endpoints).
- `WeaveTuning` (engine-physics constants) is the home for `k_ref`, `Θ₀`, `S_min`.
- Frozen-derived-field precedent on `TensionGraph`: `critical_path_delay`, `cast_time`.
- No `total_resonance_stability` / `NodeKind` yet (this story + THEOS-159's `NodeKind` may share).

---

## Decision register — **LOCKED** (2026-06-21; D5 approach-level)

| # | Resolution |
|---|---|
| **D1** | **Fatten `Edge`** — add `kᵢⱼ`, `Lᵢⱼ`, `σ_f` inline (stays `Copy`, all `f32`). Rationale: more edge-data stories coming (THEOS-151/162); faithful to "intrinsic edge properties". |
| **D2** | Defaults: `Lᵢⱼ = dᵢⱼ` ⇒ `T ≈ 0` at rest. **No collision:** `T` is a reported diagnostic, never a divisor/gain, and the Materialization spec (174063617) reads E3 tension **nowhere** (damage=`A_j`, velocity=`η`, visuals=`f`/`I`/`A`). Execution injects a *frozen* graph (no deformation), so `T` stays at its compile value — **dormant until a token supplies `L ≠ d`** (the reason `L` is its own field). `kᵢⱼ = k_default`; `k_ref = k_default` ⇒ `G ≡` coherence matrix at rest. AC1 tension test uses an `L ≠ d` fixture. |
| **D3** | `σ_f` = `WeaveTuning` default now; authentic-vs-synthetic **provenance seam deferred**. |
| **D4** | **Freeze `ρ(G)`** on `TensionGraph` (like `cast_time`); expose `stability(world_entropy) → (S, Stability)` as an execution `&self` method. `Θ₀`, `S_min` → `WeaveTuning`; `Stability { Stable, Volatile }` enum. Mirrors 159 D5 exactly. |
| **D5** | **Locked:** power iteration, `O(E)`/iter, 64-iter hard cap, early-exit on convergence tol. `G` is symmetric but the CSR is oriented → matvec covers both directions via **(b) scatter-to-both-endpoints** over the oriented edge list (confirmed 2026-06-21; no extra storage). |

<details><summary>Original open-decision rationale (historical)</summary>

### D1 — Where do `kᵢⱼ`, `Lᵢⱼ`, `σ_f` live?  ⚠️ architecture (the ticket's flagged call)
- **(A) Inline on `Edge`.** Faithful to the formalism ("intrinsic edge properties"). Simple, co-located.
  Cost: bloats the `Copy` traversal struct used in the E2/E4 hot path (`target,bool` → + 3×`f32`).
- **(B) Parallel CSR-aligned coeffs array** (`Vec<EdgeWeave>` indexed like `edges`), owned by the solver/graph.
  Keeps traversal lean and the matvec cache-friendly; more moving parts to keep aligned.
- _Lean:_ (B) given the p95 ≤ 2 ms budget and the "keep the frame loop predictable" ethos — but (A) is
  simpler and the formalism leans that way. **Your call.**

### D2 — Default values (no token supplies these yet)
- `Lᵢⱼ = dᵢⱼ` at assembly (rest length = natural length). **Consequence:** then `Tᵢⱼ = 0` everywhere —
  the structure is *at rest*, so E3 total tension is ~0 until tokens supply `L ≠ d` or geometry is perturbed.
  **Stability is unaffected** — `G` (E6) uses stiffness `kᵢⱼ` and coherence `Iᵢⱼ`, **not** tension `T` — so
  `S` is still meaningful at rest. (AC1's tension test must therefore use an `L ≠ d` fixture.)
- `kᵢⱼ = k_default` (a `WeaveTuning` constant) until token-supplied.
- `k_ref = k_default` ⇒ `kᵢⱼ/k_ref = 1` at rest ⇒ `G ≡ I` (gain matrix is the coherence matrix). Clean baseline.
- _Confirm these defaults, especially the `L = d ⇒ T = 0` consequence._ **Your call.**

### D3 — `σ_f` (frequency tolerance) source
Canon: "σ_f is larger for **authentic (analog)** weaves than **synthetic** ones" → it's a property of weave
**provenance**, not geometry.
- **(A)** global `WeaveTuning` default now, with a provenance seam (authentic/synthetic) deferred.
- **(B)** per-edge from provenance taxonomy immediately (larger scope).
- _Lean:_ (A). **Your call.**

### D4 — Authoring/execution split for `S` (mirror 159 D5)  ⚠️ keep consistent
`ρ(G)` is intrinsic (geometry + `k` + coherence, all frozen at assembly); `Θ_env` depends on `ω`
(`world_entropy`, world state). So — exactly like 159's `W`(frozen)/`E_cost`(execution):
- **Freeze `ρ(G)`** on `TensionGraph` (a derived field like `cast_time`).
- Expose **`stability(world_entropy) → (S, Stable|Volatile)`** as an execution `&self` method.
- `Θ₀`, `S_min` → `WeaveTuning`. `Stable|Volatile` → a small enum.
- _Lean:_ adopt this (consistent with 159). **Confirm.**

### D5 — Solver mechanics (mostly canon-driven; one design note)
- `ρ(G) = |λ_max|` via **power iteration**: `x ← G·x / ‖G·x‖`, sparse matvec is `O(E)`/iter (AC3),
  hard cap **64** iters, early-exit on convergence tol. Largest-magnitude eigenvalue (entries can be
  negative since `Iᵢⱼ ∈ [−1,1]`) is what power iteration finds — correct for spectral radius.
- **Design note:** `G` is a *symmetric* coupling (`Iᵢⱼ`, `kᵢⱼ` are endpoint-symmetric), but the CSR
  stores edges **oriented** (+ back-edges). The matvec must treat coupling **undirected** (`Gᵢⱼ = Gⱼᵢ`) —
  either symmetrize into a solver-local structure or visit both directions. _Decide in impl, noted here._

</details>

---

## Proposed shape (scaffolding only)
New module `theos-magic/src/graph/stability.rs`, re-exported from `graph/mod.rs`.
```rust
pub enum Stability { Stable, Volatile }          // E7 classification
// frozen at assembly:  rho_g: f32 on TensionGraph (like cast_time)
pub fn edge_tension(/* … */) -> f32 { todo!() }  // E3
pub fn edge_coherence(/* … */) -> f32 { todo!() }// E5
pub fn spectral_radius(/* … */) -> f32 { todo!() }// E6 power iteration
// execution-side:
// TensionGraph::stability(&self, world_entropy: f32) -> (f32 /*S*/, Stability)  // E7
```

## Test scenarios (names only)
- `edge_tension_zero_when_length_equals_rest` / `..nonzero_when_stretched` (E3, AC1)
- `coherence_high_for_in_phase_frequency_matched_edge` (E5, AC2)
- `coherence_low_for_divergent_frequencies_via_sigma_f` (E5, AC2)
- `spectral_radius_matches_dense_eigowel_on_small_graph` (E6 — cross-check vs a dense reference)
- `power_iteration_terminates_within_64_iters` + `..p95_under_2ms_for_256_nodes` (AC3)
- `constructive_graph_is_stable` / `destructive_deadening_*` / `cyclic_feedback_runaway_is_volatile` (AC5)

## Out of scope
Rendering (M6) · ECS lowering (M5) · injection/output amplitude (E4/E9 — THEOS-158/159).

## Acceptance criteria → coverage
AC1 E3 tension · AC2 E5 coherence (+σ_f divergence) · AC3 O(E)/iter, 64-cap, p95 ≤ 2 ms @256 nodes ·
AC4 S = 1 − ρ(G)/Θ_env, Stable/Volatile @ S_min · AC5 constructive/destructive/runaway tests ·
AC6 fmt/clippy/test green + rustdoc on every `pub`. Per-crate coverage ≥ 85%.

## Cross-story notes
- **`NodeKind`** (THEOS-159 D3) and this story both add to `Node`/assembly — coordinate so one PR adds the tag.
- **Edge-fields decision (D1)** is a dependency for any later story that reads `kᵢⱼ` (THEOS-151/162).
- **⚠️ Canon reconciliation (Materialization, page 174063617) — lands with 150.** §1.1 step 6–7 and §1.2
  describe `S` and the Stable/Volatile flag as **computed & cached at compile**. This contradicts D4 /
  E7: `Θ_env = Θ₀·(1−ω)` depends on `ω = world_entropy` (execution), and §3.2's entropy loop itself says
  high `ω` raises runtime volatility. Reconcile (same shape as the E9 `W`/`E_cost` edit): **compile freezes
  `ρ(G)`; execution computes `S(ω)` and the Stable/Volatile spawn decision.** Step 7 should cache `ρ(G)`
  (intrinsic), not `S`. Small doc edit — candidate for a canon-agent prompt like THEOS-159's.
