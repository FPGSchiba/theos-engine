# Theos-Engine

A Rust microkernel-style game engine being built for **Tower of Theos**. The workspace follows a strict-DAG crate graph: foundation (`theos-shared`), kernel (`theos-core` — service locator, event bus, module lifecycle), in-process subsystems (`theos-ecs`, `theos-render`, `theos-physics`, `theos-ai`, `theos-assets`, `theos-window`, `theos-magic`), a Tier-1 plugin boundary (`theos-plugin-api`, `theos-plugin-loader`, `theos-scripting`), and three apps (`theos-launcher`, `theos-editor`, `theos-game`). Design discipline: minimal kernel, composable modules via service locator + typed event bus, and isolation of expensive work (async I/O, AI inference) onto dedicated runtimes so the frame loop stays predictable.

**Math:** `glam` (the library Bevy uses) is the engine's canonical, SIMD-accelerated math vocabulary — use `glam` types directly in public APIs. `theos-math` is a **retired** training artifact; do not depend on it in new code. (Decision 2026-06-12, THEOS-148.)

**Architecture spec:** [Theos-Engine Microkernel Architecture v2](https://firephoenixgames.atlassian.net/wiki/spaces/TOT/pages/116785153)  
**Jira project:** [THEOS](https://firephoenixgames.atlassian.net/browse/THEOS)

---

## graphify

This project has a knowledge graph at graphify-out/ with god nodes, community structure, and cross-file relationships.

Rules:
- For codebase questions, first run `graphify query "<question>"` when graphify-out/graph.json exists. Use `graphify path "<A>" "<B>"` for relationships and `graphify explain "<concept>"` for focused concepts. These return a scoped subgraph, usually much smaller than GRAPH_REPORT.md or raw grep output.
- If graphify-out/wiki/index.md exists, use it for broad navigation instead of raw source browsing.
- Read graphify-out/GRAPH_REPORT.md only for broad architecture review or when query/path/explain do not surface enough context.
- After modifying code, run `graphify update .` to keep the graph current (AST-only, no API cost).

## Quality gates

Run at the end of every implementation or scaffolding task. **Ordered fail-fast** — if step N fails, fix it before running N+1.

**Scaffolding done = steps 1–3 pass:**
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo check --workspace --all-features`

**Implementation done = scaffolding gates + steps 4–5 pass:**
4. `cargo test --workspace --all-features`
5. Per-crate coverage ≥ 85% (see [Coverage targets](#coverage-targets))

Then, on any task that changed code: `graphify update .` (see [graphify](#graphify)).

Rules:
- Use `cargo check` not `cargo build` in the gate — same compile-check, faster. Reserve `build` for when binaries are needed.
- **`--all-features` in the gate** is intentional: some code is behind non-default features (e.g. `theos-ecs`'s `testkit` stub). Without it the gate would silently skip linting, compiling, and testing that code. CI (`.github/workflows/test.yml`) likewise runs `--workspace --all-features`.
- **When a gate fails: investigate the root cause and fix the underlying issue.** Do not `--no-verify` to bypass hooks unless explicitly asked. Do not retry the same command in a loop hoping for a different result.
- **Stale-cache recovery:** if `cargo check` reports "method not found" on code that should obviously compile (missing derives, missing re-exports — especially after editing widely-imported crates like `theos-shared`), try `cargo clean -p <suspected-crate>` and re-check before assuming the source is broken.

## Pre-commit checks

The `.githooks/` hook already runs `cargo fmt --all`. Beyond that, verify locally before issuing `git commit`:

- `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- `cargo test --workspace --all-features` green
- No new `unwrap()` / `expect()` in non-test production code without a justifying invariant comment (e.g. `// SAFETY:` or `// invariant:`)
- `graphify update .` has been run since the last code change

If any of these fail, fix and re-stage before committing. Never `--no-verify` unless the user has explicitly asked for it.

## Test scaffolding pattern

When scaffolding production code (function signatures + `todo!()` bodies, no logic yet), scaffold the tests alongside. They communicate intent — what the function will do once implemented — and seed the coverage target.

Pattern:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_returns_x_when_y() {
        // TODO: implement
    }

    #[test]
    fn function_errors_when_input_invalid() {
        // TODO: implement
    }
}
```

Rules:
- **Test names encode the scenario** in prose form. Read as English: `frame_timer_default_returns_first_frame_timer`, `tick_requests_shutdown_on_close_requested`. Never `test_1`, `test_function`, `it_works`.
- **Bodies stay empty during scaffolding.** Empty tests pass vacuously — that's fine. The intent is captured by the name. When implementation lands, fill in arrange / act / assert.
- **Never `#[ignore]` stub tests.** That hides them from the gate and lets coverage targets rot. An empty body that passes is strictly preferable to a hidden test.

## Coverage targets

Measured via `cargo llvm-cov`. **Per-crate**, not per-file (per-file is too noisy) and not workspace-wide (workspace hides outliers).

- **Minimum: 85% region.** Below this, the implementation task is not complete.
- **Target: 90% region.** Aim here; document any gap in the relevant `plans/` file.

Run per-crate via:

```powershell
cargo llvm-cov -p <crate-name> --summary-only
```

Or read per-crate rows from the workspace summary (`cargo llvm-cov --workspace --summary-only`) — the file paths group by crate.

**Defer list** — files structurally uncoverable by unit tests (winit main-thread closures, binary `main.rs` entries, log subscriber init, anything needing a real `winit::Window`) — is maintained in [plans/test-coverage-2026-06-08.md](plans/test-coverage-2026-06-08.md). When excluding a new file, also add a short code comment at the deferred site explaining why, so the omission is visible to future readers.

## Manual smoke

Tasks that touch user-observable behaviour (window, logs, input, rendering) are not complete until `cargo run -p theos-launcher` confirms the change in a live run.

`cargo test` and `cargo clippy` verify correctness of *code*. They do not verify correctness of *feature*. If interactive smoke is not possible (CI, headless agent, no display), say so explicitly rather than claim success.

## Plans

Implementation plans live in `plans/`:

- **Per-Jira-story plans:** `plans/THEOS-<ticket-id>.md` (e.g. `plans/THEOS-17.md`).
- **Cross-cutting plans not tied to a single ticket:** descriptive filename + date suffix, e.g. `plans/test-coverage-2026-06-08.md`.

Write the plan *before* touching code on any non-trivial task. Capture locked decisions, the gap table (where applicable), and a deferred-work list. Reference the plan from commit messages.
