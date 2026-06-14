# THEOS-158 — [magic] Injection + E4 interference amplitude (A_j)

**Jira:** [THEOS-158](https://firephoenixgames.atlassian.net/browse/THEOS-158) · Story · `magic` · Blocks M5 (THEOS-152), M6 (THEOS-154), M7 (THEOS-153) · Blocked by THEOS-149
**Spawned follow-up:** [THEOS-159](https://firephoenixgames.atlassian.net/browse/THEOS-159) — E9 energy + E10 efficiency (η), blocked by this story

> 🎯 **Goal:** Produce per-node output amplitude `A_j` by materializing `KinematicVector` tokens as injections into rooted nodes, then computing **E4** phasor superposition at convergent nodes — closing the roadmap gap that blocks M5/M6/M7. Amplitude read-only after assembly.

Math source of truth: [Shared Formalism: Equations of the Weave](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/170164225) — relevant equation **E4** (interference amplitude):

```
A_j = | Σ_k  A_k · e^{i·φ_k} |          phasor superposition at a convergent node j
```

where the sum runs over the contributions `k` arriving at node `j`, each a phasor of magnitude `A_k` and phase `φ_k` (φ already derived per E2 in THEOS-149).

---

## Locked decisions (planning session 2026-06-14)

- **D1 — E9/E10 split out.** Energy (E9) and efficiency η (E10) are **not** in this story. Resolved as a split to keep THEOS-158 within its slot and land "inject + interfere" as one clean, testable unit. Tracked as **THEOS-159** (blocked by this story).
- **D2 — Injection is not a node (inherited invariant).** Per THEOS-149 D-P1/D-P3 and the `token.rs` seam: a `KinematicVector` is an impulse injected *into* an existing rooted node via its `links`, never an oscillator node. This story *materializes* that injection; it does not change the invariant.
- **D3 — `A_j` is a NEW derived field on `Node`, not a replacement for authored `A_i`.** Mirrors how `phase` was made derived/read-only in THEOS-149. Rationale from canon + consumer: E1 amplitude `A_i` is authored at anchors; E4 amplitude `A_j` is derived at convergent nodes; **M5 (THEOS-152) AC3 reads both on the same node** — `HarmonicCollider(radius=A_i)` *and* `VolatileDamage(DPS=A_j)` — so both must stay live after assembly. The injected `m·v` seeds the root's source amplitude, which propagates to derive `A_j` downstream. Field name TBD (e.g. `output_amplitude`).
- **D4 — Injection contributes scalar momentum `m·v`; `direction` is dropped.** Canon amplitude is scalar throughout (E1/E4/E8). M5 (THEOS-152) consumes `A_j` as a scalar DPS and takes spatial placement from `Transform` (node `xᵢ`) — no consumer reads a momentum direction. `velocity` is already scalar speed (`token.rs:43`), so the source amplitude is `mass · velocity`. (Re-confirm against M6/THEOS-154 only if a directional render effect is later specified.)

- **D5 — E4 feeders = all strictly-upstream neighbours; only true cycle-closing edges are excluded.** *(Refined 2026-06-14 — supersedes the original "exclude all back-edges".)* The original rule was too coarse: THEOS-149's binary `is_back_edge` lumps two distinct kinds of non-tree edge together —
  - a **convergence / cross edge** (two *acyclic* branches meeting at a node, e.g. the diamond's `2→4`): orienting it makes **no** directed cycle; this is exactly the interference E4 exists to compute → **include**.
  - a **true cycle-closing edge** (back to an ancestor, e.g. a triangle's closing edge): a feedback loop → **exclude** from E4.

  Source of truth: [THEOS-150](https://firephoenixgames.atlassian.net/browse/THEOS-150) (M3, E3/E5/E6/E7) **explicitly does not define E4** — its Alignment notes state it "*does NOT consume injection… the `KinematicVector` impulse drives E4 (interference amplitude A_j)… out of scope here.*" It handles feedback as the **spectral radius `ρ(G)` of the full coupling matrix** (power iteration, E6), *reading* the back-edges rather than excluding them. So loop-gain is E6/E7's eigenvalue problem; E4 is a separate forward superposition. Orienting edges by distance-from-root order yields a DAG (every feeder strictly upstream), so E4 is a single forward pass with **no fixed-point problem** — the part that would need iterating is precisely E6/E7's.

  **Consequence for implementation (resolved):** no Stage 4 change was needed. Stage 8 orients *every* edge by `(depth, slot)` — the lower endpoint feeds the higher — which is always a DAG, so E4 is a single forward pass with nothing to exclude and `is_back_edge` is left untouched (it stays the E6/E7 / phase-tree concern). The runaway loop-gain that *would* need the cycle structure is E6/E7's spectral radius, which reads the same edges.

- **D6 — Injection binding = linked node, root as fallback, depth-attenuated (E11).** A `KinematicVector` injects its scalar momentum `m·v` into the node its `links` resolve to (reading B); a link-less impulse falls back to the injection root (lore: Prime Mana spool-in point). Multiple injections on a node **sum**. A link to another `KinematicVector` or a `TemporalFrequency` is deferred (the latter has no node yet). **KV→Effector is allowed** (not an error): it models a *terminal kinetic strike* — phase-immune delivery at the manifestation point that forgoes E4 constructive gain. To keep root-injection privileged, the injected source is scaled by a **depth-attenuation factor (new equation E11)**: full at the root (depth 0), diminishing with accumulated path length from the root. `direction` stays on the token for the future Direction grammar (lore "terminal kinetic vector"); only `m·v` magnitude is used here.
- **Dependency — E11 must be authored in canon first.** Per the canon-authority rule, the depth-attenuation math lives on the Equations of the Weave page (as E11) and is cited by number; it is not invented in Rust. Blocks the `A_j` math (AC2). See agent task to formulate E11 + lore update.

### Canon anchors (from Equations of the Weave, E4)
- **E4:** `A_j = |Σ_{k feeding j} A_k·e^{iφ_k}|`. Two-wave reduction: `A = √(A₁² + A₂² + 2·A₁·A₂·cos Δφ)`. Test oracle: constructive Δφ=0 → `A₁+A₂`; destructive Δφ=π → `|A₁−A₂|` (seeds AC3).
- **E11 (defined in canon 2026-06-14):** `Ξ(n) = exp(−α·D(n))`, where `D(n) = Σ dᵢⱼ` over edges on the oriented root→n path (`D(root)=0`), `α` default `0.15`/length-unit. Apply `A_src(n) ⟶ Ξ(n)·A_src(n)` to the injected `m·v` **before** it enters the E4 sum. `Ξ` composes multiplicatively (`exp(−α·Σd) = Π exp(−α·dᵢⱼ)`), so it accrues edge-by-edge along the same root-outward sweep as phase (E2). AC2 oracle must include `Ξ`.
- **Canon ruling (2026-06-14, via agent):** injection root = **Lumbar Anchor** (lowest-ranked chakra), distinct from the Sacral Spool (body intake). Consistent with THEOS-149 `D-G4`. Pending user ratification.

---

## Open design questions (to resolve with the user before coding)

> These are the logic-bearing calls. They get moved into "Locked decisions" as we settle each.
> **Resolved:** ~~Q1 (where injection lives)~~ → D3. ~~Q2 (what injection contributes)~~ → D4.

- ~~**Q3 — Binding**~~ **Resolved → D6.**
- ~~**Q4 — E4 "feeding" set: are back-edges included?**~~ **Resolved → D5.**
- **Q5 — When/ordering of `A_j` derivation.** `A_j` depends on its feeders' amplitudes, so derivation has an ordering dependency from the root (cf. how phase accumulates root→j). Same pass as phase, or a second pass over the assembled graph?

## AC → file gap table (provisional — firms up as Q1–Q5 settle)

| AC | Demand | Likely lands in | Logic owner |
|----|--------|-----------------|-------------|
| AC1 | `KinematicVector` materialized as injection bound to a rooted node (not a node) | `graph/assembler.rs` (+ injection carrier TBD by Q1) | **user** |
| AC2 | `A_j = |Σ A_k·e^{iφ_k}|` within 1e-5 of manual phasor calc | `graph/` (E4 module TBD by Q4/Q5) | **user** (E4 superposition) |
| AC3 | Constructive (in-phase) + destructive (anti-phase) cases verified | `graph/*` test modules | **user** (fill arrange/act/assert) |
| AC4 | fmt/clippy/test green; rustdoc on every pub item | all | scaffold provides rustdoc; gates verify |

## Scaffolding contract
- Signatures, types, rustdoc, module wiring, and empty-named tests are scaffolded.
- **Every logic-bearing body is `todo!()`** until the user implements it — injection materialization and E4 superposition are where the learning lives.
- Empty test bodies pass vacuously; names encode the scenario.

## Deferred / out of scope
- E9 energy / E10 efficiency η — **THEOS-159**.
- Stability solving (E3/E6/E7) — THEOS-150.
- ECS lowering / render / CLI consumption of `A_j` — M5/M6/M7 (THEOS-152/154/153).

## Coverage target
Per-crate ≥ 85% region (target 90%). Injection materialization and E4 math are pure/unit-testable without winit/IO, so the bar is reachable; the injection error path (Q3) must be exercised.
