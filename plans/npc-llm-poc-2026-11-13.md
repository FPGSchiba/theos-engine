# NPC LLM Proof of Concept Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Find out whether Qwen3.5-4B (4-bit GGUF, local, llama.cpp) gives a grounded NPC answer within 3 s and 6 GiB VRAM when retrieval picks its context, and produce a results table that shows it.

**Architecture:** `theos-ai` (workspace member, in the gate) owns the record types, the speaker-rule filter, retrieval, the context assembler, the JSON reply format, the backend/embedder/clock/memory boundaries, the request driver (deadline, metrics) and the worker thread. `plugins/llm-poc/` is a separate Cargo workspace (outside the gate) with `theos-ai-llama` (llama.cpp generator + embedder + DXGI VRAM probe) and `npc-poc` (harness: fixture → `theos-ai` types via `theos-content`, runs, scores, writes tables).

**Tech Stack:** Rust 2024; `llama-cpp-2 =0.1.161` (`cuda`, `llguidance`); `llguidance`/`toktrie` 1.7.5; `serde`, `serde_json` (`preserve_order`), `thiserror`, `toml`; `windows` (DXGI); `theos-content` @ `0a6f480676596120c7c62b72a84d0d0807948830`; `clap`, `regex`, `csv` in the harness.

**Spec:** `docs/superpowers/specs/2026-11-13-npc-llm-poc-design.md` — read it before any task.

## Global Constraints

- Local only: no HTTP client in any crate; models are GGUF files in `D:\Projects\theos-engine\models\` (or `THEOS_MODELS_DIR`), never in git.
- `theos-ai` must not depend on `theos-content` and must not read the content file format.
- `theos-ai`: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, no `unwrap`/`expect` in production code without an `// invariant:` comment, no `println!`/`eprintln!`/`dbg!` (clippy.toml), deps only `serde`, `serde_json` (feature `preserve_order`), `thiserror`; dev-dep `toml`; feature `testkit` for test doubles.
- Gate for `theos-ai` (run from the repo root, fail-fast, in order): `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; `cargo check --workspace --all-features`; `cargo test --workspace --all-features`; `cargo llvm-cov -p theos-ai --all-features --summary-only` ≥ 85 % region (target 90 %).
- `plugins/llm-poc/` is its own workspace, pinned to Rust 1.99.1 (`rust-toolchain.toml`), never added to the root or `plugins/` workspace.
- Deadline default **3 s** from request start, checked before every token. Answer token cap **256**. VRAM limit **6 GiB** (6 × 1024³ bytes). Context **8192** tokens. Seed **42**.
- Reply schema: `{"sources": [ids…], "reply": "…"}`, `sources` first, `maxItems` 5, `reply` 1–600 chars, `additionalProperties: false`; with no lore, `sources` is `maxItems: 0` with no `items`.
- Commit messages: `<type>(npc-poc): <summary>`, body references `plans/npc-llm-poc-2026-11-13.md`, ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Do not tune prompts, sampling or speed before the owner has seen the first results tables.

**Build environment for `plugins/llm-poc/`** (this session's shell predates the CUDA/CMake installs; new terminals already have them). Prefix cargo commands in bash with:

```bash
export PATH="/c/Program Files/CMake/bin:/c/Program Files/NVIDIA GPU Computing Toolkit/CUDA/v13.5/bin:$PATH" \
  CUDA_PATH="C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.5" \
  LIBCLANG_PATH="C:\\Program Files\\LLVM\\bin" \
  CudaToolkitDir="C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.5\\" \
  THEOS_MODELS_DIR="D:\\Projects\\theos-engine\\models"
```

`CudaToolkitDir` (with the trailing backslash) is needed by CMake's Visual Studio generator; without it the configure step fails with "The CUDA Toolkit directory '' does not exist". After a failed configure, run `cargo clean -p llama-cpp-sys-2 --release` before retrying (CMake caches the failure).

Work happens in the worktree `D:\Projects\theos-engine\.claude\worktrees\npc-llm-poc`; the models live in the main checkout, hence `THEOS_MODELS_DIR`.

## Review Focus

1. A belief that a layer holds **and** the lens rejects — expect it once, under rejected only (Task 2 test `visible_lists_a_belief_both_held_and_rejected_only_as_rejected`).
2. Reply JSON with quotes, newlines and unicode in `reply`, surrounded by whitespace — expect it to parse with escapes decoded (Task 4 test `parse_reply_decodes_escapes_and_ignores_surrounding_whitespace`).
3. Deadline already passed when prefill ends, or exactly one token generated — expect `StopReason::Deadline`/a finished reply, `time_to_first_token` `None` or set, `tokens_per_second` 0.0, no division by zero (Task 7 tests `deadline_passed_during_prefill_generates_no_tokens`, `single_token_reports_zero_tokens_per_second`).
4. Degenerate embeddings — a zero vector or a NaN component, and equal scores — expect no NaN in the ranking, NaN ranked last, ties by id (Task 5 tests `cosine_with_zero_vector_is_zero`, `select_ranks_nan_scores_last`, `select_retrieved_breaks_ties_by_id`).
5. Worker dropped with a request in flight, or a factory that fails — expect `spawn` to return the error and drop to join without hanging (Task 8 tests `spawn_returns_factory_error`, `drop_with_request_in_flight_joins`).

---

## File structure

```
theos-ai/
  Cargo.toml                 deps, feature testkit, [[test]] lore_poc requires testkit
  src/lib.rs                 module list + re-exports; template add() removed
  src/lore.rs                RecordId, LoreKind, LoreRecord, HolderKind, PersonaLayer, Lens, Speaker, Corpus, CorpusError
  src/knowledge.rs           Visible, visible()
  src/reply.rs               Reply, ReplyError, reply_schema(), parse_reply(), MAX_SOURCES, MAX_REPLY_CHARS
  src/embed.rs               EmbedInput, Embedder, EmbedError, cosine()
  src/retrieval.rs           LoreScope, Retriever
  src/context.rs             ContextOptions, Prompt, Context, assemble()
  src/backend.rs             Thinking, Phase, Piece, GenerationRequest, Backend, Decoder, BackendError
  src/clock.rs               Clock, SystemClock
  src/memory.rs              MemoryProbe, NoProbe
  src/driver.rs              NpcRequest, NpcResponse, Metrics, StopReason, Npc, NpcParts, NpcError, DEFAULT_DEADLINE, ANSWER_TOKEN_CAP
  src/worker.rs              NpcWorker, Ticket, WorkerGone
  src/testkit.rs             FakeClock, FakeEmbedder, ScriptedBackend, Script, FakeProbe   (cfg(any(test, feature = "testkit")))
  tests/lore_poc.rs          gate tests on the fixture (golden filter, no forbidden record in any prompt)
