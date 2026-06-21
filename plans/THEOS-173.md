# THEOS-173 — [M2] Cast-time energy-throughput floor (Option B): `E_cost / P`

**Status:** PLANNING (decisions open). **Hard-blocked by THEOS-159** (needs `E_cost` computable) —
plan-only until 159 ships. **Blocked by:** THEOS-156 (E13 settling seam, merged) + THEOS-159 (E9).
**Epic:** THEOS-95 · **Crate:** `theos-magic`.

---

## Goal (one sentence)
Layer a **caster-specific energy-throughput floor** over the frozen graph's settling delay, so an
expensive-but-geometrically-tiny spell still takes time to charge:
```
energy_time          = E_cost / P            P = caster "channel power" (energy/time)
effective_cast_time  = max( graph.cast_time(), energy_time )
```
…computed at **execution** (caster + env dependent), **without touching** the frozen `TensionGraph`.

## Canon state
- **E13 (THEOS-156, shipped)** already reserves this: its *"Deferred extension — energy-throughput
  floor (Option B)"* paragraph names `max(settling, E_cost/P)`, says **P has no canon yet**, and
  defers it here. So canon has a *placeholder* to promote, not a blank slate.
- `E_cost` is now (THEOS-159) `E_base + ρ_med·ρ_env·W`, an **execution** quantity → so `energy_time`
  is naturally execution-side, composing over the frozen settling delay. Fits the authoring/execution
  seam we locked for energy and stability.
- **Lore constraint (Core Lore):** the "pouring energy" framing is **explicitly rejected**, and chakra
  **Capacity (E8)** is a *withstand* limit, **not a rate**. So `P` must be framed as a passive
  **conductance** of the caster's channel, never an act of forcing energy through faster.

## Current code state
- `TensionGraph::cast_time()` (`graph/mod.rs:125`) — frozen settling delay (E13). **Do not modify.**
- `ChakraState.capacity: f32` (`chakra.rs:116`) — exists, trained ("grows with cultivation"),
  doc = "how much energy the point holds" (a reserve/withstand, consistent with E8 ≠ rate).
- `CompilerContext` carries caster state (`energy`, `chakras`, `world_entropy`) + `weave_tuning`.

---

## Decision register — **LOCKED** (2026-06-21)

| # | Resolution |
|---|---|
| **DP1** | **Opt 2 — `P` is a new trained "Channel Power" (conductance) stat.** A third caster axis: reserve (`E`) · withstand (`Cᵢ`, E8) · **flow (`P`)**. Framed as passive conductance (no "pouring"). |
| **DP2** | **Per-chakra** `channel_power` on `ChakraState` (mirrors `capacity`); the spell's `P` = **bottleneck `min`** over the chakras it anchors to. |
| **DP3** | Execution-layer `effective_cast_time(graph, caster, env) = max(graph.cast_time(), E_cost/P)`; frozen `TensionGraph` untouched. New module (e.g. `graph/cast.rs`). |
| **DP4** | `P ≤ 0` ⇒ **"cannot channel"** typed error (untrained conduit blocks the cast — lore beat, not ∞). |
| **DP5** | Canon: new **E14 — energy-throughput floor** (*Equations of the Weave*) + **Conductance** concept (*Core Lore*, trained per chakra like capacity). Lands via a canon-agent prompt like 159's, **with implementation** (173 is blocked by 159, so no rush). |

<details><summary>Original open-decision rationale (historical)</summary>

### DP1 — **What IS `P`?**  ⚠️ worldbuilding — your call (no canon today)
Electrical analogy keeps the lore honest: `E` (energy pool) = battery charge; `Cᵢ` (capacity, E8) =
max a node withstands; **`P` = conductance / max current** — the rate energy flows *through* the
channel. Floor `= E_cost/P` = "time to move `E_cost` of charge at max current." Framed as
**conductance (passive)**, not pumping (active) — satisfies the no-"pouring" rule.

- **Opt 0 — Don't adopt the floor.** Keep `cast_time = settling` only (E13 as shipped). Close 173 as
  won't-do. _Cost:_ a tiny-but-expensive spell casts instantly — the exact gap Option B exists to
  close. Legitimate, but gives up the mechanic.
