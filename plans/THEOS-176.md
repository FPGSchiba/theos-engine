# THEOS-176 — [magic] E15 resonance gain (Q = 1/Sᵞ)

**Status:** ✅ **IMPLEMENTED** 2026-06-25 (guided session). Decisions DG1–DG5 locked & shipped:
`resonance_gain(&self, ω, &tuning) -> Result<f32, ResonanceRunaway>` = `S.powf(-γ)` in new
`graph/gain.rs`; `γ` on `WeaveTuning`; `Err` only at `S ≤ 0` (volatile band returns finite Q).
5 tests; `theos-magic` 98.0% (`gain.rs` 94.7%). Applying `Q` to damage/velocity stays downstream.
**Epic:** THEOS-95 · **Crate:** `theos-magic` · **Blocked by:** THEOS-150 (ρ(G)/S) ✅,
THEOS-159 (output amplitudes) ✅. Canon **E15** landed (cosmology canon agent, 2026-06-21).

---

## Goal (one sentence)
Expose the **resonance gain** `Q = 1/Sᵞ` — the power multiplier a graph's output earns
from spending stability margin — as an **execution-side** read over the frozen graph
(the power half of the orthodox/demonic spine).

## Canon (E15, Equations of the Weave)
```
Q = Θ_env/(Θ_env − ρ(G)) = 1/Sᵞ          γ = WeaveTuning steepness constant
S → 1     ⇒ Q ≈ 1        (Light: modest, well-damped)
S → S_min ⇒ Q grows large (Dark: powerful, thin margin) — still FINITE
S → 0     ⇒ Q → ∞        (detonation / runaway)
```
`Θ_env/(Θ_env − ρ(G))` is only the γ=1 case; the general form is `1/Sᵞ`. Power and safety
are reciprocal — every gain in output is bought with stability margin.

## Decision register — LOCKED (2026-06-25)
| # | Resolution |
|---|---|
| **DG1** | **`Q = S.powf(-γ)`** (the γ-general `1/Sᵞ`), reading `S` from `TensionGraph::stability(ω, tuning).0`. |
| **DG2** | **API = a single scalar `resonance_gain(&self, world_entropy, &tuning) -> Result<f32, ResonanceRunaway>`.** `Q` is graph-wide (one `S` per graph); "applying to output amplitude" is a `× Q` at the consumption site (damage/velocity, Materialization §2.2/§3.1) — **not** in 176's scope. (Chose scalar over a gained-amplitude helper — YAGNI, and it would overlap E10's effector sum.) |
| **DG3** | **`S ≤ 0` ⇒ `Err(ResonanceRunaway)`** — a typed error (same shape as THEOS-173's `CannotChannel`), with a specific message. `1/Sᵞ` is undefined for `S ≤ 0`, and that is exactly the canon runaway/detonation (`ρ(G) ≥ Θ_env`). **Not a new failure mode** — it surfaces the existing "Runaway" mode; the `Stability::Volatile` verdict (150) is the gameplay signal. |
| **DG4** | **Volatile band (`0 < S < S_min`) returns `Ok(finite large Q)`** — the reward-for-risk payoff. The error is the *detonation* boundary only (`S ≤ 0`), not the volatile classification. |
| **DG5** | **γ → `WeaveTuning::gamma`** (a steepness constant, `> 0`); placeholder value, balance-tunable like `Θ₀`/`E_base`. |

## Proposed shape (scaffolding only)
```rust
// execution layer — frozen graph untouched
pub struct ResonanceRunaway;  // S ≤ 0 ⇒ Q diverges (canon Runaway)
impl TensionGraph {
    pub fn resonance_gain(&self, world_entropy: f32, tuning: &WeaveTuning)
        -> Result<f32, ResonanceRunaway> { todo!() }   // Q = 1/Sᵞ
}
```
New module `graph/gain.rs` (one module per equation family: E9/E10 `energy.rs`,
E3–E7 `stability.rs`, E14 `cast.rs`, **E15 `gain.rs`**).

## Test scenarios (names only)
- `gain_near_one_for_well_damped_graph` (S → 1 ⇒ Q ≈ 1, AC1/AC3)
- `gain_matches_manual_q_for_known_s_and_gamma` (Q = 1/Sᵞ vs manual, 1e-5, AC1)
- `gain_rises_as_stability_margin_thins` (lower S ⇒ higher Q; volatile band finite, AC3)
- `gain_differs_with_environment_on_same_graph` (two ω ⇒ different Q; frozen graph unchanged, AC2)
- `runaway_when_stability_nonpositive` (S ≤ 0 ⇒ Err(ResonanceRunaway), DG3)

## Out of scope
Applying `Q` to damage/velocity (Materialization / downstream) · overdraw (THEOS-177) ·
cultivation economy (THEOS-174) · the stability solver itself (THEOS-150).

## Acceptance criteria
AC1 `Q = 1/Sᵞ` from `S`, `γ` from `WeaveTuning`, vs manual within 1e-5 · AC2 execution-side,
frozen `TensionGraph` unchanged (same spell, two ω ⇒ different Q) · AC3 `S→1 ⇒ Q≈1`,
`S→S_min ⇒ Q` rises, `S ≤ 0` runaway via the existing path · AC4 fmt/clippy/test green +
rustdoc on every `pub`. Per-crate coverage ≥ 85%.
