# Element Cosmology — 2-D (frequency × phase-polarity)

**Status:** DESIGN (pinning now, 2026-06-21). **Expands E1** from a 1-D to a 2-D element model.
**Supersedes** the 1-D framing of THEOS-171 (which becomes "normalize Axis 1"). Spawn as its own
Jira design ticket (proposed). Decisions below are leaning/recommended unless marked LOCKED.

## Goal
Represent **seven traditional fantasy elements** — Fire, Earth/Nature, Water/Ice, Lightning, Air,
Light, Darkness — from **two orthogonal axes**, grounded in physics the engine already computes.

## The two axes

### Axis 1 — Frequency `f` (E1): the *material* spectrum — **LOCKED: logarithmic**
Five material elements ordered low→high frequency (decreasing density / increasing agitation):

```
Earth/Nature → Water/Ice → Fire → Air → Lightning
  (low f)                                  (high f)
```
- **Curve: logarithmic** — `f_norm = log(f/f_min) / log(f_max/f_min)`, clamped to [0,1].
  Equal frequency-*ratio* per element; perceptually even; range-robust. (LOCKED)
- **Range `[f_min, f_max]`** — _open._ Rec: audible **20 Hz – 20 kHz** (3-decade, acoustic "Harmonic
  Strings" fit). `WeaveTuning`-style tunable.
- **Band widths** — _open._ Even log-fifths by default; can warp per element later (the renderer
  config is hot-reloadable). 5 bands replace A2's current 3 thermal/kinetic/transient families.

### Axis 2 — Phase coherence: **Light ↔ Darkness** polarity — **curve: sigmoid (confirm)**
Rides the coherence/constructiveness the engine already has (E4 superposition, E5 `I ∈ [−1,+1]`,
and A2 already renders coherent=bright-gold, dissonant=crimson). **Not a new authored field —
emergent** from phase geometry (E2-derived): you *construct* light/dark by building a constructive
vs anti-phase graph.

```
Darkness  ←——————  neutral  ——————→  Light
(destructive,        (knife-edge,      (constructive,
 anti-phase)          hard to hold)     in-phase, additive)
```
- **Curve: sigmoid**, steep at center → **neutral is an unstable knife-edge**; a spell *commits* to a
  pole. Staying balanced is a precision feat. (CONFIRM this reading.)
- **Symmetric curve, asymmetric stakes:** Light is safe; **Darkness borders the self-harm cliff**.
- **`c_harm` — self-harm threshold (dark side only).** Past it, controlled destruction tips into the
  canon **deadening/backlash** (E4: undelivered energy reflects as self-harm). Tunable
  (`WeaveTuning`-style), so the "how far is too far" line is dial-able. _Value: open._
### Why choose Darkness? — murim risk/reward (Orthodox vs Demonic)
**Light must NOT be a free lunch**, or no one treads the dark path. Framing (per murim: orthodox =
safe+slow, demonic = dangerous+fast+powerful):

- **Recommended — Axis 2 = stability-margin / loop-gain dial** (reuses E6/E7 `S`, THEOS-150). One dial
  moves all three murim properties together:
  | | Light / Orthodox | Darkness / Demonic |
  |---|---|---|
  | loop gain `ρ(G)` | low, well-damped | high, near ceiling |
  | power | capped | high (gain amplifies) |
  | speed (settling/cast) | slow | fast (under-damped) |
  | risk (margin to `S_min`) | safe | razor-thin → runaway/backlash |
  **Light's drawback = opportunity cost (modest + slow), not risk.** `c_harm` collapses onto **`S_min`**
  (existing `WeaveTuning` constant): tune `S_min` + how steeply power rises as `S → S_min` to set the
  whole risk/reward curve. Nullification = a *dark technique*, not the axis.
- **Alternative — Axis 2 = constructive/destructive**, darkness's power = pure **nullification/anti-magic**
  (negates, doesn't out-damage). Narrower; weaker "fast & powerful" fit.

**LOCKED: Option 1** (stability-margin dial). Nullification = a dark *technique*, not the axis.

**Power-from-margin coupling (LOCKED) — the resonance gain `Q`:** output amplitude is multiplied by
```
Q = 1 / Sᵞ          (Q = Θ_env/(Θ_env − ρ(G)); γ = tunable steepness, WeaveTuning)
```
Light (`S≈1`) → `Q≈1` (modest); Dark (`S→S_min`) → `Q` rises; `S→0` → `Q→∞` (detonation). **Power and
safety are exact reciprocals.** Cliff = `S_min` (existing). This is a NEW canon coupling (today E4 is
feed-forward; `ρ(G)` drove only stability). γ is the "how steeply power rises" dial.

## The seven elements as a grid
5 material × polarity. Pure Light / Darkness are the polarity poles (material-agnostic or any
material pushed to a pole): e.g. *radiant fire*, *shadow ice*, neutral air, pure void.

## Canon impact (edits this implies)
- **E1** — expand prose: element = (frequency band **Axis 1**) × (phase-polarity **Axis 2**); name the
  5 material bands.
- **Axis-2 equation** — pin the polarity = `sigmoid(coherence)` mapping + `c_harm` self-harm threshold.
  Likely references E5 (coherence) rather than a wholly new symbol; numbering TBD (E15?).
- **A2 (Core UX 177897474)** — material families 3 → 5; the Light/Dark *polarity overlay* mostly exists
  already (bright/dissonant rendering, §5). Remap on-hit components (Fire→`Ignite`, Lightning→arc/
  `Resonance`, etc.) — Materialization §3.1.
- **Core Lore** — the Light/Dark path, darkness-as-dangerous, nullification niche.

## Decisions (LOCKED 2026-06-21 unless noted)
- **Axis-1 order:** Earth→Water→Fire→Air→Lightning. ✅
- **Axis-1 range:** 20 Hz – 20 kHz (WeaveTuning). ✅
- **Axis-1 curve / widths:** logarithmic, even log-fifths (warp later via hot-reload). ✅
- **Axis-2:** stability-margin dial (Option 1), sigmoid (neutral = unstable knife-edge). ✅
- **Power:** `Q = 1/Sᵞ` resonance gain; cliff = `S_min`; γ tunable. ✅
- **Numbering:** extend E1 (material bands) + reference E5; add new eq for polarity + `Q`. ✅
- **A2 (3→5 families + on-hit remap):** **separate A2 canon prompt**, not the Shared-Formalism one. ✅
- **Speed / energy accumulation (B) — OPEN:** lives in the chakra capacity + conductance (`P`, THEOS-173)
  system, *not* a damping/settling coupling. **Fork:** B1 speed independent of Light/Dark · B2 Darkness
  **overdraws** safe conductance/capacity for faster accumulation, risking an E8 chakra snap. _(deciding now)_

## Cultivation & resource economy — DESIGN STUB (own ticket; NOT in the canon prompts)
How chakras are trained and how Qi is gathered. Captured here, specified later.
- **Per-chakra trained triad:** Capacity `Cᵢ` (reservoir + E8 withstand) · Conductance `Pᵢ` (throughput,
  173) · Attunement (frequency/element affinity — Axis 1; *proposed third axis*).
- **Training raises ceilings; B2 overdraw exceeds them** → **Qi deviation** = our existing self-harm
  (E8 snap / `S<S_min` runaway / deadening backlash). Orthodox = raise ceilings safely; demonic =
  overdraw past them (faster gains, deviation risk).
- **Qi gathering:** Energy pool `E` (E9). Max `E` = Σ chakra Capacity. Refill = ambient absorption over
  time, gated by Conductance + environment (`ambient_elements`, entropy `ω`, both already on
  `CompilerContext`). "More mana" = train Capacity + faster regen + attunement/locale.
- **Open:** is `E` global vs Σ-per-chakra reserves? regen formula? attunement as a real axis?
  progression/XP? demonic Qi-draining sources? → **defer to a dedicated cultivation design ticket.**

## Relationship to sprint stories
- **THEOS-171** — absorbed: becomes "Axis 1 log normalization + range" once the cosmology lands.
- **THEOS-173** — independent (energy-throughput floor); its canon edit can still ship separately or
  bundle with this one.
- **THEOS-172** (transient on-hit) — affected: "transient" band → "Lightning"; revisit its component.