fixtures/lore-poc/expected/corpus.json   theos-ai snapshot, written by `npc-poc export`, committed
plugins/llm-poc/
  Cargo.toml                 [workspace] members = ["theos-ai-llama", "npc-poc"]
  rust-toolchain.toml        channel 1.99.1
  .cargo/config.toml         [net] git-fetch-with-cli = true; [env] CMAKE_CUDA_ARCHITECTURES = "120"
  models.toml                model + embedder files, thinking prefixes, sampling
  theos-ai-llama/
    Cargo.toml
    src/lib.rs               LlamaRuntime, re-exports
    src/generator.rs         LlamaGenerator (impl Backend), ModelProfile, Sampling, render_prompt()
    src/embedder.rs          LlamaEmbedder (impl Embedder), l2_normalize(), QUERY_INSTRUCTION
    src/vram.rs              DxgiProbe (impl MemoryProbe)
    examples/spike.rs        first milestone
    tests/real_model.rs      needs the GGUF files
  npc-poc/
    Cargo.toml
    src/main.rs              clap: export | run
    src/convert.rs           theos-content → theos-ai types
    src/scenarios.rs         scenarios.toml + visible.json types
    src/score.rs             the six checks
    src/report.rs            results.md, results.csv, answers.md
    src/run.rs               the run loop
