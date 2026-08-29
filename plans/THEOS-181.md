# THEOS-181 — [magic] E16c construction transient + per-chakra throughput meter / overdraw

**Status:** ✅ IMPLEMENTED 2026-08-29 (guided session). `graph/transient.rs` — `ThroughputMeter`
(`new`/`spawn`/`release`/`tick`/`floor`/`transient`/`occupation`/`overdrawn`), `ConstructId`, free
fns `transient_spike` / `recovery_tau` / `relax_excess`; `WeaveTuning` gains `transient_spike_gain`
(ζ) + `transient_spike_alpha` (α_Δ). 30 transient tests + 2 new tuning tests green. **Full gate green:**
fmt / clippy `-D warnings` / check / test (`--workspace --all-features`); `theos-magic` coverage
**99.42% region** (`transient.rs` 99.23%). `graphify update .` run (AST-only). **Not yet committed.**
Canon writeback (E16c `Δ ∝ E_cost` → `Δ_c = exp(−α_Δ·D_c)·ζ·E_cost`) drafted below, not yet filed.

### Implementation notes (2026-08-29, guided)
- `spawn`: hoist `e_cost` / `tau_p` / `upkeep_rate` / `spike` above the `Chakra::ALL` loop; per-chakra
  `d_c` = shallowest anchoring node depth (mirrors `upkeep.rs`). `floor` and `transient` share the
  `throughput_attenuation(d_c, Some(α))` shape — `α = throughput_alpha_p` for the floor,
  `α = transient_spike_alpha` for the spike. **Not** `injection_attenuation` (E11 `α`) — that conflation
  was caught by `spawn_transient_is_flat_across_anchors_when_alpha_delta_is_zero`.
- `tick`: `lane.transient = relax_excess(lane.transient, construct.tau_p, dt)` — the transient field
  *is* the excess `(u_c − u_∞,c)`; `relax_excess` returns the whole decayed value (assign, not `*=`).
- `floor` / `transient` / `occupation`: aggregate lanes into a `HashMap<Chakra, f32>`, then emit by
  iterating `Chakra::ALL` so the `Vec` is canonical-order and anchored-only.
- `overdrawn`: `self.occupation().into_iter().filter(|&(c, occ)| occ > chakras[c].channel_power)` —
  strict `>` matches canon `Σ u_c > P_c`; order inherited from `occupation()`.
- `release`: `self.constructs.retain(|c| c.id != id)` — unknown id ⇒ no-op.
- Test fix (scaffold bug, not impl): `tick_matches_a_sampled_manual_integration_within_1e_5` now
  accumulates its Euler reference in `f64` — an `f32` accumulation over 200k steps drifts ~8e-5,
  exceeding the 1e-5 gate even though the analytic `tick` value is exact.
**Blocked-by:** THEOS-180 ✅ (E16a/b `Upkeep` — floor `u_c`, consumed here), THEOS-179 ✅ (E16 `WeaveTuning`
constants — `κ` already present), THEOS-159 ✅ (`E_cost` via `energy_cost`), THEOS-177 ✅ (E14/E8 overdraw
cliff this reuses).
**Relates:** THEOS-178 ✅ (maintained-construct foundation).
**Blocks:** THEOS-182 (lifecycle — wake funnel, `CollapseReason`, ECS exits; routes this story's overdraw
signal into the snap).
**Crate:** `theos-magic` only. New module `graph/transient.rs`.

---

## Goal (one sentence)
Model the E16c construction transient — a spike `Δ_c ∝ E_cost` a manifesting construct adds to a
chakra's throughput occupation above the E16b steady floor, relaxing exponentially with
`τ_p = κ·E_cost` — as an execution-side per-chakra **meter** (`ThroughputMeter`) that sums floor +
live transient over every live construct and raises an overdraw signal when `floor + transient > P_c`
on any anchor.

