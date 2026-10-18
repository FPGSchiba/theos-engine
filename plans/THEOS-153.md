---
title: THEOS-153 — TensionGraph CLI debug tool
slug: theos-153-tensiongraph-cli-debug
tier: technical
status: active
created: 2026-10-18
branch: theos-153-tensiongraph-cli-debug
owns_branch: true
---

# THEOS-153 — [M7][magic][tool] TensionGraph CLI debug tool

Jira: [THEOS-153](https://firephoenixgames.atlassian.net/browse/THEOS-153) · Epic: THEOS-95 (G2) ·
Blocked by THEOS-149 (CSR assembly, Done) and THEOS-158 (E4 amplitude, Done) · Blocks THEOS-155
(GUI Altar debugger), THEOS-168 (two-layer save/load).

## Goal

A developer can take a hand-written JSON file describing a token stream, plus optional context
overrides, and get answers about it from a headless `theos-magic-debug` binary, with no graphics or
editor. Three subcommands: show what the assembler built (CSR layout, nodes, oriented edges, phase
shifts), show what the solver concluded (S, η, Stable/Volatile under the file's context), and write
a Graphviz `.dot` rendering. Bad input gives a descriptive error on stderr and a non-zero exit, never
a panic or a plausible-looking wrong answer.

## Non-goals

- The canonical on-disk token-stream / `PreparedSpell` format. That belongs to THEOS-168; this tool's
  envelope is a debug format only.
- Adding serde support to `CompilerContext`, `Chakras` or `EnvResistance` themselves.
  (`WeaveTuning` already derives `Serialize`/`Deserialize` since THEOS-179; the overrides may reuse
  that, but must not make the whole context serializable.)
- Any interactive or GUI surface (THEOS-155), and rendering the `.dot` to an image.
- New solver math. The tool reports what the existing assembler and E7/E9 code compute.
- Maintained-construct / upkeep reporting (E16). Possible later subcommand, not in this story.

## Constraints

- The binary lives at `theos-magic/src/bin/theos-magic-debug.rs` and is only built with the
  `debug-tool` feature (`required-features`). Default `theos-magic` builds gain no dependency and
  no public API.
- `clap` is an **optional** dependency that only `debug-tool` enables.
- `CsrAdjacency` and `TensionGraph` internals stay `pub(crate)`. Only the feature-gated debug module
  inside the library may read them.
- Gate per CLAUDE.md runs `--workspace --all-features`, so the debug module, the binary and the CLI
  tests are linted, compiled and tested in CI. Per-crate coverage for `theos-magic` must stay ≥ 85%
  region with the new module counted.
- `glam` is the math vocabulary; no `theos-math`.
- No new `unwrap()`/`expect()` in non-test code without an invariant comment.

## Decisions

- **Binary inside `theos-magic` behind a `debug-tool` feature.** Chosen so the tool tracks the
  crate's internals with no cross-crate API to keep in sync. Rejected: (b) a separate
  `theos-magic-debug` crate plus public CSR accessors on `theos-magic`, which makes a permanent
  public read API just for a tool; (a2) ungated public accessors, same cost, and always visible.
- **A feature-gated public `debug` module in the library does the work; the binary stays thin.**
  A `src/bin` target is compiled as a separate crate and cannot see `pub(crate)` items, so a
  bin-only design cannot print CSR offsets (AC1). The module has crate access, exposes a read-only
  view plus the report/export formatting, and is reusable by THEOS-155 by enabling the feature.
  Rejected: bin-only (can't see the data); widening visibility for the bin (see above).
- **Input is an envelope of tokens plus optional context overrides.** Assembly needs a full
  `CompilerContext`, and the same tokens assemble and solve differently under different tuning and
  caster state, which is half of what a debugger is for. A minimal file is just the tokens.
  Rejected: a bare token array (context fixed to defaults); accepting both shapes untagged (two
  formats to test, and malformed input fails both shapes with a vague serde message, which
  undermines AC4).
- **The overrides are a small debug-only shape, not serde on the real context types.** This keeps
  THEOS-168 free to define the canonical format without inheriting a debug tool's choices.
  Rejected: deriving serde on `CompilerContext`/`WeaveTuning`, which settles the save format early
  and as a side effect.
- **All solve-time world state comes from the context, including ω. No `--entropy` flag.**
  **Deliberate deviation from AC2**, which names `solve <file> --entropy <f>`. World state is
  context, and one source of truth beats a flag that can disagree with the file. Sweeping ω means
  editing the file. `EnvResistance` (medium, environment) also lives in the overrides, default
  1.0/1.0 (resistance-free). Rejected: `--entropy` (and `--medium`/`--env`) flags, which give two
  sources and need a precedence rule. Jira AC2 should be reworded to match before review.
- **ω outside `[0, 1)` is rejected as input error.** At ω = 1, Θ_env = 0 and S divides by zero;
  above 1, S flips sign. Printing a classification for either would be a confident wrong answer.
- **`clap` derive for argument parsing.** Free `--help`, usage errors and the conventional exit
  code 2 for misuse. Rejected: hand-rolled `std::env::args` parsing, which is cheap to write but
  adds usage/error code that then has to be tested and teaches nothing about the magic system.
- **CLI tests drive the real binary via `std::process::Command` and `CARGO_BIN_EXE_*`, gated on
  the feature.** No new dev-dependencies, and the `--all-features` gate runs them. Rejected:
  `assert_cmd` (+ `predicates`), which reads better but adds two crates for about a dozen assertions.

## Open questions

- **Frequency → colour mapping in the `.dot` export.** The TOT page *Core UX: Spell Appearance &
  Visual Mapping* fixes five frequency-band material families with band boundaries. Using those
  bands would make the debug view agree with the game's look, while a continuous hue is simpler.
  Low risk; settle it in the Phase 1 brainstorm. Either way the contract in C5 holds.

## Risks

- **Float formatting makes CLI tests brittle.** Asserting exact stdout on `f32` values breaks on
  harmless math changes. Signal: CLI tests failing after an unrelated solver tweak. Mitigation:
  fixed, documented precision in the report output, and fixtures chosen with stable values.
- **The debug module becomes a second public API by stealth.** If THEOS-155 starts relying on it,
  its shape hardens. Signal: non-debug code enabling `debug-tool`. Accept for now; revisit when
  THEOS-155 is planned.
- **Coverage drag.** The binary entry is structurally hard to cover. Mitigation: keep it thin
  (parse args, call the module, map errors to exit codes). If needed, defer it per
  `plans/test-coverage-2026-06-08.md` with a comment at the site.

## Components

### C1 — Feature gate and binary target wiring

status: pending
kind: chore

**Responsibility.** Make `debug-tool` exist: the feature, the optional `clap` dependency (added to
the workspace dependency table), and the binary target restricted to the feature.

**Contracts.** A default build of `theos-magic` neither compiles `clap` nor exposes the debug
module. A `--features debug-tool` build produces the `theos-magic-debug` binary. The
`--all-features` workspace gate still passes.

**Tests.** None of its own; it is proven by C6's tests running under the gate, and by a default
`cargo check -p theos-magic` staying clean.

**Done when.** Both build configurations compile and clippy is clean under `--all-features`.

### C2 — Input envelope and context resolution

status: pending
depends-on: 1

**Responsibility.** Turn a JSON document into the tokens plus a fully resolved `CompilerContext`
and `EnvResistance`, ready for assembly and solving. Defaults fill every override the file leaves
out.

**Contracts.** A document that carries only tokens resolves to the engine's default context and a
resistance-free environment. Each override the file supplies (at minimum world entropy, the
environment resistances, and the tuning values the assembler and solver read) replaces only its
own default. Callers must be able to tell apart malformed JSON, a well-formed document of the
wrong shape (missing tokens, unknown/mistyped fields, malformed tokens), and a well-formed document
with an out-of-domain value (ω outside `[0, 1)`), and each failure describes what was wrong and,
where serde gives it, where. Unknown override fields are rejected rather than silently ignored, so
a typo can't fall back to a default unnoticed.

**Tests.** A tokens-only document resolves to default context and environment. An entropy
override changes only world entropy. An environment override changes only the resistances. A
tuning override changes only that constant. Malformed JSON is reported as such. A document missing
its token list is reported as a shape error. A misspelled override field is rejected. ω = 1.0 and
ω = −0.1 are both rejected as out of domain, and ω = 0.0 is accepted.

**Done when.** Those cases pass and lint is clean.

### C3 — Assembled-graph inspection view

status: pending
depends-on: 2

**Responsibility.** Expose a read-only view of what the assembler built and format it as the
`parse` report: CSR offsets, the node list (identity, position, frequency, amplitude, derived
phase, kind), the oriented edges (source → target, back-edge flag) and each edge's phase shift.

**Contracts.** The view is read-only: nothing reachable through it can mutate a graph. Offsets
have node-count + 1 entries and each node's edge range matches the edges listed for it. Every
oriented edge in the graph appears exactly once, and back edges are visibly marked. Phase shifts
are the values the existing E2 code derives, not recomputed differently. Output order is
deterministic for a given input. Assembly errors (`CompilerError`) pass through unchanged so the
CLI can report them.

**Tests.** On a two-node chain the report shows three offsets, two nodes, one forward edge and its
phase shift. On a small cyclic fixture the back edge is marked and appears once. The offsets and
listed edges agree on a branching fixture. Running the report twice on the same input yields
identical text. A token stream with no injection root surfaces the assembler's own error.

**Done when.** Those cases pass and lint is clean.

### C4 — Solve report

status: pending
depends-on: 2

**Responsibility.** Run the existing stability (E7) and efficiency (E9) calculations under the
resolved context and format the `solve` report: S, η and the Stable/Volatile classification.

**Contracts.** S and the classification come from the graph's own stability calculation under the
context's ω and tuning. η comes from the existing efficiency calculation under the context's
`EnvResistance`. The report states the ω and resistances it used, so a reader knows which context
produced the numbers. Numbers are printed at fixed, documented precision.

**Tests.** A stable fixture reports Stable with S above `S_min`. The same fixture with ω raised
past its threshold reports Volatile. η matches the library's efficiency for the same graph and
environment. Changing only the environment resistances changes η but not S. The report echoes the
context values it ran under.

**Done when.** Those cases pass and lint is clean.

### C5 — Graphviz export

status: pending
depends-on: 3

**Responsibility.** Render an assembled graph as a Graphviz `.dot` document: nodes styled by
frequency (colour) and amplitude (size), oriented edges, back edges distinguishable.

**Contracts.** Output is syntactically valid DOT: one directed graph, every node declared once,
every edge referencing declared nodes. Two nodes with clearly different frequencies get different
colours, and the mapping is deterministic. Node size grows monotonically with amplitude and stays
positive and finite for zero or very small amplitudes. Back edges are styled differently from tree
edges. Export never emits NaN or infinite attribute values.

**Tests.** A two-node fixture yields a directed graph with two node declarations and one edge. A
cyclic fixture's back edge carries the distinct style. Nodes at different frequencies get different
colours. A higher-amplitude node gets a larger size than a lower one. A zero-amplitude node still
gets a finite positive size. Exporting the same input twice gives identical text.

**Done when.** Those cases pass and lint is clean.

### C6 — CLI front end and end-to-end tests

status: pending
depends-on: 3, 4, 5

**Responsibility.** The `theos-magic-debug` binary: `parse <file>`, `solve <file>`,
`export <file>` (with an output destination), reading the file, calling the debug module, writing
reports to stdout and failures to stderr, and mapping outcomes to exit codes.

**Contracts.** Success exits 0 with the report on stdout and nothing on stderr. Usage errors
(unknown subcommand, missing file argument) exit with clap's usage code. Every input failure (an
unreadable file, malformed JSON, a shape error, an out-of-domain value, an assembler error) exits
with a single consistent non-zero code, distinct from the usage code, and writes a descriptive
one-or-more-line message to stderr. It never panics. `export` writes the DOT document where asked
and reports where. The binary holds no logic the module tests don't cover.

**Tests.** Driving the built binary: `parse` on a valid fixture exits 0 and stdout contains the
expected offsets and edge lines. `solve` on a stable fixture exits 0 and reports Stable with the
expected S and η. `solve` on the same tokens with a high-ω context reports Volatile. `export`
writes a file that starts as a directed graph. A malformed-JSON file exits non-zero with a
JSON-related message on stderr and empty stdout. A file with ω = 1.0 exits non-zero naming
entropy. A missing file exits non-zero naming the path. An unknown subcommand exits with the usage
code.

**Done when.** Those cases pass, the full gate is green (fmt, clippy `-D warnings`, check, test,
all `--workspace --all-features`), `theos-magic` region coverage ≥ 85%, and every `pub` item has
rustdoc.

## Deviations

