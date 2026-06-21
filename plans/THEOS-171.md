# THEOS-171 — [formalism] Pin physical-`f` → `f_norm` normalization on Shared Formalism

**Status:** PLANNING — **pure canon** (no engine code). **Blocks:** A2 (final shader band values).
**Epic:** THEOS-95.

---

## Goal (one sentence)
Pin, as canon on *Shared Formalism* (which owns the frequency axis), the mapping from the **physical
frequency `fᵢ`** (E1, Hz) to the **normalized `f_norm ∈ [0,1]`** that the band table is defined on —
replacing A2's provisional linear stopgap.

## Grounded context (from *Core UX: Spell Appearance*, page 177897474, §3–4)
- Bands are fixed on `f_norm`: **Thermal `[0,0.33)` · Kinetic `[0.33,0.67)` · Transient `[0.67,1]`**,
  boundaries at 1/3 & 2/3, crossfade δ=0.04. *(These stay; 171 does not touch them.)*
- A2 today: *"provisional **linear** normalization over the authored frequency range"* — the one
  dependency blocking a final shader build.
- Physical `f` is Hz (E12 tokens carry `frequency_hz`). Theme: **"Harmonic Strings"** — oscillators/pitch.

## Decision register — **OPEN** (two coupled worldbuilding calls)

### DN1 — Range `[f_min, f_max]` (the "authored frequency range", Hz)
What physical-frequency window maps onto `f_norm ∈ [0,1]`? Thermal/fire at the low end, transient/arc
at the high end. _Candidate:_ anchor to the **human audible range ≈ 20 Hz – 20 kHz** — a 3-decade span
that fits the acoustic "Harmonic Strings" theme. Narrower windows are possible if weaves are meant to
occupy a tighter band. **Your call.**

### DN2 — Curve (linear vs logarithmic)
- **Linear:** `f_norm = (f − f_min)/(f_max − f_min)`. Even **Hz width** per band. Simplest; matches the
  provisional. Fine if the range is narrow (< ~1 decade).
- **Logarithmic:** `f_norm = log(f/f_min) / log(f_max/f_min)`. Even **octave/ratio** per band.
  Perceptually natural (pitch, EM, colour are all log), and **range-robust** — widening `f_min/f_max`
  later keeps the bands balanced instead of crushing everything into a sliver.
- _Coupling:_ a wide range (DN1 ≈ 3 decades) ⇒ **log is effectively required** (linear would jam most
  frequencies into the thermal sliver). A narrow range ⇒ either works.
- _Lean:_ **logarithmic** — best default for a vibration/pitch theme and robust to later range changes.
  **Your call.**

### DN3 — Clamp
`f` outside `[f_min, f_max]` clamps to `f_norm ∈ {0, 1}` (monotonic, no wraparound). _Lean: yes._

## Out of scope
Shader implementation (A2/`theos-render`) · the band boundaries themselves (already canon) · any engine code.

## Acceptance criteria
AC1 the `f → f_norm` mapping (range + curve + clamp) is documented as canon on *Shared Formalism*.
AC2 A2 (§4) cites it instead of the provisional linear normalization.

## Delivery
Canon-only edit → folded into a **combined canon-agent prompt with THEOS-173** (E14 + Conductance),
once DN1/DN2 are decided.
