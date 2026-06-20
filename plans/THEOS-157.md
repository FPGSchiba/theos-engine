# THEOS-157 — [M2][magic] TemporalFrequency → transfer element (E12)

**Jira:** [THEOS-157](https://firephoenixgames.atlassian.net/browse/THEOS-157) · Story · `M2` `magic`
**Blocked by:** THEOS-149 (assembled graph + spatial-provenance path), THEOS-158 (E11 injection + E4 amplitude — the pass this folds into)
**Math source of truth:** [Equations of the Weave — **E12**](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/170164225)

> 🎯 **Goal:** Lower a `TemporalFrequency` token into a **frequency-selective transfer element**
> (a Gaussian band-pass + propagation delay) attached to a positional node — **not** a node of its
> own — so it scales and delays that node's incoming feeder contributions in the E4 phasor pass.

---

## How the scope changed (design journey, 2026-06-14)

This story began as *"TemporalFrequency → node: timing→position synthesis"* and was redesigned mid-flight.
The decisions, in order:

1. **No compile-time transform (AC3 renegotiated).** The original plan synthesized a node position
   from the caster transform (`d = delay·c`). But E2 phase depends only on *relative* inter-node
   distances, and a rigid `Transform` preserves distances — so applying it at assembly changes no
   phase. World anchoring is therefore a **cast/execution concern only**. `assemble` takes **no**
   `Transform` and **no** `WorldAccess`; the THEOS-147 dependency was dropped.

2. **TemporalFrequency is a filter, not a node (canon change → E12).** Synthesizing a phantom node was
   artificial: a temporal oscillation physically *is* a band-pass filter. A finite tone of frequency
   `f₀` lasting `τ` is a windowed sinusoid whose spectrum (Gabor) is a Gaussian of width ∝ `1/τ`
   centred on `f₀`. So the token already *is* a band-pass spec. It now lowers to a
   **`TemporalTransfer`** attached to the positional node its links resolve to. New canon equation
   **E12** added to *Shared Formalism*; provenance taxonomy + data shapes updated in *Core Architecture*.

3. **Throughput-only.** The filter scales a node's **incoming feeder** contributions (the E4 inputs),
   **never** the node's own authored amplitude `Aᵢ` — the filter centres on the token's `f₀`, generally
   ≠ the host node's own frequency, so self-filtering would silently mute the host.

4. **Parallel filterbank (union) stacking.** Several transfers may stack on one node. They compose by
   the **per-frequency maximum** `H_total(f) = maxₑ Hₑ(f)` (a feeder passes if it lies in *any* band —
   broadens the passable set), with the **dominant band's delay**. (First drafted as multiplicative,
   which intersects/narrows — the opposite of intent — and corrected to union.) Because every `Hₑ ≤ 1`,
   `H_total ≤ 1`: stacking never manufactures gain.

5. **E5 ≠ E12.** E5 coherence weights the gain matrix by pairwise endpoint `Δf`; E12 `H(f)` scales
   amplitude by absolute distance from `f₀`. They are independent; `H(f)` touches amplitude only.

## Locked equations (E12)

```
H(f)       = exp(−(f − f₀)² / (2·σ²))        per-element band-pass, H ∈ (0, 1]
τ          = clamp(duration_ms/1000, τ_min, τ_max)
σ          = 1 / (k · τ)                      Gabor width, default k = 2π
Δφ(f)      = 2π · f · τ                        delay phase (E2 temporal form)
H_total(f) = maxₑ Hₑ(f)                        union of a stack (parallel filterbank)
τ(f)       = τ of the band maximizing Hₑ(f)    dominant-band delay
```

A feeder of source frequency `f_k`, amplitude `A_k`, crossing a node's stack contributes
`A_k · H_total(f_k)` at phase `φ_k + 2π·f_k·τ(f_k)`, applied before the E4 sum (after E11 scaling).

## Implementation map

| Concern | Lands in | Notes |
|---|---|---|
| `WeaveTuning { bandwidth_k, tau_min_s, tau_max_s }` | `tuning.rs` (new) | engine-physics config, carried on `CompilerContext.weave_tuning`; distinct from caster/world state |
| `TemporalTransfer { center_hz, sigma_hz, delay_s }` + E12 helpers | `graph/transfer.rs` (new) | `band_pass_response`, `clamp_tau_seconds`, `sigma_from_duration`, `from_token`, `filter_bank_response` |
| `Node.transfers: Vec<TemporalTransfer>` | `graph/mod.rs` | stacking ⇒ `Vec`; `Node` loses `Copy` |
| Stage 2b — attach by link resolution, root fallback | `graph/assembler.rs` | mirrors the `KinematicVector` (Stage 7) fallback |
| Stage 8 — band-pass + delay into the E4 phasor | `graph/assembler.rs` | `eff_phase = φ + 2π·f·τ` inside the trig; amplitude × `H` |
| `assemble` gains `ctx: &CompilerContext` | `graph/assembler.rs` | reads `ctx.weave_tuning`; `wave_speed`/`alpha` stay explicit params |

## Acceptance criteria

- **AC1** — E12 documented on *Shared Formalism*; provenance taxonomy + data shapes on *Core
  Architecture*; this plan records the local-frame / union / throughput-only decisions. ☑
- **AC2** — a `TemporalFrequency` materializes a transfer whose band-pass + delay match the E12
  closed form within `1e-5`. ☑ (16 `transfer.rs` tests + 7 assembler tests)
- **AC3** — synthesis is defined in the caster-local frame; node positions are local; world anchoring
  deferred to cast/execution; `assemble` takes no `Transform`/`WorldAccess`. ☑
- **AC4** — `cargo fmt --check`, `clippy -D warnings`, `cargo test` green; every `pub` item has
  rustdoc. ☑

## Coverage

`cargo llvm-cov -p theos-magic --all-features` → **98.43% region** overall (target 85%, aspirational 90%):
`transfer.rs` 96.46%, `assembler.rs` 98.72%, `tuning.rs` 100%.

## Deferred / follow-ups

- **Cast-time world anchoring** — the ECS execution backend must compose `anchor_transform ·
  local_position` at spawn (today it does a bare `Transform::from_translation` assuming absolute
  positions). Separate execution/backend story.
- **Chanting reconciliation** — a transfer on a feeder-less Source node is inert (throughput-only);
  a pure-temporal chant must author a feeder topology if it wants its filters to act. Documented on
  the Chanting interface page; an interface concern, not the Core's.
