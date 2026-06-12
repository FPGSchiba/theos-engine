# THEOS-148 — [M1][magic] theos-magic crate scaffold & core HarmonicToken seam

**Jira:** [THEOS-148](https://firephoenixgames.atlassian.net/browse/THEOS-148) · Story · `M1` `magic` · Blocks M2 (TensionGraph)

> 🎯 **Goal:** Stand up the `theos-magic` crate and the `HarmonicToken` seam — the M1
> foundation of the Harmonic Strings system — so M2 (TensionGraph) and downstream G2 AI prompt
> work have a stable, serde-capable token contract + `MagicInterface` compilation trait to build
> against. **Scaffold + seam only**: no graph assembly (M2), no solving (M3), no concrete
> castable Interfaces (Runes / Martial Arts / Chanting).

**Confluence:**
[Harmonic Strings](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/170000386) ·
[Core Foundation](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/170065921) ·
[Decision: glam canonical](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/174850050)

---

## Locked decisions

1. **glam is the canonical math vocabulary; `theos-math` is retired.** Decided 2026-06-12 (this
   story). `HarmonicToken` uses `glam::Vec3` directly. Workspace `glam` pin bumped `0.27 → 0.33`;
   `theos-magic` enables glam's `serde` + `bytemuck` features. Recorded in Confluence (decision
   page, child of Microkernel Architecture v2), Jira AC2, `README.md`, `CLAUDE.md`, and memory.
   - **Consequence:** the original AC2 ("no raw glam in the public API") was *inverted* — glam
     types are intentionally part of the public API, matching Bevy's own call.
   - **`Vec3` vs `Vec3A`:** store tokens with `glam::Vec3` (12-byte scalar, no padding in the
     serialized form, deterministic layout); reserve `Vec3A` (16-byte SIMD-aligned) for hot
     compute paths that don't yet exist in M1.

2. **Crate location → top-level `theos-magic/`** (not `crates/theos-magic`). The ticket text said
   `crates/`, but all existing workspace members are top-level; we match convention. Added to
   `[workspace].members` and `[workspace.dependencies]` (path dep) in root `Cargo.toml`.

3. **DAG placement.** `theos-magic` is an in-process subsystem. Edges: `theos-shared → magic`
   (for `EntityId`) and external `glam`. No `ai`/`ecs`/`render` edges in M1. Edition 2024,
   `[lints] workspace = true` (mirrors the sibling subsystem crates).

4. **`CompilerError` → new `thiserror` enum in `theos-magic`.** AC4's `parse_input` returns
   `Result<(), CompilerError>`. Scaffolded with a minimal placeholder variant; real variants land
   when parsing logic exists (M2+ / interface modules). thiserror is already a workspace dep.

5. **`MagicInterface` is object-safe.** Methods: `modality_name(&self) -> &'static str` and
   `parse_input(&self, raw: &[u8], ctx: &CompilerContext, out: &mut Vec<HarmonicToken>)
   -> Result<(), CompilerError>`. No generics / no `Self`-returning methods → usable as
   `dyn MagicInterface`, so interface modules (`MartialArtsParser`, `RuneParser`, …) plug in as
   trait objects. Matches the Core Architecture page: parsers emit tokens; the Core consumes them
   blindly.

---

## Acceptance-criteria gap table

| AC | Requirement | Status | Notes |
|----|-------------|--------|-------|
| AC1 | `theos-magic` compiles as a workspace member | ☑ done | Top-level crate; added to `members` + `workspace.dependencies` |
| AC2 | `HarmonicToken` uses `glam::Vec3`; glam is canonical, intentionally public | ☑ done | 3 variants in `token.rs`; glam `serde` feature on |
| AC3 | `CompilerContext { caster, world_entropy, energy, chakras }` (+ `ambient_elements`) | ☑ done | Chakra model resolved (below); `world_entropy` kept scalar, enriched with `ElementField ambient_elements` |
| AC4 | `MagicInterface` trait with the two declared methods | ☑ done | Object-safe (`interface.rs`); verified by a `dyn` dispatch test |
| AC5 | serde round-trip tests for all `HarmonicToken` variants | ☑ done | 3 JSON round-trip tests in `token.rs` |
| AC6 | `fmt --check`, `clippy -D warnings`, `test` green; rustdoc on every `pub` | ☑ done | `#![warn(missing_docs)]` makes the rustdoc requirement gate-enforced |

---

## Resolved decisions (chakra & element model)

Resolved 2026-06-12 with the user during scaffolding:

1. **Chakras are universal** — every caster has them; Martial Arts merely leans on them hardest.
   So the table stays in the Core `CompilerContext` (no interface leak; option (i)).
2. **Seven chakra points**, English placeholder names (`Root … Crown`) — lore may rename later.
   `Chakra` enum with `COUNT`/`ALL`/`index()` as the single source of truth for "7".
3. **One element per point, retunable.** `Attunement { kind: ElementKind, strength: f32 }` as
   `Option` per point (`None` = unattuned). Element changes only once `strength` decays to `0.0`
   (decay/retune logic is M2+). Modelled as a *continuous attunement*, not a fixed category.
4. **Polarity is dynamic.** `Polarity(f32)` in `-1.0..=1.0` (Yin…Yang), balanced at `0.0`; M2+
   owns drift/oscillation/output-bias. Grounded in the Taoist "flux" principle.
5. **`Chakras` newtype over `[ChakraState; 7]`** with `Index<Chakra>` + `iter`/`iter_mut` **and**
   named accessors (`heart()`, `heart_mut()`, …) — readable *and* iterable for graph passes.
6. **`world_entropy` enriched.** Kept as a scalar (overall instability) and **joined by**
   `ambient_elements: ElementField` — a per-`ElementKind` intensity vector (the "element mix")
   that strengthens tokens matching the locale. `ElementField` mirrors the `Chakras` pattern and
   lives, with `ElementKind`, in its own `element.rs` module.

Behaviour (attunement growth, retuning, polarity dynamics, element biasing, capacity progression)
is **deferred to M2+** — only the data shapes are fixed at the seam.

---

## Test execution

**Result:** 19 unit tests pass; `theos-magic`'s six source files are at **100% region coverage**
(workspace summary shows 96.91% total only because the scoped run includes `theos-shared/types.rs`
at 0% — a dependency file covered by theos-shared's own suite, not part of this crate). No coverage
gap to defer.

- Scaffold tests alongside per the project pattern: descriptive names, empty bodies (pass
  vacuously) until logic lands. Seed AC5 round-trip tests (one per `HarmonicToken` variant) and a
  `MagicInterface` object-safety compile test.
- Coverage target ≥ 85% region per crate. A scaffold crate with `todo!()` bodies will not hit
  85% until the seam has real consumers — **document the expected gap here** and revisit when M2
  lands rather than padding with low-signal tests.

## Deferred

- Real `CompilerError` variants (parsing doesn't exist in M1).
- `MagicInterface` implementations (interface modules are future modality work).
- TensionGraph / IR (M2), solving (M3).
- Fate of `theos-math` files: retired in docs now; whether to drop it from `[workspace].members`
  or keep it building as a reference artifact is a separate cleanup — **not** in this story.