## Canon (Equations of the Weave, E16c — pinned + the gap this story fills)
```
du_c/dt = −(u_c − u_∞,c) / τ_p        exponential relaxation to the floor
u_∞,c   = Σ (over live constructs) Ξ_p(D_c)·U     E16b floor (THEOS-180, consumed)
τ_p     = κ · E_cost                  recovery time constant (κ = recovery_kappa, THEOS-179)
Δ_c     = Ξ_Δ(D_c) · ζ · E_cost       CONSTRUCTION SPIKE — see D3; canon only says "Δ ∝ E_cost"
Ξ_Δ(D)  = exp(−α_Δ · D)               spike depth-attenuation, α_Δ its own constant (D3)
```
- **Analytic step (D2):** `du_c/dt = −(u_c − u_∞,c)/τ_p` has the exact solution
  `(u_c − u_∞,c)(t+dt) = (u_c − u_∞,c)(t) · exp(−dt/τ_p)`. Used verbatim — frame-rate-independent,
  unconditionally stable, one `exp` per construct per `tick`. Explicit Euler is rejected (a cheap
  construct has tiny `τ_p`, so `dt/τ_p` is large and Euler oscillates/diverges).
- **Spawn:** raises the floor by the new construct's `Ξ_p(D_c)·U` (E16b) **and** injects `Δ_c` as the
  initial excess. **Collapse/dismiss:** drops that construct's floor + transient **instantly** — the
  *verdict* is THEOS-182; this story only exposes `release(id)`.
- **Overdraw:** `floor + transient > P_c` on any anchor → signal → routed by THEOS-182 into the
  **existing** E14/E8 capacity snap. **No new failure mode.**