- **Opt 1 — Derive `P` from chakra Capacity (reuse `capacity`).** `P = κ · agg(anchoring chakras'
  capacity)`. _Pros:_ reuses an existing trained stat; ties cast speed to cultivation already tracked;
  no new caster field. _Cons:_ must keep canon's "capacity = withstand" distinct from "P = rate"
  (derive, don't redefine); aggregation is a sub-decision (DP2).
- **Opt 2 — New trained "Channel Power" (conductance) stat.** A third caster axis: reserve (`E`) vs
  withstand (`Cᵢ`) vs **flow (`P`)**. _Pros:_ cleanest conceptual separation, most deliberate world
  model. _Cons:_ new caster-state field + canon symbol + balancing surface; most work.
- **Opt 3 — Derive `P` from energy regen/throughput.** `P` = caster's energy draw/regen rate. _Pros:_
  ties to the existing `E` pool. _Cons:_ reads closest to "pouring" — weakest lore fit.

_Lean:_ if adopting, **Opt 1** for a first cut (reuse `capacity`, framed as conductance) with an
**Opt 2** upgrade path. **Your worldbuilding call.**

### DP2 — Aggregation (if `P` derives from chakras)
Which chakras, combined how? **(a) bottleneck** `min` of the chakras the spell anchors to ("only as
fast as your narrowest involved conduit"); **(b) sum** (combined channel); **(c)** the injection-root
chakra alone. _Lean:_ (a) bottleneck. **Your call.**

### DP3 — `effective_cast_time` seam (scaffolding)
Execution-layer free function / type — **not** on the frozen `TensionGraph`:
`effective_cast_time(graph, caster, env) -> f32 = max(graph.cast_time(), E_cost(graph, env) / P(caster))`.
Reads frozen settling + execution `E_cost` (159) + `P` (caster state). New module `graph/cast.rs` or an
execution module. AC3: same graph, two casters ⇒ same `cast_time()`, different `effective_cast_time`.

### DP4 — `P ≤ 0` guard
Newborn chakras have `capacity = 0` ⇒ under Opt 1, `P = 0` ⇒ `E_cost/P = ∞`. **(a)** treat `P ≤ 0` as
**"cannot channel"** → typed error/`None` (lore-sensible: an untrained caster can't cast); **(b)**
saturate to a max time. _Lean:_ (a) error. **Your call.**

### DP5 — Canon home + numbering
Promote E13's placeholder into a real definition: add `P` to *Equations of the Weave* (likely a **new
E14 — energy-throughput floor**, since E13 explicitly "settles the settling delay only; the floor
layers on at execution") + a **conductance** concept in *Core Lore*. Mirrors the 159 canon-agent edit.
_Numbering (extend E13 vs new E14) is a sub-decision._

</details>

---

## Proposed shape (scaffolding only)
```rust
// execution layer — frozen graph untouched
pub fn effective_cast_time(/* graph, caster, env */) -> f32 { todo!() } // max(settling, E_cost/P)
fn channel_power(/* caster */) -> f32 { todo!() }                       // P, per DP1/DP2
```

## Test scenarios (names only)
- `effective_cast_time_is_settling_when_propagation_bound` (settling > energy_time, AC2)
- `effective_cast_time_is_energy_time_when_throughput_bound` (energy_time > settling, AC2)
- `same_graph_two_casters_differ_only_in_effective_time` (AC3 — frozen `cast_time()` identical)
- `frozen_cast_time_unchanged_by_floor` (AC3)
- `zero_channel_power_caster_cannot_cast` (DP4)

## Out of scope
Interruption / channel-cancel / re-cast · HUD/UX timers · the settling model itself (THEOS-156) ·
the "minimum cast-time" idea (parked — would be a separate `max(settling, t_min)` floor).

## Acceptance criteria
AC1 `P` defined in canon (Equations of the Weave + Core Lore), cited by number · AC2
`effective_cast_time = max(settling, E_cost/P)` vs manual calc within 1e-5 on both bound cases ·
AC3 frozen `TensionGraph`/`cast_time()` unchanged (floor composes at execution) · AC4 fmt/clippy/test
green + rustdoc. Per-crate coverage ≥ 85%.
