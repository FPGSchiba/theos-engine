# THEOS-180 — [magic] E16a/b upkeep rate + depth-attenuated throughput occupation

**Status:** ✅ IMPLEMENTED 2026-08-29 (guided session). `throughput_attenuation` +
`TensionGraph::{upkeep_rate, upkeep}` + `Upkeep` shipped in `graph/upkeep.rs`; 16 tests green,
`theos-magic` coverage 99.47% (`upkeep.rs` 99.79% region). Full gate green
(fmt/clippy `-D warnings`/`check`/`test`, all `--workspace --all-features`). **Follow-up:**
`graphify update .` — CLI not on PATH in this env; run when available.
**Blocked-by:** THEOS-179 ✅ (E16 data shapes). **Relates:** THEOS-178 ✅ (maintained-construct
foundation), THEOS-159 ✅ (`W`/`E_base`/`ρ`), THEOS-150 ✅ (`S`/E7), THEOS-158 ✅ (E11 depth `D`),
THEOS-173 ✅ (E14 `P`). **Blocks:** THEOS-181 (E16c). **Crate:** `theos-magic` (only).

## Implementation notes (2026-08-29)
- `upkeep_rate`: direct E16a formula over `self.stability(ω, tuning).0`, `env`, `self.authoring_work()`.
  No `S ≤ 0` guard — D3 precondition holds.
- `upkeep`: iterate `Chakra::ALL` in canonical order; for each, `reduce(f32::min)` the depths of
  the nodes on that chakra, derive `u_c` once from that minimum. Non-anchored chakras drop out via
  `filter_map`. Output order is deterministic (canonical), not `HashMap`-random.
- Dropped an interim `impl Default for Upkeep` (was only propping up a `::default()`+reassign that
  tripped `clippy::field_reassign_with_default`).

---

## Goal (one sentence)
Compute the per-tick upkeep of a maintained construct — the scalar rate
`U = λ·(1/S)^μ·ρ_med·ρ_env·W` (E16a) and its depth-attenuated distribution over the
anchoring chakras `u_c = exp(−α_p·D_c)·U` (E16b) — as an execution-side read pass over a
frozen `TensionGraph`, no re-solve.

## Canon (Equations of the Weave, E16a/b)
```
U   = λ · (1/S)^μ · ρ_med · ρ_env · W          E16a — per-tick hold cost (E_base NOT re-charged)
u_c = exp(−α_p · D_c) · U                       E16b — per-chakra occupation, D_c from E11 depth
```
- **Convergence:** the same `U` both drains the Energy pool `E` and is the construct's
  Channel-Power `P` occupation. One rate, two roles. This story *computes* the rate; the
  pool-drain and the `P`-ceiling check are lifecycle/ECS (out of scope).
- **`u_c` is per-conduit load, NOT conserved:** `Σ u_c` may exceed `U`. Root (`D_c = 0`) bears
  the full `U`; deeper anchors bear less.
- `W` = `TensionGraph::authoring_work()` (frozen). `S` from `TensionGraph::stability(ω, tuning)`.
  `ρ_med`/`ρ_env` = `EnvResistance` (live). `D_c` = `Node::depth` (E11, frozen).
  `λ`/`μ`/`α_p` = `WeaveTuning::{upkeep_lambda, upkeep_mu, throughput_alpha_p}` (THEOS-179).

## Current code state (all inputs already exist)
- `TensionGraph::authoring_work() -> f32` — frozen `W` (`graph/energy.rs`, THEOS-159).
- `TensionGraph::stability(world_entropy, &WeaveTuning) -> (f32, Stability)` — execution-side
  `S`; reads frozen `rho_g` (`graph/stability.rs`, THEOS-150).
- `EnvResistance { medium, environment }`, `Default` = `1.0`/`1.0` (`graph/energy.rs`).
- `Node::depth: f32` — E11 path length from root, `0.0` at root (`graph/mod.rs`, THEOS-158).
- `Node::chakra: Option<Chakra>` — `Some` on body anchors only.
- `WeaveTuning::{upkeep_lambda, upkeep_mu, throughput_alpha_p}` + `DEFAULT_*` consts (THEOS-179).
- Pattern to mirror: `injection_attenuation(depth, alpha: Option<f32>) -> f32` (`graph/injection.rs`).
- Pattern to mirror: `energy_cost` / `effective_cast_time` — execution-side `&self` read pass,
  never mutates the graph.