## Canon writeback (E16c has a genuine gap — for the canon-authority agent)
E16(c) states the spike only as **`Δ ∝ E_cost`** — proportional-to, no constant, no per-chakra form —
while every other E16 quantity is a closed form with a named `WeaveTuning` constant, and the E16
provenance note's constant inventory (λ, μ, α_p, κ) omits any spike constant. This story promotes it to:
```
Δ_c = exp(−α_Δ · D_c) · ζ · E_cost
```
adding **ζ** (`transient_spike_gain`, magnitude) and **α_Δ** (`transient_spike_alpha`, spread —
independent of E16b's `α_p`, same independence discipline as E11 `α` ≠ E16b `α_p`, E15 `γ` ≠ E16a `μ`).
`α_Δ = 0` ⇒ flat across anchors; `α_Δ = α_p` ⇒ spike spreads exactly like the floor.

## Current code state (all inputs already exist)
- `TensionGraph::upkeep(env, ω, &WeaveTuning) -> Upkeep` — `total` (`U`) + `per_chakra`
  `Vec<(Chakra, f32)>` (`u_c`, **shallowest-node depth per chakra**, D4 of THEOS-180)
  (`graph/upkeep.rs`).
- `TensionGraph::upkeep_rate(env, ω, &WeaveTuning) -> f32` (`graph/upkeep.rs`).
- `throughput_attenuation(depth, alpha_p: Option<f32>) -> f32` = `Ξ_p` — **takes alpha as a param**,
  so `Ξ_Δ` reuses it directly: `throughput_attenuation(d_c, Some(tuning.transient_spike_alpha))`
  (`graph/upkeep.rs`).
- `TensionGraph::energy_cost(env, &WeaveTuning) -> f32` = `E_cost` (E9) (`graph/energy.rs`).
- `Node::depth: f32` — E11 path length from root, `0.0` at root (`graph/mod.rs`).
- `Node::chakra: Option<Chakra>` — `Some` on body anchors only.
- `Chakra::ALL` (canonical order), `Chakras` (7-lane, `Index<Chakra>`),
  `ChakraState::channel_power` = `P_c` (`chakra.rs`).
- `WeaveTuning::recovery_kappa` = `κ`, default `1.0` placeholder (`tuning.rs`, THEOS-179).
- Patterns to mirror: `graph/upkeep.rs` (free `*_attenuation` fn + small result struct, execution-side
  `&self` read pass) and `graph/cast.rs` (`OverdrawOutcome { snap: Vec<Chakra> }`, canonical-order
  anchor lists, `min`-over-anchors bottleneck).

## Decision register — LOCKED (2026-08-29, guided session)
| # | Resolution |
|---|---|
| **D1** | **Stateful `ThroughputMeter` value type** (user's call: option A over free-functions-only). Owns per-construct **scalars** keyed by an opaque `ConstructId` — per-chakra floor `u_c`, per-chakra transient excess, and `τ_p`. Never holds a `TensionGraph`, an ECS entity, or a `CollapseReason`. A plain data structure the ECS layer (THEOS-182) will embed, exactly as `Chakras` is. |
| **D2** | **Analytic exponential step**: `excess *= exp(−dt/τ_p)` per construct per `tick(dt)`. Exact solution of the E16c ODE; unconditionally stable. Euler rejected. |
| **D3** | Spike is **depth-attenuated per chakra with its own constant** (user's call, + "extra gameplay knob"): `Δ_c = exp(−α_Δ·D_c)·ζ·E_cost`. Two new E16 `WeaveTuning` constants — `transient_spike_gain` (ζ, `>0`, default `1.0`) and `transient_spike_alpha` (α_Δ, `≥0`, default `0.15` mirroring `α_p`). `α_Δ=0` ⇒ flat; `α_Δ=α_p` ⇒ spreads like the floor. `D_c` = same **shallowest-node depth** rule as THEOS-180 D4. |
| **D4** | Overdraw signal = **`overdrawn(&self, &Chakras) -> Vec<Chakra>`** — the anchors where `floor + transient > P_c`, canonical order, empty ⇒ all clear. **No snap/backlash logic here** — the doc comment records that THEOS-182 routes these into the existing E14/E8 snap; E16c raises no new failure mode. (A one-field wrapper struct is the fallback if a named type is wanted later.) |
| **D5** | **Floor is snapshot at `spawn`** under the then-current `env`/`ω`; the meter stores scalars, not a graph, and never re-solves. Canon's "ω raises U with no re-solve" for a *held* construct (live-ω floor drift) is deferred to THEOS-182, which owns the construct registry and can call a future `refresh(id, &graph, env, ω, tuning)`. Rationale: every THEOS-181 AC (AC1 transient decay, AC2 spawn/collapse floor *delta*, AC3 succession-of-constructs overdraw) is expressible on a spawn-snapshot floor; storing `&TensionGraph`/`Arc<TensionGraph>` per construct is the lifecycle-registry flavour THEOS-182 owns. **Flag on review.** |
| **D6** | `graph/transient.rs` **recomputes `D_c` per anchored chakra** (shallowest-node `reduce(f32::min)`, ~4 lines, mirrors `upkeep.rs`) rather than extending `Upkeep::per_chakra` to carry depth. Keeps the shipped-not-committed THEOS-180 output frozen. Floor `u_c` is taken straight from `Upkeep::per_chakra`. **Flag on review.** |
| **D7** | `spawn(&mut self, graph: &TensionGraph, env, world_entropy, tuning) -> ConstructId` — the meter reads the frozen graph **once** at spawn (computes `Upkeep`, `E_cost`, `D_c`, `Δ_c`, `τ_p`) and keeps no reference. Mirrors `overdraw_cast_time`'s `&graph + env + tuning` read-pass shape. |

## Proposed shape (scaffolding only — bodies `todo!()`)
```rust
// theos-magic/src/graph/transient.rs — E16c: execution-side, time-evolving meter over the frozen graph
use super::TensionGraph;
use crate::{Chakra, Chakras, EnvResistance, WeaveTuning};

/// Opaque handle to one live maintained construct inside a [`ThroughputMeter`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstructId(u32);

/// E16c construction-transient magnitude `Δ = ζ · E_cost` (before depth attenuation).
pub fn transient_spike(e_cost: f32, tuning: &WeaveTuning) -> f32 { todo!() }

/// E16c recovery time constant `τ_p = κ · E_cost`.
pub fn recovery_tau(e_cost: f32, tuning: &WeaveTuning) -> f32 { todo!() }

/// One analytic relaxation step of `du/dt = −(u − u_∞)/τ_p`: the excess `(u − u_∞)`
/// after `dt` seconds, `excess · exp(−dt/τ_p)`. Exact; stable for any `dt ≥ 0`.
pub fn relax_excess(excess: f32, tau_p: f32, dt: f32) -> f32 { todo!() }

/// Per-chakra throughput meter (E16c): standing floor `u_∞,c` + decaying construction
/// transient, summed over every live construct. Execution-side state; scalars only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ThroughputMeter { /* Vec<(ConstructId, Record)>, next_id: u32 */ }

impl ThroughputMeter {
    pub fn new() -> Self { todo!() }

    /// Register a manifesting construct: raise the floor by its `Ξ_p(D_c)·U` (E16b) and
    /// inject `Δ_c = exp(−α_Δ·D_c)·ζ·E_cost` as the initial excess; store `τ_p = κ·E_cost`.
    /// Reads `graph` once, keeps no reference (D5, D7).
    pub fn spawn(&mut self, graph: &TensionGraph, env: EnvResistance,
                 world_entropy: f32, tuning: &WeaveTuning) -> ConstructId { todo!() }

    /// Collapse/dismiss: this construct's floor + transient vanish instantly (AC2).
    /// Unknown / already-released id ⇒ no-op. The *verdict* is THEOS-182.
    pub fn release(&mut self, id: ConstructId) { todo!() }

    /// Advance every live transient by `dt`s: `excess *= exp(−dt/τ_p)` (D2).
    pub fn tick(&mut self, dt: f32) { todo!() }

    /// Floor `u_∞,c = Σ Ξ_p(D_c)·U` per anchored chakra, canonical order.
    pub fn floor(&self) -> Vec<(Chakra, f32)> { todo!() }
    /// Live transient `Σ (u_c − u_∞,c)` per anchored chakra — what `tick` decays to zero.
    pub fn transient(&self) -> Vec<(Chakra, f32)> { todo!() }
    /// Meter reading: `floor + transient` per anchored chakra.
    pub fn occupation(&self) -> Vec<(Chakra, f32)> { todo!() }

    /// E16c overdraw signal: anchors where `occupation > P_c`
    /// ([`ChakraState::channel_power`]), canonical order, empty ⇒ all clear.
    /// THEOS-182 routes these into the existing E14/E8 snap — no new failure mode.
    pub fn overdrawn(&self, chakras: &Chakras) -> Vec<Chakra> { todo!() }
}
```
Exports: `graph/mod.rs` `mod transient;` + `pub use transient::{ConstructId, ThroughputMeter,
recovery_tau, relax_excess, transient_spike};`; `lib.rs` adds the same to its `pub use graph::{…}`.
`tuning.rs` gains `DEFAULT_TRANSIENT_SPIKE_GAIN`, `DEFAULT_TRANSIENT_SPIKE_ALPHA`, the two
`WeaveTuning` fields + `Default` wiring, and re-export via `lib.rs`.

## Test scenarios (names only — real bodies at implementation; fixtures mirror `upkeep.rs`/`cast.rs`)
**Free fns — `transient_spike` / `recovery_tau` / `relax_excess`:**
- `transient_spike_is_gain_times_energy_cost`
- `transient_spike_scales_linearly_with_energy_cost`
- `recovery_tau_is_kappa_times_energy_cost`
- `recovery_tau_is_shorter_for_a_cheaper_construct` (AC1)
- `relax_excess_is_analytic_exponential_decay`
- `relax_excess_is_stable_when_dt_far_exceeds_tau` (→ 0, never negative / oscillating)
- `relax_excess_with_zero_dt_is_identity`

**`spawn` + `floor` / `occupation` — E16b consumption + D3 (AC2):**
- `spawn_raises_the_floor_by_the_construct_occupation` (AC2 — floor delta == `Upkeep::per_chakra`)
- `spawn_floor_matches_upkeep_per_chakra` (consumes THEOS-180 exactly, within `1e-5`)
- `spawn_injects_the_construction_transient_on_top_of_the_floor` (occupation right after spawn == floor + Δ_c)
- `spawn_transient_at_the_root_is_gain_times_energy_cost` (D3 — `D_c = 0` ⇒ `Ξ_Δ = 1`)
- `spawn_transient_attenuates_from_the_injection_root` (D3 — deeper anchor's Δ_c < root's)
- `spawn_transient_is_flat_across_anchors_when_alpha_delta_is_zero` (D3 — α_Δ = 0)
- `two_constructs_stack_their_floors` (AC2 — `Σ` live `u_c`)

**`release` (AC2):**
- `release_drops_the_floor_instantly`
- `release_drops_the_transient_instantly`
- `release_of_an_unknown_id_is_a_noop`
- `release_one_of_two_constructs_leaves_the_other_intact`

**`tick` — the E16c ODE (AC1):**
- `tick_relaxes_the_occupation_toward_the_floor` (transient → 0 as t grows)
- `tick_matches_a_sampled_manual_integration_within_1e_5` (**AC1 headline** — fine-step reference integration vs analytic)
- `tick_a_cheaper_construct_recovers_sooner` (**AC1** — two constructs, cheaper transient smaller after equal `dt`)
- `tick_with_no_live_constructs_is_a_noop`
- `tick_never_overshoots_the_floor` (analytic ⇒ monotone; transient stays `≥ 0`)

**`overdrawn` — E14/E8 reuse (AC3):**
- `overdrawn_is_empty_when_every_anchor_is_within_channel_power` (AC3)
- `overdrawn_flags_an_anchor_when_floor_plus_transient_exceeds_channel_power` (AC3)
- `overdrawn_trips_on_a_succession_of_costly_constructs` (**AC3 headline** — spawn N costly, no `tick`, stacked transients push root past `P_c`)
- `overdrawn_clears_after_the_transient_decays` (AC3 — trips at spawn, `tick` a while, clears once floor alone `< P_c`)
- `overdrawn_reports_anchors_in_canonical_order`
- `overdrawn_root_strains_before_deeper_anchors` (D3 consistency)

**`tuning.rs`:**
- extend `default_e16_constants_are_within_their_domains` — `transient_spike_gain > 0`, `transient_spike_alpha >= 0`
- `default_transient_spike_alpha_is_independent_of_e16b_load_alpha`
- extend `weave_tuning_round_trips_through_json` with the two new fields

**`ConstructId`:**
- `construct_ids_are_unique_across_spawns`

## Out of scope / deferred
- The collapse/dismiss **verdict**, wake funnel, `CollapseReason`, ECS exits, and the actual routing
  of `overdrawn` into the E8 snap/backlash → **THEOS-182** (lifecycle). This story only *exposes*
  `release(id)` and *raises* the signal.
- Live-ω floor drift on a held construct (D5) → THEOS-182 `refresh`.
- The upkeep/occupation math itself → THEOS-180 (consumed).
- Tuned values for ζ / α_Δ (and the other E16 constants) — still `TODO(THEOS-179)` placeholders.
- Pool-drain application, `P`-ceiling enforcement → lifecycle / cultivation.

## Acceptance criteria (from Jira)
- **AC1:** the transient relaxes to the floor with `τ_p = κ·E_cost`; a cheaper construct recovers
  sooner (within `1e-5` of a sampled manual integration).
- **AC2:** spawn raises the floor by `u_c`; collapse/dismiss drops it instantly.
- **AC3:** `floor + transient > P_c` raises overdraw → the existing E14/E8 snap (no new failure mode);
  a succession of costly constructs trips it.
- **AC4:** `cargo fmt --check`, `clippy --all-targets --all-features -D warnings`, `test` green +
  rustdoc on every `pub`; per-crate coverage ≥ 85%.

## Coverage note
`theos-magic` sat at 99.47% after THEOS-180 (target ≥ 90%). The test list covers every `pub` symbol
and each AC branch (spawn/release/tick/overdrawn × floor/transient, both `α_Δ` regimes, the ODE
reference check, the succession-overdraw path). No structural gap expected; document any in this file.
