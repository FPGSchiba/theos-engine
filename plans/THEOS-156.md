# THEOS-156 — Cast-time from critical-path propagation delay (E13)

**Story:** [THEOS-156](https://firephoenixgames.atlassian.net/browse/THEOS-156) ·
parent epic THEOS-95 · status In Progress.

Cast-time is a derived, read-only quantity on an assembled `TensionGraph`: the longest
root→leaf path where each **edge** contributes its geometric delay (`τ = d/c`, E2) and
each **crossed node** contributes its dominant-band temporal-transfer delay (E12). It is
the time the injected wave takes to propagate from the injection root to the farthest node
*and* settle through any temporal filters on the way.

Builds on THEOS-149 (`critical_path_delay` substrate), THEOS-157 (E12 transfer delays),
THEOS-158 (E11 injection / E4 amplitude). Canon for E13 lives on the *Equations of the
Weave* page (added; see AC1).

---

## Locked decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **D1** | **Frequency key = feeder frequency of the crossing wave.** A node `n`'s filter delay is `filter_bank_response(&n.transfers, freq_feeder).1`. | Mirrors the E12 application already in Stage 8 (E4), which evaluates a consumer's filter at its *feeder's* frequency. The delay a feeder receives in E12 is the delay E13 charges it. |
| **D2** | **The root contributes no filter delay.** | The root is never *crossed into* — it originates the wave. Falls out naturally if we accumulate a node's filter delay *as we traverse into it* (edge `u→v` adds `v`'s delay); the root is only ever the starting `u`. Consistent with E12's "a transfer on a feeder-less source is inert." No special-casing. |
| **D3** | **Dedicated `cast_time` field + accessor; `critical_path_delay` stays purely geometric.** | Keeps the geometric substrate (THEOS-149) independently inspectable; cast-time is the new combined value. |
| **D4** | **Option B (energy-throughput floor `max(settling, E_cost/P)`) deferred → [THEOS-173](https://firephoenixgames.atlassian.net/browse/THEOS-173).** | `E_cost` is E9, not yet engine-implemented (blocked by [THEOS-159](https://firephoenixgames.atlassian.net/browse/THEOS-159)); `P` ("channel power") has no canon; and per canon's Authoring/Execution split a throughput floor is an execution-time, caster-specific quantity that must compose *over* `cast_time()`, not be baked into the frozen graph. |

## Implementation locus

All in `theos-magic/src/graph/`:

- **assembler.rs Stage 6** (`GraphAssembler::assemble`, the phase/delay BFS): today carries
  `(NodeId, phase, delay, depth)` and tracks geometric `max_delay`. Extend the BFS to *also*
  carry a cast-delay accumulator that adds, per traversed edge `u→v`,
  `edge_delay(d, c)` **plus** `v`'s dominant-band filter delay keyed at `freq_u` (D1/D2).
  Track its per-leaf maximum as `max_cast_time`.
- **assembler.rs Stage 9**: set the new `cast_time` field from `max_cast_time`.
- **mod.rs**: `cast_time: f32` field on `TensionGraph` + `cast_time()` accessor (E13 rustdoc).

`critical_path_delay`'s geometric accumulation is unchanged.

## AC mapping

| AC | Plan |
|----|------|
| AC1 — E13 on canon page | **Done** (Equations of the Weave page; numbered E13, header bumped E1–E13). |
| AC2 — matches manual calc ≤1e-5, incl. transfer-on-critical-path extending cast-time | Tests: cast==geometric with no transfers (acyclic + cyclic THEOS-149 graphs); transfer on critical path measurably extends `cast_time` vs the same graph without it; transfer *off* the critical path does not. |
| AC3 — read-only after assembly | **Structural** — only `&` accessor, no setter; same proof as `critical_path_delay` / `phase`. |
| AC4 — gates green + rustdoc on pub items | `fmt`/`clippy`/`check`/`test` + ≥85% region coverage; rustdoc on `cast_time()`. |

## Deferred / out of scope

- Energy-throughput floor (Option B) → THEOS-173.
- Cast-time **world anchoring** (`anchor_transform · local_position` at spawn) — execution concern.
- Interruption / channeling-cancel / HUD timers.
