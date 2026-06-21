# THEOS-159 — E9 energy + E10 efficiency (η)

**Status:** PLANNING (decisions LOCKED 2026-06-21; canon edits **LANDED** 2026-06-21 on pages 170164225, 168951809, 174063617). Implementation deferred to a dedicated chat.
**Epic:** THEOS-95 · **Crate:** `theos-magic` · **Blocked by:** THEOS-158 (merged) · **Blocks:** THEOS-152 (M5), THEOS-153 (M7), THEOS-154 (M6), THEOS-173 (cast-time floor)

---

## Goal (one sentence)
Compute **E9 energy cost** (per-node and per-graph) and **E10 efficiency η** over the per-node
output amplitudes `Aⱼ` that THEOS-158 already materializes — no new physics, a read pass over
the assembled graph.

## Canon (pinned from *Shared Formalism: Equations of the Weave*, page 170164225)

**E9 — Energy cost**
```
E_cost = Σ (over edges) ρ_med · ρ_env · dᵢⱼ · Aᵢⱼ ,   require E_cost ≤ E
```
ρ_med = medium resistance, ρ_env = environmental resistance, dᵢⱼ = edge length, Aᵢⱼ = edge amplitude.

**E10 — Construct efficiency**
```
η = ( Σ effector output amplitudes ) / E_cost
```

## Current code state (grounding — `theos-magic/src/graph`)
- `Node` (`mod.rs:40`) carries: `position: Vec3`, `frequency`, `amplitude` (authored `Aᵢ`, E1),
  `phase`, `depth` (E11), `output_amplitude` (`Aⱼ`, E4), `transfers` (E12).
  **No** node-kind tag (Anchor/Router/Effector is *not* retained post-assembly).
- `Edge` (`csr.rs:12`) carries: `{ target, is_back_edge }` only. **No** per-edge amplitude, no `ρ`.
- `dᵢⱼ` is derivable from endpoint `position`s. "Effector" exists as an assembly concept
  (`assembler.rs:409`, tests at `:869`) = a node with only incoming edges, but it is not a stored flag.
- Frozen derived fields precedent: `critical_path_delay`, `cast_time` computed at assembly.

---

## Decision register — **LOCKED** (2026-06-21)

| # | Resolution |
|---|---|
| **D1** | `Aᵢⱼ := A_i` — the **emitting (upstream) endpoint's** `output_amplitude`. Resolves canon's underspecified `Aᵢⱼ` in the only way consistent with E10's "destructive → η ≈ 0". **Requires a canon clarification on the E9 page.** |
| **D2** | `ρ_med`, `ρ_env` are **execution-time environment scalars** (default `1.0`), supplied via context — the dynamic factors in D5's model. **No** new `Edge` fields. |
| **D3** | Add a `NodeKind` tag (Anchor/Source · Router · Effector) derived at assembly. **Effector = sink** (has feeders, no outgoing forward edge). η numerator sums effector `A_j`. |
| **D4** | Dissolved by D5: `E_base > 0` makes `E_cost` never zero → η always defined. Invariant `E_base > 0`; "zero-energy" test becomes "minimal graph costs exactly `E_base`". |
| **D5** | **Split intrinsic vs execution.** Freeze `W = Σ dᵢⱼ·A_i` on `TensionGraph` (authoring-side, like `cast_time`). Expose `energy_cost(env) = E_base + ρ_med·ρ_env·W` as an execution `&self` method. `E_base` is a `WeaveTuning` engine-physics constant. **Requires a canon edit (E9 gains `E_base` + the authoring/execution decomposition).** |

**Two canon edits required before/with implementation** (delegated to a Confluence agent —
see `plans/THEOS-159-canon-agent-prompt.md`): (1) pin `Aᵢⱼ = A_i` on E9; (2) add the `E_base`
floor + authoring(frozen `W`)/execution(`ρ`, `E_base`) decomposition to E9, with the cast-time
parallel (E13 settling = authoring; THEOS-173 floor = execution) made explicit.

<details><summary>Original open-decision rationale (historical)</summary>

