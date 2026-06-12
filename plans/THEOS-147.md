# THEOS-147 — [seam] ECS World access trait + in-memory stub + AI components

**Jira:** [THEOS-147](https://firephoenixgames.atlassian.net/browse/THEOS-147) · Story · `gate-g2` `seam` · Blocked by THEOS-75 (resolved, see below)

> 🎯 **Goal:** Define a stable, object-safe **World access contract** plus an in-memory
> implementation and the `AiBrain` / `PendingAiResponse` components, so G2 work
> (THEOS-68 ContextBuilder, THEOS-80 ValidationGate, THEOS-83 BT runtime, THEOS-152 magic)
> can be built and tested against a fixed ECS interface **before** the real `hecs`-backed
> ECS (THEOS-42/43) exists.

---

## Locked decisions

1. **No `theos_types` crate.** `theos-shared` is the canonical, dependency-free shared
   type/contract crate. The THEOS-75 dependency is satisfied by what `theos-shared` already
   exports (`EntityId` is present). Decided 2026-06-12; THEOS-75 updated to match.

2. **World access trait → `theos-shared`.** Forced by the DAG: the dependency graph has no
   `ai → ecs` edge, but `shared → ai` and `shared → ecs` both exist, so `theos-shared` is the
   only crate every consumer can reach. Also consistent with the README, which reserves
   `theos-shared` for "shared object-safe traits." Matches the existing **EventBus precedent**
   (trait in `theos-shared`, `CrossbeamEventBus` impl in `theos-core`).

3. **In-memory stub → `theos-ecs`, behind a non-default `testkit` cargo feature.** The stub is
   simply the *first* World implementation; the `hecs`-backed production World (THEOS-42/43) is
   the second, and they belong in the same crate. Gating it off by default keeps it out of
   release builds; G2 crates depend on it via `dev-dependencies` with `features = ["testkit"]`.
   Pattern precedent: `tokio`'s `test-util` feature. The stub stays valuable as a deterministic
   unit-test double **after** the real ECS lands — the seam graduates into test infrastructure.
   - **Known risk:** cargo feature unification. Only ever enable `testkit` from
     `dev-dependencies`; keep the feature purely additive so it can't leak into production.
   - **Alternative (not chosen):** a dedicated `theos-ecs-testkit` crate. Revisit only if
     `hecs` compile cost in G2 test builds becomes painful, or a hard wall against the
     unification footgun is wanted.

4. **`AiBrain` / `PendingAiResponse` → `theos-ai`** (domain components for the AI layer).

---

## Acceptance-criteria gap table

| AC | Requirement | Status | Notes |
|----|-------------|--------|-------|
| AC1 | World access trait defined + in-memory implementation | ☑ done | Trait + `Component`/`ComponentValue` in `theos-shared/src/world_access.rs`; `StubWorld` in `theos-ecs/src/testkit.rs` (`testkit` feature) |
| AC2 | `AiBrain` and `PendingAiResponse` components defined | ☑ done | `theos-ai/src/components.rs`; both `impl Component` |
| AC3 | Component round-trip unit tests pass | ☑ done | 11 tests in `testkit.rs` (round-trip, replace, both error paths, despawn) |
| AC4 | `cargo fmt --check`, `clippy -D warnings`, `cargo test` green; rustdoc on every `pub` | ☑ done | Gate green; CI runs + covers the `testkit` tests via `--all-features` (see Test execution below) |

---

## Resolved design decisions (were open during design)

- **Object-safety vs. generic component access.** Resolved with the EventBus pattern: an
  object-safe, type-erased `WorldAccess` (keys on `TypeId`, moves `Box<dyn Any + Send>` /
  `Box<dyn ComponentValue>`) plus a generic `WorldAccessExt` (`get<C>` / `insert<C>`)
  blanket-impl'd for every `WorldAccess`.
- **Reads = owned snapshots, not borrows.** `get_erased` returns an owned `Box<dyn Any + Send>`.
  Driven by the architecture's AI path (`AiRequest { snapshot }`, spec §10.1) where world state
  crosses a thread boundary as owned data. On-thread zero-copy can be added later as a separate
  query path on the production World.
- **`ComponentValue` wrapper.** Owned-return clones, but `dyn Any` isn't `Clone` and a runtime
  `TypeId` can't recover a concrete type. So clone-ability is captured at the `insert<C>` boundary
  via `ComponentValue: Any + Send + Sync` (blanket-impl'd for `Component`) — mirrors `ChannelHandle`.
  `insert_erased` takes `Box<dyn ComponentValue>`. (The `Sync` bound is required so the world is
  `Sync` for `Arc<dyn WorldAccess>`.)
- **Component model.** `Component: Send + Sync + 'static + Debug + Clone` marker. The stub stores
  `HashMap<EntityId, RwLock<HashMap<TypeId, Box<dyn ComponentValue>>>>`.
- **Entity ids.** Monotonic `AtomicU64` allocator in the stub (deterministic, collision-free,
  never-reuse → stale ids fail lookup, mirroring generational semantics). The production World
  uses `hecs`'s generational allocator instead.
- **Stub locking.** `parking_lot::RwLock` (optional dep, folded into the `testkit` feature) — no
  poisoning, so no `unwrap()` on lock acquisition.
- **Archetype registry shape.** Exposed minimally as `component_ids(entity) -> Vec<TypeId>`.

## Test execution / CI

- The stub and its tests live behind the non-default `testkit` feature, so the bare
  `cargo test --workspace` does **not** compile or run them.
- **CI already covers them:** `.github/workflows/test.yml` runs both `cargo tarpaulin` and
  `cargo test` with `--workspace --all-features`, which enables `testkit`. No CI change was
  needed.
- **Local gate aligned:** `CLAUDE.md`'s test gate updated to `cargo test --workspace --all-features`
  so a local run matches CI and sees the feature-gated tests.

---

## Crate wiring (done)

- `theos-shared`: `WorldAccess` + `WorldAccessExt` + `Component` + `ComponentValue` in
  `world_access.rs`; `WorldAccessError` (+ `EntityNotFound`/`ComponentNotFound`/
  `ComponentDowncastFailed`) in `error.rs`; all re-exported from `lib.rs`.
- `theos-ecs`: `testkit = ["dep:theos-shared", "dep:parking_lot"]`; `StubWorld` behind
  `#[cfg(feature = "testkit")]`; `theos-shared` + `parking_lot` are optional deps.
- `theos-ai`: `theos-shared` dep added; `AiBrain` / `PendingAiResponse` in `components.rs`.

---

## Deferred / out of scope

- Production `hecs`-backed World + generational entity arena → **THEOS-42 / THEOS-43**.
- Anything a specific G2 consumer needs beyond the four trait capabilities — add when that
  consumer lands, not speculatively.

## Related

- Decision record: `theos-shared` is the type sink (memory `project-theos-shared-type-crate`).
- Precedent: EventBus trait (`theos-shared`) / `CrossbeamEventBus` impl (`theos-core`).