```

---

### Task 0: Branch setup (controller)

Done by the controller in the worktree before Task 1.

- [ ] Copy into the worktree the untracked `fixtures/lore-poc/`, `docs/superpowers/specs/2026-11-13-npc-llm-poc-design.md` and this plan.
- [ ] Append to `.gitignore`:

```
/models/
*.gguf
fixtures/**/.theos/cache/
plugins/llm-poc/results/
plugins/llm-poc/target/
```

- [ ] Run the root gate steps 1–4 once to get a green baseline (a fresh worktree builds the whole engine).
- [ ] Commit: `chore(npc-poc): add lore-poc fixture, design and plan`.

---

### Task 1: Spike — Qwen3.5-4B on the GPU (first milestone)

A check, not product code: no tests. **If the build fails, stop and report the error to the owner; do not install anything and do not continue.**

**Files:**
- Create: `plugins/llm-poc/Cargo.toml`, `plugins/llm-poc/rust-toolchain.toml`, `plugins/llm-poc/.cargo/config.toml`
- Create: `plugins/llm-poc/theos-ai-llama/Cargo.toml`, `src/lib.rs` (crate doc only for now), `examples/spike.rs`

**Interfaces:**
- Produces: the `plugins/llm-poc` workspace and `theos-ai-llama` crate later tasks extend. Dependencies: `llama-cpp-2 = { version = "=0.1.161", features = ["cuda", "llguidance"] }`, `llguidance = "1.7.5"`, `toktrie = "1.7.5"`, `serde_json = "1"`, `windows = { version = "0.62", features = ["Win32_Graphics_Dxgi", "Win32_Graphics_Dxgi_Common", "Win32_Foundation"] }`.

- [ ] **Step 1:** Check the model files exist: `ls /d/Projects/theos-engine/models/` shows `Qwen3.5-4B-Q4_K_M.gguf` and `Qwen3-Embedding-0.6B-Q8_0.gguf`. If missing, stop and ask the owner to download them (spec §6).
- [ ] **Step 2:** Write the workspace files: `rust-toolchain.toml` (`channel = "1.99.1"`, components rustfmt, clippy); `.cargo/config.toml` with `[net] git-fetch-with-cli = true` and `[env] CMAKE_CUDA_ARCHITECTURES = "120"` (RTX 5080, Blackwell; also keeps the CUDA build short).
- [ ] **Step 3:** Write `examples/spike.rs`. It takes the models dir from `THEOS_MODELS_DIR` (default `../../models` relative to the crate) and prints, in order:
  1. DXGI `CurrentUsage` (local segment) of this process for the NVIDIA adapter (vendor id `0x10DE`), before any load.
  2. Load `Qwen3.5-4B-Q4_K_M.gguf` with all layers on the GPU (`with_n_gpu_layers(u32::MAX)`), context 8192; print `n_params()`, `size()` and DXGI usage again.
  3. Generate 128 tokens unconstrained from a short ChatML prompt (thinking off prefix `<think>\n\n</think>\n\n`), greedy; print prompt tokens, time to first token, tokens per second.
  4. Generate once more with an llguidance sampler built from the spec's reply schema with `enum` `["B-946","B-945"]`; print the raw output and whether `sources` precedes `reply`.
  5. Load `Qwen3-Embedding-0.6B-Q8_0.gguf` in embeddings mode with `LlamaPoolingType::Last`, embed one sentence; print the dimension (expect 1024) and the L2 norm before normalising; DXGI usage again.

  llguidance wiring: `let env = LlamaSampler::llguidance_tok_env(&model); let factory = llguidance::ParserFactory::new_simple(&env)?; let grammar = llguidance::api::TopLevelGrammar::from_json_schema(schema); let parser = factory.create_parser(grammar)?; let matcher = llguidance::Matcher::new(Ok(parser)); let sampler: LlamaSampler = matcher.into();` — verify the exact names against `~/.cargo/registry/src/*/llguidance-1.7.5/src` and adapt.
- [ ] **Step 4:** Build: `cd plugins/llm-poc && cargo build --release --example spike -p theos-ai-llama` (with the build environment above). Expected: builds (the CUDA compile can take 10–30 min). On failure: stop and report.
- [ ] **Step 5:** Run: `cargo run --release --example spike -p theos-ai-llama`. Expected: all five sections print; tokens per second > 0; schema output parses as JSON.
- [ ] **Step 6:** Record the printed numbers, the key-order result and the DXGI deltas in a `## Spike results` section at the end of this plan. If DXGI did not rise by roughly the model size on load, note "DXGI blind to CUDA → use NVML fallback in Task 9".
- [ ] **Step 7:** Commit: `feat(npc-poc): spike — Qwen3.5-4B on CUDA via llama-cpp-2`.

---

### Task 2: `theos-ai` record types and the knowledge filter

**Files:**
- Modify: `Cargo.toml` (root): add `toml = "0.8"` to `[workspace.dependencies]`.
- Modify: `theos-ai/Cargo.toml`, `theos-ai/src/lib.rs` (remove `add`/`it_works`, add `#![deny(missing_docs)]`, modules, re-exports).
- Create: `theos-ai/src/lore.rs`, `theos-ai/src/knowledge.rs` (unit tests inline).

**Interfaces:**
- Produces:
  - `RecordId` — newtype over `String`, `#[serde(transparent)]`, `Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord`; `RecordId::new(impl Into<String>)`, `as_str(&self) -> &str`, `Display`, `From<&str>`.
  - `LoreKind { Belief, Truth, Clue, Topic }` (serde lowercase).
  - `LoreRecord { id: RecordId, kind: LoreKind, title: String, body: String, voice: Vec<String>, held_by: Vec<RecordId>, target: Option<String> }`.
  - `HolderKind { Npc, Faction, People }` (serde lowercase); `PersonaLayer { id, kind: HolderKind, title, role: Option<String>, body }`.
  - `Lens { thinks_from, stance, speaks_to, tier: Option<String>, rejects: Vec<RecordId>, unaware_of: Vec<RecordId> }` (`Default`).
  - `Speaker { id: RecordId, layers: Vec<PersonaLayer>, lens: Lens }` — `layers[0]` is the speaker, then faction, then people.
  - `Corpus::new(records: Vec<LoreRecord>) -> Result<Corpus, CorpusError>`; `get(&RecordId) -> Option<&LoreRecord>`; `records() -> impl Iterator<Item = &LoreRecord>` (id order); `beliefs()` (same, kind Belief only). `CorpusError::DuplicateId(RecordId)`.
  - `Visible { persona: Vec<RecordId>, held: Vec<RecordId>, rejected: Vec<RecordId> }` (held, rejected sorted by id); `Visible::lore(&self) -> impl Iterator<Item = &RecordId>` (held then rejected); `Visible::contains_lore(&self, &RecordId) -> bool`.
  - `pub fn visible(corpus: &Corpus, speaker: &Speaker) -> Visible`.
  - All types derive `Clone, Debug, PartialEq, Serialize, Deserialize` (`Eq` where possible).

- [ ] **Step 1: Write the failing tests** (a hand-built mini corpus in `src/test_support.rs`, declared in `lib.rs` as `#[cfg(test)] pub(crate) mod test_support;`, exposing `pub(crate) fn mini() -> (Corpus, Speaker)` so later tasks' unit tests reuse it: people `p`, faction `f` (parent p), npc `n` (parent f); beliefs `B-1` held by `p`, `B-2` held by `f`, `B-3` held by `other`, `B-4` held by `p` and in `n`'s rejects; truth `T-1` held_by `[p]` (malformed on purpose) and in rejects; clue `C-1`; topic `topic-a`; lens `unaware_of = [T-1, B-2]`):
  - `record_id_serializes_as_plain_string` — `serde_json::to_string(&RecordId::new("B-1"))` is `"\"B-1\""`.
  - `corpus_new_rejects_duplicate_ids` — `Err(CorpusError::DuplicateId("B-1".into()))`.
  - `corpus_beliefs_yields_only_beliefs_in_id_order`.
  - `visible_persona_lists_speaker_then_ancestors` — `["n", "f", "p"]`.
  - `visible_held_includes_beliefs_held_by_any_layer` — contains `B-1`.
  - `visible_held_excludes_beliefs_of_other_holders` — no `B-3`.
  - `visible_excludes_unaware_of_ids` — no `B-2` anywhere.
  - `visible_never_includes_truths_clues_or_topics` — `T-1`, `C-1`, `topic-a` in neither list, though `T-1` names `p` and is in rejects.
  - `visible_lists_a_belief_both_held_and_rejected_only_as_rejected` — `held` lacks `B-4`, `rejected == ["B-4"]`.
  - `visible_ignores_rejects_ids_missing_from_corpus` — lens rejects `B-99` → not in `rejected`, no panic.
- [ ] **Step 2:** Run `cargo test -p theos-ai --all-features` — expect compile failure (types missing).
- [ ] **Step 3:** Implement the types and `visible` per spec §7.2. Rule order: rejected = `lens.rejects` ∩ beliefs ∖ `unaware_of`; held = beliefs whose `held_by` meets any layer id, ∖ rejected ∖ `unaware_of`. Doc comments on every public item.
- [ ] **Step 4:** Run `cargo test -p theos-ai --all-features` — PASS.
- [ ] **Step 5:** Run root gate steps 1–3. Commit: `feat(npc-poc): theos-ai record types and speaker-rule filter`.

---

### Task 3: Harness `export` and the golden filter test

**Files:**
- Modify: `plugins/llm-poc/Cargo.toml` (add member `npc-poc`).
- Create: `plugins/llm-poc/npc-poc/Cargo.toml`, `src/main.rs`, `src/convert.rs`, `src/scenarios.rs`.
- Create: `theos-ai/tests/lore_poc.rs`; modify `theos-ai/Cargo.toml` (`[[test]] name = "lore_poc"`, `required-features = ["testkit"]`; add an empty `testkit` feature now if Task 5 has not).
- Create (generated): `fixtures/lore-poc/expected/corpus.json`.

**Interfaces:**
- Consumes: Task 2 types, `visible()`.
- Produces:
  - `npc-poc` deps: `theos-ai = { path = "../../../theos-ai", features = ["testkit"] }`, `theos-content = { git = "https://github.com/FPGSchiba/theos-editor", rev = "0a6f480676596120c7c62b72a84d0d0807948830" }`, `clap` (derive), `serde`, `serde_json`, `toml`, `anyhow`.
  - `convert::load_fixture(records_dir: &Path) -> anyhow::Result<(Vec<LoreRecord>, Vec<Speaker>)>` — records of kind belief/truth/clue/topic become `LoreRecord`; every `npc` record becomes a `Speaker` via `Index::lens(id)` (chain → layers with title/role/body read from each layer's record; `fields` → `Lens`). Voice lines = in-world quote lines of `> [!voice]` callouts without the header line. Body = body text with callouts removed, trimmed. Entry points: `Project::open(root)`, `theos_content::parts::builtin()`, `Index::open(&project, &registry)`, `Index::records()`, `Index::lens(id)`, `record::read_record`/`Record::parse` and its `body().callouts()`.
  - `scenarios::Scenarios { forbid_words: Vec<String>, top_k: usize, scenario: Vec<Scenario> }`, `Scenario { id, npc, question, behaviour: Behaviour, expect, also_ok, rejects, forbid, forbid_words, hand_scored: bool, note }` (`#[serde(default)]` on optional lists); `Behaviour { Answer, Unaware, Rumour, Deflect }`; `VisibleEntry { persona, held, rejected, unaware_of }` keyed by npc id. `load_scenarios(path)`, `load_visible(path) -> BTreeMap<String, VisibleEntry>`.
  - Snapshot JSON shape: `{ "records": [LoreRecord…], "speakers": [Speaker…] }`, pretty-printed, records in id order, speakers in id order.
  - CLI: `npc-poc export [--fixture <dir>]` (default `../../fixtures/lore-poc` relative to `plugins/llm-poc`) writes `<fixture>/expected/corpus.json`.

- [ ] **Step 1: Write the failing gate test** `theos-ai/tests/lore_poc.rs` with helpers `fixture(path)` (`concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/lore-poc/")`), a local `#[derive(Deserialize)] struct Snapshot { records: Vec<LoreRecord>, speakers: Vec<Speaker> }`, and a local golden type for `visible.json`:
  - `snapshot_has_100_lore_records_and_8_speakers` — kinds belief 53, truth 25, clue 10, topic 12 (the fixture's other 25 records are world, people and faction records, which become persona layers, not lore); speakers 8.
  - `visible_matches_golden_for_all_eight_speakers` — for each speaker: `persona` equal in order; `held` and `rejected` equal as sorted lists.
- [ ] **Step 2:** Run `cargo test -p theos-ai --all-features --test lore_poc` — FAIL (no `corpus.json`).
- [ ] **Step 3: Write harness tests** in `convert.rs`: `fixture_converts_to_beliefs_truths_clues_topics_and_eight_speakers` (counts as above); `binder_speaker_has_two_layers_and_throat_kin_lens` (`layers` ids `["fx-binder","throat-kin"]`, `lens.thinks_from` starts with `"The world is thick with Listeners"`); `b942_voice_has_saying_without_header` (`voice` contains `"A binder casts nothing. A binder only sings until something stronger says yes."`, no line contains `[!voice]`); `t913_target_is_never`. Run `cd plugins/llm-poc && cargo test -p npc-poc` — FAIL.
- [ ] **Step 4:** Implement `convert.rs`, `scenarios.rs`, `main.rs` (`export`; `run` prints "not yet" for now). Run harness tests — PASS.
- [ ] **Step 5:** Run `cargo run -p npc-poc -- export`. Check `git status` shows only `fixtures/lore-poc/expected/corpus.json` new (the index cache is ignored).
- [ ] **Step 6:** Run `cargo test -p theos-ai --all-features --test lore_poc` — PASS. If the golden test fails, fix the filter or the conversion, never the golden file.
- [ ] **Step 7:** Root gate 1–4. Commit: `feat(npc-poc): harness export and golden speaker-rule test`.

---

### Task 4: Reply schema and parser

**Files:**
- Create: `theos-ai/src/reply.rs` (unit tests inline). Modify `lib.rs`.

**Interfaces:**
- Produces:
  - `pub const MAX_SOURCES: usize = 5; pub const MAX_REPLY_CHARS: usize = 600;`
  - `Reply { sources: Vec<RecordId>, text: String }`.
  - `pub fn reply_schema(lore_ids: &[RecordId]) -> serde_json::Value` — exactly the spec §4 schema; keys inserted in order `sources`, `reply`.
  - `pub fn parse_reply(raw: &str, lore_ids: &[RecordId]) -> Result<Reply, ReplyError>`.
  - `ReplyError { Malformed(String), UnknownSource(RecordId), EmptyReply, Unfinished(StopReason) }` (`thiserror`; `StopReason` is defined in Task 7 — declare `ReplyError::Unfinished` in Task 7 instead if it does not exist yet).

- [ ] **Step 1: Write the failing tests:**
  - `reply_schema_lists_lore_ids_as_source_enum` — `schema["properties"]["sources"]["items"]["enum"] == json!(["B-945","B-946"])` for input `[B-945, B-946]`; `maxItems == 5`.
  - `reply_schema_puts_sources_before_reply` — `schema["properties"].as_object().unwrap().keys().collect::<Vec<_>>() == ["sources", "reply"]`.
  - `reply_schema_with_no_lore_fixes_sources_empty` — `maxItems == 0`, no `items` key.
  - `reply_schema_bounds_reply_length_and_forbids_extra_keys` — `minLength 1`, `maxLength 600`, `additionalProperties == false`, `required == ["sources","reply"]`.
  - `parse_reply_reads_sources_and_text` — `{"sources":["B-946"],"reply":"Hush."}` → `sources == [B-946]`, `text == "Hush."`.
  - `parse_reply_decodes_escapes_and_ignores_surrounding_whitespace` — `"\n  {\"sources\":[],\"reply\":\"She said \\\"no\\\".\\nÆther – ok\"}  \n"` → text `She said "no".\nÆther – ok`.
  - `parse_reply_rejects_source_outside_context` → `Err(UnknownSource("B-1"))`.
  - `parse_reply_drops_duplicate_sources_keeping_order` — `["B-2","B-1","B-2"]` → `[B-2, B-1]`.
  - `parse_reply_rejects_blank_reply` → `EmptyReply` for `"   "`.
  - `parse_reply_rejects_extra_keys_and_bad_json` → `Malformed(_)`.
- [ ] **Step 2:** Run — FAIL. **Step 3:** Implement (`serde_json` with `preserve_order` in `theos-ai/Cargo.toml`; private `#[serde(deny_unknown_fields)]` wire struct). **Step 4:** Run — PASS.
- [ ] **Step 5:** Gate 1–3. Commit: `feat(npc-poc): JSON reply schema and parser`.

---

### Task 5: Embedder boundary, fake embedder and retrieval

**Files:**
- Create: `theos-ai/src/embed.rs`, `theos-ai/src/retrieval.rs`, `theos-ai/src/testkit.rs` (this task adds `FakeEmbedder`). Modify `lib.rs`, `Cargo.toml` (`[features] testkit = []`).

**Interfaces:**
- Consumes: Task 2 `Corpus`, `Speaker`, `visible`.
- Produces:
  - `EmbedInput<'a> { Query(&'a str), Document(&'a str) }`.
  - `pub trait Embedder { fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError>; }`
  - `EmbedError { Backend(String), CountMismatch { expected: usize, got: usize }, DimensionMismatch { expected: usize, got: usize } }`.
  - `pub fn cosine(a: &[f32], b: &[f32]) -> f32` — 0.0 when either norm is 0.
  - `LoreScope { None, AllVisible, Retrieved { top_k: usize } }` (`Copy`).
  - `Retriever::build(corpus: Corpus, embedder: &mut dyn Embedder) -> Result<Retriever, EmbedError>` — embeds every belief once as `Document(title\nbody\nvoice…)`, in one call; `corpus(&self) -> &Corpus`; `select(&self, speaker: &Speaker, question: &str, scope: LoreScope, embedder: &mut dyn Embedder) -> Result<Vec<RecordId>, EmbedError>`.
  - `testkit::FakeEmbedder` — `new(dims: usize)`, default 64; lowercases, splits on non-alphanumeric, hashes each word (FNV-1a) into a bucket, L2-normalises; same for Query and Document; `calls(&self) -> usize`.

- [ ] **Step 1: Write the failing tests:**
  - `cosine_of_identical_unit_vectors_is_one`; `cosine_with_zero_vector_is_zero`.
  - `fake_embedder_is_deterministic`; `fake_embedder_scores_shared_words_higher` (`"frozen binders in the passes"` closer to `"binders frozen mid-song"` than to `"the songbook ends"`).
  - `build_embeds_only_beliefs` (`test_support::mini()`; a test-local recording embedder → documents embedded == belief count; truth/clue/topic texts never passed).
  - `build_errors_when_embedder_returns_wrong_vector_count` → `CountMismatch`.
  - `select_none_returns_nothing_and_does_not_embed` (`calls` unchanged).
  - `select_all_visible_returns_held_then_rejected`.
  - `select_retrieved_returns_top_k_most_similar_visible_beliefs`.
  - `select_retrieved_never_returns_invisible_belief_even_when_most_similar` (question equals `B-3`'s title).
  - `select_retrieved_breaks_ties_by_id`; `select_ranks_nan_scores_last` (a test embedder that returns a NaN vector for one document).
  - `select_retrieved_with_top_k_larger_than_visible_returns_all_visible`; `select_retrieved_with_top_k_zero_returns_nothing`.
- [ ] **Step 2:** Run — FAIL. **Step 3:** Implement (filter with `visible()` first, then score only `visible.lore()`; sort by score desc with `f32::total_cmp` on a NaN-mapped-to-−∞ key, then id asc). **Step 4:** Run — PASS.
- [ ] **Step 5:** Gate 1–3. Commit: `feat(npc-poc): embedder boundary, fake embedder, filtered retrieval`.

---

### Task 6: Context assembler and the leak gate test

**Files:**
- Create: `theos-ai/src/context.rs` (unit tests inline). Modify `lib.rs`, `theos-ai/tests/lore_poc.rs`.

**Interfaces:**
- Consumes: Tasks 2, 4, 5.
- Produces:
  - `ContextOptions { persona_bodies: bool }` (`Default` → `true`).
  - `Prompt { system: String, user: String }`.
  - `Context { prompt: Prompt, lore_ids: Vec<RecordId>, schema: serde_json::Value }`.
  - `pub fn assemble(corpus: &Corpus, speaker: &Speaker, question: &str, lore_ids: &[RecordId], options: &ContextOptions) -> Context` — renders only ids that `visible(corpus, speaker).contains_lore`, keeping the given order; `Context::lore_ids` lists exactly the rendered ids; `schema = reply_schema(&lore_ids)`; `user = question`.

System prompt — exact copy; `{…}` are substitutions; omit a line whose field is `None`; omit "Your faction"/"Your people" lines for absent layers; bodies only when `persona_bodies`:

```
You are {layers[0].title}, {layers[0].role}.
{layers[0].body}
Your faction: {faction.title}. {faction.body}
Your people: {people.title}. {people.body}
Your people see the world this way: {lens.thinks_from}.
Your stance: {lens.stance}.
You speak to: {lens.speaks_to}.

Rules:
- Speak as yourself, in the first person, to the traveller in front of you, in a few sentences.
- You know only what is written under "What you know" and "What you reject". If that does not answer the question, say in your own voice that you do not know, and invent nothing.
- You know the claims under "What you reject" and you argue against them.
- You live inside this world. If you are asked about anything outside it, stay in the world and do not follow.
- Never say the bracketed record ids aloud.
- Answer in JSON: first "sources", the ids of the records you draw on (an empty list if none), then "reply", what you say.

What you know:
[{id}] {title}
{body}
> {voice line}
…
What you reject:
[{id}] {title}
…
```

With no held records the "What you know" section reads `Nothing beyond the ways of your people.`; the "What you reject" section is omitted when empty.

- [ ] **Step 1: Write the failing unit tests:**
  - `assemble_names_speaker_role_and_lens`; `assemble_includes_persona_bodies_by_default`; `assemble_omits_persona_bodies_when_disabled`.
  - `assemble_lists_held_under_what_you_know_with_bracketed_ids`; `assemble_lists_rejected_under_what_you_reject`.
  - `assemble_drops_lore_ids_the_speaker_may_not_see` (pass `B-3`, `T-1` → neither id nor title in `system`, not in `lore_ids`).
  - `assemble_puts_question_in_user`; `assemble_schema_allows_only_rendered_lore_ids`; `assemble_with_no_lore_says_nothing_beyond_and_fixes_sources_empty`.
- [ ] **Step 2: Add the gate test** `no_forbidden_record_reaches_any_prompt` to `tests/lore_poc.rs` (reads `scenarios.toml` with `toml`): for every scenario × `[None, AllVisible, Retrieved { top_k }]` × `[FakeEmbedder::new(64), Adversarial]` × `persona_bodies [true, false]`: `ids = retriever.select(..)`, `ctx = assemble(..)`; assert every `ctx.lore_ids` id ∈ golden held ∪ rejected; no `forbid` id; no record of kind truth/clue/topic; no truth with `target == Some("never")`; no `lens.unaware_of` id; and `ctx.prompt.system` contains neither the id (as `[id]`) nor the title of any lore record outside the speaker's golden visible set. `Adversarial` is a test-local `Embedder` returning `e0` for the query and for any document whose text starts with the title of a record in the current scenario's `forbid`, `e1` otherwise.
- [ ] **Step 3:** Run — FAIL. **Step 4:** Implement `assemble`. **Step 5:** Run unit + gate tests — PASS.
- [ ] **Step 6:** Gate 1–4. Commit: `feat(npc-poc): context assembler; gate test proves no forbidden record reaches a prompt`.

---

### Task 7: Backend boundary, clock, memory probe, test doubles and the request driver

**Files:**
- Create: `theos-ai/src/backend.rs`, `src/clock.rs`, `src/memory.rs`, `src/driver.rs` (unit tests in `driver.rs`). Extend `src/testkit.rs`. Modify `lib.rs`, `reply.rs` (`Unfinished`).

**Interfaces:**
- Consumes: Tasks 2–6.
- Produces:
  - `Thinking { Off, On }`, `Phase { Thinking, Answer }` (`Copy`, `Debug`, `PartialEq`).
  - `Piece { text: String, phase: Phase }` — exactly one generated token.
  - `GenerationRequest<'a> { prompt: &'a Prompt, schema: &'a serde_json::Value, thinking: Thinking }`.
  - `pub trait Backend { fn begin<'a>(&'a mut self, request: &GenerationRequest<'_>) -> Result<Box<dyn Decoder + 'a>, BackendError>; }`
  - `pub trait Decoder { fn prompt_tokens(&self) -> u32; fn next(&mut self) -> Result<Option<Piece>, BackendError>; }`
  - `BackendError { Failed(String), ContextFull }`.
  - `pub trait Clock { fn now(&self) -> Instant; }`, `SystemClock`.
  - `pub trait MemoryProbe { fn vram_bytes(&self) -> Option<u64>; }`, `NoProbe` (always `None`).
  - `StopReason { Finished, Deadline, TokenCap, ContextFull }`.
  - `Metrics { prompt_tokens: u32, thinking_tokens: u32, answer_tokens: u32, retrieval: Duration, time_to_first_token: Option<Duration>, tokens_per_second: f32, total: Duration, vram_peak_bytes: Option<u64>, context_ids: Vec<RecordId>, cited_ids: Vec<RecordId>, stop: StopReason }`.
  - `NpcRequest { speaker: RecordId, question: String, scope: LoreScope, thinking: Thinking, deadline: Duration }`.
  - `NpcResponse { reply: Result<Reply, ReplyError>, raw: String, thinking: String, metrics: Metrics }` — `raw` is the answer-phase text, `thinking` the thinking-phase text.
  - `NpcParts { backend: Box<dyn Backend>, embedder: Box<dyn Embedder>, corpus: Corpus, speakers: Vec<Speaker>, options: ContextOptions, clock: Box<dyn Clock>, probe: Box<dyn MemoryProbe> }`.
  - `Npc::new(parts: NpcParts) -> Result<Npc, NpcError>` (builds the `Retriever`); `Npc::answer(&mut self, request: &NpcRequest) -> Result<NpcResponse, NpcError>`.
  - `NpcError { UnknownSpeaker(RecordId), DuplicateSpeaker(RecordId), Embed(EmbedError), Backend(BackendError) }` — must be `Send`.
  - `pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(3); pub const ANSWER_TOKEN_CAP: u32 = 256;`
  - testkit: `FakeClock` (`Clone`, shared offset; `new()`, `advance(Duration)`, `impl Clock`); `Script { prompt_tokens: u32, prefill: Duration, per_token: Duration, pieces: Vec<Piece> }`; `ScriptedBackend::new(clock: FakeClock)`, `push(Script)`, `requests(&self) -> Vec<(Prompt, serde_json::Value, Thinking)>` (shared, readable after the backend moves into `Npc`); `begin` errors `Failed("no script")` when empty, advances the clock by `prefill`; `next` advances by `per_token` before returning each piece and does not advance when it returns `None`; `FakeProbe::new(Vec<u64>)` returns the samples in turn, then the last.

Driver loop (the part the tests do not fully determine):

```
t0 = clock.now(); samples = [probe]
speaker lookup → UnknownSpeaker
ids = retriever.select(...)?          ; retrieval = now − t0
ctx = assemble(...)
decoder = backend.begin(&GenerationRequest { &ctx.prompt, &ctx.schema, thinking })?; samples += probe
loop {
  if now − t0 ≥ deadline        → Deadline
  if answer_tokens ≥ CAP        → TokenCap
  match decoder.next() { Ok(None) → Finished; Err(ContextFull) → ContextFull; Err(e) → return Err(Backend(e));
    Ok(Some(p)) → first/last token time = now; count by phase; append text to raw or thinking }
}
samples += probe; total = now − t0
tokens_per_second = generated ≥ 2 && span > 0 ? (generated − 1) / span_secs : 0.0
reply = stop == Finished ? parse_reply(&raw, &ctx.lore_ids) : Err(Unfinished(stop))
cited_ids = reply.sources or []
```

- [ ] **Step 1: Write the failing tests** (`driver.rs`; mini corpus, `FakeEmbedder`, `ScriptedBackend`, `FakeClock`):
  - `answer_returns_parsed_reply_and_cited_ids` — script pieces spell `{"sources":["B-1"],"reply":"Aye."}` → `reply.text == "Aye."`, `metrics.cited_ids == [B-1]`, `stop == Finished`.
  - `answer_records_context_ids_from_retrieval` — `AllVisible` → `metrics.context_ids == ctx.lore_ids`.
  - `answer_passes_prompt_schema_and_thinking_to_backend` — `requests()[0]` has `Thinking::On` and the schema's enum lists the context ids.
  - `answer_measures_time_to_first_token_total_and_tokens_per_second` — prefill 200 ms, 11 pieces at 10 ms → `ttft == Some(210 ms)`, `total == 310 ms`, `tokens_per_second == 100.0`.
  - `answer_stops_at_deadline_between_tokens` — prefill 100 ms, 400 pieces at 20 ms, deadline 3 s → `stop == Deadline`, `answer_tokens == 145` (stays under the 256 cap), `reply == Err(Unfinished(Deadline))`, `total == 3 s`.
  - `deadline_passed_during_prefill_generates_no_tokens` — prefill 3.5 s → `answer_tokens == 0`, `ttft == None`, `tokens_per_second == 0.0`.
  - `single_token_reports_zero_tokens_per_second`.
  - `answer_stops_at_token_cap` — 300 pieces, generous deadline → `answer_tokens == 256`, `stop == TokenCap`.
  - `answer_counts_thinking_and_answer_tokens_separately_and_parses_only_answer` — 5 thinking pieces then JSON pieces → `thinking_tokens == 5`, `thinking` holds their text, reply parses.
  - `context_full_stops_with_reason_not_error`.
  - `answer_keeps_highest_vram_sample` — `FakeProbe::new(vec![1, 5, 3])` → `Some(5)`.
  - `answer_errors_on_unknown_speaker`; `new_errors_on_duplicate_speaker`.
  - `unparsable_reply_is_a_reply_error_not_an_npc_error`.
- [ ] **Step 2:** Run — FAIL. **Step 3:** Implement. **Step 4:** Run — PASS.
- [ ] **Step 5:** Gate 1–4. Commit: `feat(npc-poc): backend boundary and request driver with deadline and metrics`.

---

### Task 8: Worker thread

**Files:**
- Create: `theos-ai/src/worker.rs` (unit tests inline). Modify `lib.rs`.

**Interfaces:**
- Consumes: Task 7 `Npc`, `NpcRequest`, `NpcResponse`, `NpcError`.
- Produces:
  - `Ticket(u64)` (`Copy, Debug, PartialEq, Eq, Hash`), `WorkerGone` (error).
  - `NpcWorker::spawn<F>(factory: F) -> Result<NpcWorker, NpcError> where F: FnOnce() -> Result<Npc, NpcError> + Send + 'static` — runs the factory on a new `std::thread`, waits for it to report ready or the error.
  - `submit(&mut self, request: NpcRequest) -> Result<Ticket, WorkerGone>`; `poll(&self) -> Option<(Ticket, Result<NpcResponse, NpcError>)>` (non-blocking); `wait(&self) -> Option<(Ticket, Result<NpcResponse, NpcError>)>` (blocking; `None` when the worker is gone).
  - `Drop`: closes the request channel, then joins.

- [ ] **Step 1: Write the failing tests** (factory builds an `Npc` from testkit parts; `FakeClock` so nothing sleeps):
  - `spawn_runs_factory_on_worker_thread` (factory sends `thread::current().id()` out; differs from the test thread).
  - `spawn_returns_factory_error` (`UnknownSpeaker` from the factory comes back from `spawn`).
  - `submit_then_wait_returns_response_with_same_ticket`.
  - `responses_come_back_in_submit_order` (three requests → tickets 0, 1, 2).
  - `poll_returns_none_when_nothing_submitted`; `poll_eventually_returns_the_response` (bounded loop, ≤ 2 s real time).
  - `drop_with_request_in_flight_joins` (submit, drop immediately; test finishes).
- [ ] **Step 2:** Run — FAIL. **Step 3:** Implement with `std::sync::mpsc`. **Step 4:** Run — PASS.
- [ ] **Step 5:** Full gate 1–5 (coverage ≥ 85 % for `theos-ai`; add tests where it falls short). Commit: `feat(npc-poc): inference worker thread`.

---

### Task 9: llama.cpp backend — generator, embedder, VRAM probe

Outside the gate; run its tests on this machine (they need the GGUF files and the GPU).

**Files:**
- Modify: `plugins/llm-poc/theos-ai-llama/Cargo.toml` (add `theos-ai = { path = "../../../theos-ai" }`, `thiserror`), `src/lib.rs`.
- Create: `src/generator.rs`, `src/embedder.rs`, `src/vram.rs`, `tests/real_model.rs`; create `plugins/llm-poc/models.toml`.

**Interfaces:**
- Consumes: Task 7 `Backend`, `Decoder`, `Piece`, `Phase`, `Thinking`, `GenerationRequest`, `BackendError`, `MemoryProbe`; Task 5 `Embedder`, `EmbedInput`, `EmbedError`.
- Produces:
  - `LlamaRuntime::init() -> Result<Arc<LlamaRuntime>, LlamaError>` — one per process (wraps `LlamaBackend`).
  - `Sampling { temperature: f32, top_p: f32, top_k: i32, presence_penalty: f32 }`; `ModelProfile { thinking_off_prefix: String, thinking_on_prefix: String, sampling_off: Sampling, sampling_on: Sampling, seed: u32 }` (serde, read from `models.toml`).
  - `LlamaGenerator::load(rt: Arc<LlamaRuntime>, path: &Path, profile: ModelProfile) -> Result<LlamaGenerator, LlamaError>` (all layers on GPU, `n_ctx` 8192, `n_batch` 8192); `n_params(&self) -> u64`; `impl Backend`.
  - `pub fn render_prompt(templated: &str, thinking: Thinking, profile: &ModelProfile) -> String` — `templated` + the matching prefix.
  - `LlamaEmbedder::load(rt: Arc<LlamaRuntime>, path: &Path) -> Result<LlamaEmbedder, LlamaError>` (embeddings on, `LlamaPoolingType::Last`); `impl Embedder`; `pub const QUERY_INSTRUCTION: &str = "Instruct: Given a player's question to a character, retrieve the lore that character would draw on\nQuery: ";`; `pub fn l2_normalize(v: &mut [f32])`.
  - `DxgiProbe::new() -> Option<DxgiProbe>` (NVIDIA adapter, vendor `0x10DE`); `impl MemoryProbe` returning local-segment `CurrentUsage`. If the spike found DXGI blind to CUDA, implement `NvmlProbe` instead (device used − baseline at `new()`, `nvml-wrapper` crate) and note it in `models.toml`'s comment and the results header.
  - `models.toml`:

```toml
embedder = "Qwen3-Embedding-0.6B-Q8_0.gguf"

[[model]]
name = "Qwen3.5-4B Q4_K_M"
file = "Qwen3.5-4B-Q4_K_M.gguf"
thinking_off_prefix = "<think>\n\n</think>\n\n"
thinking_on_prefix = "<think>\n"
seed = 42
sampling_off = { temperature = 0.7, top_p = 0.8, top_k = 20, presence_penalty = 1.5 }
sampling_on = { temperature = 1.0, top_p = 0.95, top_k = 20, presence_penalty = 1.5 }
```

Generator behaviour:
- `begin`: `apply_chat_template(model.chat_template(None)?, [system, user], add_ass = true)` → `render_prompt` → tokenize (`AddBos::Always`, special tokens parsed) → error `ContextFull` if prompt ≥ `n_ctx` → decode the prompt in one batch → build the sampler chain. Thinking off: `[llguidance(schema), penalties(64, 1.0, 0.0, presence), top_k, top_p, temp, dist(seed)]`. Thinking on: the same chain without llguidance.
- `next`: sample, accept, decode one token; `Ok(None)` on an end-of-generation token; `ContextFull` when the context is full. Thinking on: emit `Phase::Thinking` pieces until the accumulated thinking text ends with `</think>`; then decode the tokens of `"\n\n"` without emitting them, rebuild the chain with llguidance, and emit `Phase::Answer` pieces.
- Embedder: one sequence per input; query text = `QUERY_INSTRUCTION + q`; document text as given; ensure the token sequence ends with `<|endoftext|>` (append it when the tokenizer did not); clear the KV cache between inputs; `l2_normalize` the result.

- [ ] **Step 1: Write the failing tests.** Unit (in `generator.rs`/`embedder.rs`): `render_prompt_appends_thinking_off_prefix`, `render_prompt_appends_thinking_on_prefix`, `l2_normalize_yields_unit_length`, `l2_normalize_leaves_zero_vector_zero`, `models_toml_parses_profile`. Real-model (`tests/real_model.rs`; fail with a clear message naming `THEOS_MODELS_DIR` when the files are missing): `generator_reply_parses_with_theos_ai_parse_reply` (thinking off, schema with `[B-945, B-946]`, `parse_reply` is `Ok`), `generator_emits_thinking_pieces_then_answer_when_thinking_on` (stop after the first `Answer` piece or 2000 pieces; at least one `Thinking` piece precedes it), `embedder_returns_unit_vectors_of_dimension_1024`, `embedder_ranks_related_lore_above_unrelated` (query "Which spirit should a beginner learn on?" scores B-947's text above B-946's), `vram_probe_reads_more_after_loading_the_generator`.
- [ ] **Step 2:** Run `cd plugins/llm-poc && cargo test --release -p theos-ai-llama` — FAIL. **Step 3:** Implement. **Step 4:** Run — PASS.
- [ ] **Step 5:** `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` inside `plugins/llm-poc`. Commit: `feat(npc-poc): llama.cpp generator, embedder and VRAM probe`.

---

### Task 10: Harness `run` — scoring and results tables

**Files:**
- Create: `plugins/llm-poc/npc-poc/src/score.rs`, `src/report.rs`, `src/run.rs`; modify `src/main.rs`, `Cargo.toml` (add `theos-ai-llama = { path = "../theos-ai-llama" }`, `regex`, `csv`).

**Interfaces:**
- Consumes: Tasks 3, 7, 8, 9.
- Produces:
  - `score::Checks { retrieval: Option<bool>, leak: bool, forbidden: bool, words: Vec<String> /* forbidden words found */, grounding: bool, deadline: bool, vram: Option<bool> }` and `score::check(scenario: &Scenario, global_words: &[String], visible: &VisibleEntry, scope: LoreScope, response: &NpcResponse, deadline: Duration) -> Checks`.
  - Words rule: each word or phrase → `(?i)\b` + words joined by `\s+` + `\b`; plurals are not matched because `\b` does not match inside `nodes`.
  - Grounding rule: spec §9 table; for `answer`/`rumour` with `expect` empty, cites ⊆ `also_ok` ∪ `rejects` is enough.
  - VRAM rule: `Some(peak ≤ 6 × 1024³)`, `None` when no sample.
  - `run::Row { model, thinking, scope, scenario_id, npc, behaviour, checks, metrics, reply_text, raw, hand_read: bool }`.
  - `run::run_model(worker: &mut NpcWorker, model: &str, scenarios: &Scenarios, visible: &BTreeMap<String, VisibleEntry>, deadline: Duration) -> Vec<Row>` — thinking `[Off, On]` × scenarios × scopes `[None, AllVisible, Retrieved { top_k }]`, one request at a time (submit, wait).
  - `report::write(dir: &Path, header: &RunHeader, rows: &[Row]) -> anyhow::Result<()>` → `results.md` (header; summary per thinking × scope: pass counts of each check, median/p95 time to first token and total, median tokens per second, median prompt tokens, peak VRAM; then one row per request), `results.csv` (every field, reply text included), `answers.md` (per scenario: question, behaviour, note, each condition's reply and cites; "read by hand" marks).
  - `RunHeader { model, file_bytes, n_params, gpu, deadline, seed, persona_bodies, probe }`.
  - CLI `npc-poc run [--models models.toml] [--deadline-ms 3000] [--no-persona-bodies]` — refuses to start when a fresh `load_fixture` ≠ committed `corpus.json` or any speaker's `visible()` ≠ `visible.json`; creates one `Arc<LlamaRuntime>`; per model spawns an `NpcWorker` whose factory loads embedder + generator + probe; writes to `plugins/llm-poc/results/<unix-seconds>-<model-slug>[-no-persona-bodies]/`; prints the summary table to stdout.

- [ ] **Step 1: Write the failing tests:**
  - `score.rs`: `words_match_whole_words_in_any_case` (`"A NODE hums"` finds `node`); `words_do_not_match_plurals` (`"Sub-Clavicular Nodes"` finds nothing); `words_match_phrases_across_whitespace` (`"out  of\nphase"`); `grounding_answer_needs_an_expect_id`; `grounding_answer_fails_on_cite_outside_allowed`; `grounding_answer_with_empty_expect_accepts_rejects_cite` (s14 shape); `grounding_unaware_passes_with_no_cites_or_only_also_ok` (s13 shape); `grounding_deflect_fails_on_cite_outside_also_ok`; `retrieval_needs_every_expect_and_rejects_id` (only for `Retrieved`, `None` otherwise); `leak_fails_on_id_outside_visible`; `vram_is_none_without_sample_and_fails_above_six_gib`.
  - `run.rs`: `run_model_produces_one_row_per_thinking_scenario_and_scope` — `NpcWorker` built from testkit parts (scripted replies, `FakeClock`), three scenarios → 18 rows.
  - `report.rs`: `csv_quotes_replies_with_commas_and_newlines`; `summary_counts_passes_per_thinking_and_scope`.
- [ ] **Step 2:** Run `cargo test -p npc-poc` — FAIL. **Step 3:** Implement. **Step 4:** Run — PASS.
- [ ] **Step 5:** fmt + clippy inside `plugins/llm-poc`. Commit: `feat(npc-poc): harness run, scoring and results tables`.

---

### Task 11: First run, gates, graph

- [ ] **Step 1:** `cd plugins/llm-poc && cargo run --release -p npc-poc -- run` — expect 192 requests and a results dir. If any leak or forbidden check fails, stop: that is a defect, not a result.
- [ ] **Step 2:** Root gate 1–5 green; `theos-ai` coverage ≥ 85 % region (record the figure here).
- [ ] **Step 3:** `graphify update .` from the repo root.
- [ ] **Step 4:** Commit: `chore(npc-poc): first run green; graph updated`.

Then (controller, per the owner's process): whole-branch review by a fresh reviewer; after each round of fixes a new reviewer until nothing is open; then `run` and `run --no-persona-bodies` on the reviewed code; then stop and show both tables. No tuning before that.

---

## Deferred

- LFM3-1.7B: needs a `llama-cpp-2` release bundling llama.cpp b11960+; then one `[[model]]` entry.
- An extra run with `--deadline-ms` lifted for thinking on, if the owner wants to see thinking's quality without the time limit.
- Persona-body text cleanup (design callouts) if the comparison says it matters.

## Spike results

_(filled in by Task 1)_
