# THEOS-177 — [magic] B2 overdraw (channel beyond safe P/Cᵢ — demonic speed)

**Status:** ✅ **IMPLEMENTED** 2026-06-25 (guided session). Canon **E14 B2** overdraw
quantification landed 2026-06-25 (THEOS-177 canon agent). DO1–DO5 shipped:
`overdraw_cast_time(&self, &Chakras, env, &tuning, o) -> Result<OverdrawOutcome, CannotChannel>`
in `graph/cast.rs` (`o = energy/(o·P)`, snap `Vec<Chakra>` where `o·Aᵢ > Cᵢ`). 6 tests
(incl. multi-snap); `theos-magic` 98.1% (`cast.rs` 98.8%). **Blocked-by:** THEOS-173 ✅,
THEOS-150 ✅, THEOS-159 ✅. **Crate:** `theos-magic`.

---

## Goal (one sentence)
Let a caster **overdraw** the conduit — channel at `o·P` (`o ≥ 1`) for a **faster cast**
at the risk of the **E8 capacity snap** — as an execution-side extension of THEOS-173's
safe floor, without touching the frozen graph.

## Canon (E14 B2, landed 2026-06-25)
```
o ≥ 1                         overdraw factor (o = 1 safe / orthodox; o > 1 demonic)
energy_time         = E_cost / (o · P)
effective_cast_time = max( cast_time (E13 settling), energy_time )      ← o = 1 ⇒ E14 safe floor
snap (E8 cliff)     when  o · Aᵢ > Cᵢ  for any anchoring chakra i        ← o = 1 ⇒ plain E8 gate
```
One knob `o` unifies the two overdraw forms (faster than `P`; `Aᵢ` past `Cᵢ`). **No new
failure mode** — the only cliff is the existing E8 snap (E7/E15 runaway stays `S`-governed).

## Current code state
- `TensionGraph::effective_cast_time(&self, &Chakras, EnvResistance, &WeaveTuning) -> Result<f32, CannotChannel>` (THEOS-173, `graph/cast.rs`) — the **safe** floor (`o = 1`), no E8 check. **Leave as-is.**
- Bottleneck-`P` logic (min `channel_power` over `node.chakra` anchors) lives in that method — reuse the pattern.
- `ChakraState.capacity` (`Cᵢ`, E8) exists, currently **unused** — this story is its first reader.
- `Node.chakra: Option<Chakra>`, `Node.output_amplitude` (`Aᵢ`), `chakras[chakra]` (`Index<Chakra>`) all available.

## Decision register — LOCKED (2026-06-25)
| # | Resolution |
|---|---|
| **DO1** | **E8 snap is in scope** (AC2 requires it; no separate E8 story). Implemented as the `o·Aᵢ > Cᵢ` per-anchor check within 177 — `ChakraState.capacity`'s first consumer. |
| **DO2** | **Overdraw factor `o ≥ 1`**, **clamped** (`overdraw.max(1.0)`): a caller can't under-draw below safe, and `o·P > 0` always holds (since `P > 0`). `o = 1` ⇒ the THEOS-173 safe timing. |
| **DO3** | **Coupling (canon 3a):** `o` scales effective anchor amplitude → `energy_time = E_cost/(o·P)` (faster) **and** snap when `o·Aᵢ > Cᵢ`. `Aᵢ` = the anchor node's `output_amplitude` (for a body anchor = authored `Aᵢ`); `Cᵢ` = `chakras[node.chakra].capacity`. |
| **DO4** | **`overdraw_cast_time(&self, &Chakras, EnvResistance, &WeaveTuning, overdraw: f32) -> Result<OverdrawOutcome, CannotChannel>`** in `graph/cast.rs`. `OverdrawOutcome { effective_cast_time: f32, snap: Vec<Chakra> }` — `snap` = **every** anchor that snapped (`o·Aᵢ > Cᵢ`); empty when all within `Cᵢ`. A snap is *local* (canon E8), so several can occur at once and each routes its own backlash downstream — reporting only one would lose that. (`OverdrawOutcome` is therefore `Clone`, not `Copy`.) `Err(CannotChannel)` when `P ≤ 0` (reuse THEOS-173's error). |
| **DO5** | **`effective_cast_time` (THEOS-173) stays untouched** — it's the pure safe-throughput floor (no E8 gate). `overdraw_cast_time(…, 1.0)` is the safe cast *with* the E8 injection gate; the two coincide on timing. (A future cleanup could unify; out of scope here.) |

## Proposed shape (scaffolding only)
```rust
// execution layer — frozen graph untouched
pub struct OverdrawOutcome { pub effective_cast_time: f32, pub snap: Option<Chakra> }
impl TensionGraph {
    pub fn overdraw_cast_time(&self, chakras: &Chakras, env: EnvResistance,
        tuning: &WeaveTuning, overdraw: f32) -> Result<OverdrawOutcome, CannotChannel> { todo!() }
}
```

## Test scenarios (names only)
- `overdraw_shortens_cast_vs_safe_floor` (o > 1 ⇒ effective < safe `effective_cast_time`, AC1)
- `safe_factor_one_matches_throughput_floor` (o = 1 ⇒ same timing as THEOS-173, AC1)
- `overdraw_within_capacity_does_not_snap` (o·Aᵢ ≤ Cᵢ ⇒ snap None)
- `overdraw_past_capacity_snaps_the_anchor` (o·Aᵢ > Cᵢ ⇒ snap Some(chakra), AC2)
- `zero_channel_power_cannot_overdraw` (P ≤ 0 ⇒ Err(CannotChannel), reuse)

## Out of scope
The safe floor itself (THEOS-173) · maintained-construct transient overdraw (E16/THEOS-178) ·
cultivation/training of `P`,`Cᵢ` (THEOS-174) · backlash damage modelling (the snap reports
*which* anchor; routing it to `BacklashTarget` is ECS-lowering, THEOS-152).

## Acceptance criteria
AC1 safe vs overdraw modelled; overdraw shortens `energy_time` vs the safe floor (vs manual,
1e-5) · AC2 `o·Aᵢ > Cᵢ` ⇒ E8 snap reported (existing failure mode) · AC3 tests for safe vs
overdraw (faster + risk) · AC4 fmt/clippy/test green + rustdoc on every `pub`. Coverage ≥ 85%.