- **No `PreparedSpell` type** (that's THEOS-152, To Do). The frozen carrier is `TensionGraph`.

## Decision register — LOCKED (2026-08-29, guided session)
| # | Resolution |
|---|---|
| **D1** | Live on `TensionGraph` as `upkeep_rate(&self, EnvResistance, world_entropy, &WeaveTuning) -> f32` (E16a) and `upkeep(&self, …) -> Upkeep` (E16a+E16b). Matches the E9/E14/E7 execution-side read-pass seam. No `PreparedSpell` — do not front-run THEOS-152. |
| **D2** | `ω` is a **parameter**, not frozen: `upkeep*` take `world_entropy` and call `stability()` internally, exactly as `effective_cast_time` takes `env` and calls `energy_cost`. "Frozen `S`" in the story = frozen `ρ_g`/graph structure; the story's AC2 ("live `ρ`/`ω` raise `U`") is authoritative on `ω` being live. |
| **D3** | `S > 0` is a **documented precondition** — no clamp, no `Result`. A Volatile construct (`S ≤ 0` possible) cannot be maintained; that gate is THEOS-182 (lifecycle). Outside the domain the result may be non-finite. Rationale: a clamp is a lifecycle call wearing a math hat; this story is the pure formula. |
| **D4** | `Upkeep { total: f32, per_chakra: Vec<(Chakra, f32)> }`. `total` = E16a `U`; `per_chakra` = E16b `u_c`, body-anchored chakras only (`Node::chakra == Some`). When several nodes anchor the **same** chakra, `D_c` = the **shallowest** (`min` depth) — heaviest load wins, mirroring the `min`-over-anchors bottleneck in `graph/cast.rs`. |
| **D5** | Expose a free fn `throughput_attenuation(depth, alpha_p: Option<f32>) -> f32` (E16b `Ξ_p`), sibling of `injection_attenuation`. Makes "root bears full `U`" / "strictly decreasing" unit-testable without a graph, and keeps the E11/E16b parallel visible. `None` ⇒ `DEFAULT_THROUGHPUT_ALPHA_P`. |

## Proposed shape (scaffolding only — bodies `todo!()`)
```rust
// graph/upkeep.rs — execution-side read pass, frozen graph untouched
pub fn throughput_attenuation(depth: f32, alpha_p: Option<f32>) -> f32 { todo!() }

#[derive(Clone, Debug, PartialEq)]
pub struct Upkeep { pub total: f32, pub per_chakra: Vec<(Chakra, f32)> }

impl TensionGraph {
    pub fn upkeep_rate(&self, env: EnvResistance, world_entropy: f32, tuning: &WeaveTuning) -> f32 { todo!() }
    pub fn upkeep(&self, env: EnvResistance, world_entropy: f32, tuning: &WeaveTuning) -> Upkeep { todo!() }
}
```
Exports: `Upkeep` + `throughput_attenuation` via `graph/mod.rs` and `lib.rs`; `mod upkeep;`.

## Test scenarios (names only — real bodies filled at implementation)
**`throughput_attenuation`** (mirrors the `injection_attenuation` tests):
- `throughput_attenuation_is_one_at_zero_depth`
- `throughput_attenuation_strictly_decreases_with_depth`
- `throughput_attenuation_composes_multiplicatively`

**`upkeep_rate`** — E16a / AC1, AC2:
- `upkeep_rate_matches_manual_formula` (AC1, within `1e-5`)
- `upkeep_rate_excludes_the_e_base_spawn_cost` (AC1 — contrast `energy_cost`)
- `upkeep_rate_scales_with_authoring_work`
- `upkeep_rate_rises_with_medium_and_environment_resistance` (AC2 — live `ρ`)
- `upkeep_rate_rises_as_world_entropy_thins_the_stability_margin` (AC2 — live `ω` ⇒ `S↓` ⇒ `(1/S)^μ ↑`)
- `same_construct_two_environments_differ_only_in_upkeep_rate` (AC2)
- `upkeep_rate_with_mu_zero_is_margin_blind` (`μ = 0` ⇒ `U` independent of `S`)

**`upkeep`** — E16b / AC3 + D4:
- `upkeep_total_equals_upkeep_rate`
- `upkeep_root_chakra_bears_the_full_rate` (AC3 — `D_c = 0`)
- `upkeep_deeper_anchor_bears_less_than_root` (AC3)
- `upkeep_per_chakra_sum_may_exceed_total` (AC3 — not conserved)
- `upkeep_per_chakra_uses_shallowest_node_when_a_chakra_repeats` (D4)
- `upkeep_omits_non_body_nodes` (`SpatialAnchor` → `chakra: None` absent from `per_chakra`)

## Out of scope / deferred
- E16c construction transient + exponential recovery `τ_p = κ·E_cost` → **THEOS-181**.
- Lifecycle / ECS wiring — wake funnel, `CollapseReason`, pool-drain application, `P`-ceiling
  enforcement, Volatile-can't-hold gate → lifecycle story (**THEOS-182**).
- Training/progression of `λ`/`μ`/`α_p` and the `channel_power`/`capacity` conduits → cultivation.
- Tuned values for the E16 constants (still `TODO(THEOS-179)` placeholders).

## Acceptance criteria (from Jira)
- **AC1:** `U = λ·(1/S)^μ·ρ_med·ρ_env·W` matches a manual calc within `1e-5`; `E_base` not charged per tick.
- **AC2:** live `ρ`/`ω` raise `U` with the frozen graph unchanged (same construct, two environments ⇒ different `U`).
- **AC3:** per-chakra `u_c = exp(−α_p·D_c)·U`; root (`D=0`) bears full `U`; `Σ u_c` may exceed `U`.
- **AC4:** `cargo fmt --check`, `clippy --all-targets --all-features -D warnings`, `test` green + rustdoc on every `pub`; per-crate coverage ≥ 85%.

## Coverage note
`theos-magic` sat at 98.1% after THEOS-177. Target ≥ 90%; the test list above covers every
`pub` symbol and each AC branch, so no structural gap expected.