### D1 — What is the per-edge amplitude `Aᵢⱼ`?  ⚠️ core
Canon sums **per edge** with an *edge* amplitude, but the engine stores amplitude **per node**
(`output_amplitude` = converged `Aⱼ`). So edge `(i→j)` must map to some node amplitude.
- **(A)** downstream target amplitude `A_j` — the wave converged at the edge's target.
- **(B)** upstream source amplitude `A_i`.
- **(C)** mean `(A_i + A_j)/2`.
- **Note:** whatever we pick resolves a genuine ambiguity in canon → per the Canon Authority rule
  it should be pinned with a one-line clarification on the E9 page, not silently coded.
- _Lean:_ (A) — mirrors E11's "per-edge, length-is-resistance" framing. **Your call.**

### D2 — Where do `ρ_med` and `ρ_env` come from? (not in the data model today)
- **(A)** global/contextual scalars (ρ_env is literally environmental; ρ_med a default) supplied at compute time.
- **(B)** per-edge fields on `Edge` (+ token plumbing) — most faithful, largest scope.
- **(C)** constants `= 1.0` now, with a TODO seam for token/world supply later.
- _Lean:_ (A)/(C) hybrid to keep 159 lean. **Your call** — couples to D5.

### D3 — Which nodes are "effectors" for the E10 numerator?  ⚠️ core
`Node` has no kind tag, so effectors aren't directly identifiable.
- **(A)** add an effector/kind tag to `Node` at assembly (faithful; M5/THEOS-152 will want it too) — grows scope.
- **(B)** structural proxy: leaves (no outgoing forward edge) = outputs — cheap, but conflates leaf≠effector.
- _Lean:_ depends on whether canon's "effector" maps cleanly to "graph leaf." **Your domain call.**

### D4 — η when `E_cost == 0` (no-injection / zero-energy graph)? (AC3)
- **(A)** typed error / `Option` — η undefined (honest; matches Rust "parse don't validate").
- **(B)** define η = 0.
- _Lean:_ (A). **Your call** (also fixes the public API shape: `Result<f32, _>` vs `Option<f32>`).

### D5 — Authoring-side (frozen) or execution-side (env-dependent)?  ⚠️ architecture, couples to THEOS-173
E9 depends on `ρ_env` (world state) → by the Authoring/Execution seam (the same argument THEOS-173
uses for P) `E_cost` may **not** belong as a frozen graph field; it may be a method taking an
environment param. Decide whether `energy_cost`/`efficiency` are frozen fields (like `cast_time`)
or `&self`-methods reading an env/context. **Your call** — keep consistent with how 150 treats `Θ_env(ω)`.

</details>

---

## Proposed shape (scaffolding only — fill bodies in the impl chat)
Module: `theos-magic/src/graph/energy.rs` (new), re-exported from `graph/mod.rs`.
Signatures TBD by D4/D5, sketch:
```rust
// per-node E9 contribution and graph total; η over effectors.
// (exact signatures gated on D4 [Result vs Option] and D5 [field vs method+env])
pub fn node_energy(/* … */) -> f32 { todo!() }      // E9 per node
pub fn graph_energy(/* … */) -> f32 { todo!() }     // E9 Σ
pub fn efficiency(/* … */) -> /* η */ { todo!() }   // E10
```

## Test scenarios (names only — AAA bodies in the impl chat)
- `graph_energy_matches_manual_sum_over_edges` (AC1, 1e-5)
- `efficiency_matches_manual_eta_on_constructive_graph` (AC2, 1e-5)
- `efficiency_on_zero_energy_graph_is_well_defined` (AC3 — exact assertion gated on D4)
- `full_transfer_graph_efficiency_near_one` (AC3 boundary)
- `node_energy_scales_with_edge_length_and_amplitude` (E9 monotonicity)

## Out of scope
Injection + E4 `Aⱼ` (THEOS-158, done) · stability E3/E6/E7 (THEOS-150) · ECS/render/CLI consumers (M5/M6/M7).

## Acceptance criteria → coverage
AC1 E9 per-node+graph vs manual 1e-5 · AC2 η vs manual 1e-5 · AC3 zero-energy + full-transfer boundaries ·
AC4 fmt/clippy/test green + rustdoc on every `pub`. Per-crate coverage ≥ 85%.

## Open questions to carry into canon
- D1 resolution → one-line clarification of `Aᵢⱼ` on the E9 page.
- D3 resolution → if "effector" ≠ "leaf", canon should say how an effector is identified.
