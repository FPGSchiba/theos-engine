# NPC LLM proof of concept — design

**Date:** 2026-11-13 · **Status:** awaiting approval · **Plan (next):** `plans/npc-llm-poc-2026-11-13.md`

## 1. Intent

Find out whether a local model of **4B parameters or less** can give a **grounded NPC answer within 3 seconds** when retrieval picks its context. Answer quality is the goal. Speed and memory are fixed limits: they are measured on every request and reported, never tuned.

The NPC answers from what its people believe. Retrieval filters by what the speaker may know before it ranks by similarity, so a record the speaker may not see can never reach a prompt.

`theos-ai` is the part meant to survive into the engine: its public types get the care. The llama.cpp backend crate and the harness are disposable and stay small.

## 2. Goals, non-goals, anti-goals

**Goals**

- `theos-ai` (workspace member, pure Rust, inside the quality gate): the knowledge filter (the speaker rule), retrieval, the context assembler, the reply format, the backend and embedder boundaries, a scripted backend and a fake embedder for tests, the request driver (deadline, metrics) and the inference worker thread.
- A llama.cpp backend crate outside the workspace: generator and embedder, CUDA, llguidance.
- A harness binary that loads the fixture through the editor's `theos-content`, runs every scenario under three lore conditions per model, with thinking off and on, and writes a results table (Markdown and CSV).
- A test inside the gate that proves no forbidden record reaches a prompt.

**Non-goals (out of scope):** ECS and event-bus wiring, behaviour trees, request priorities, GPU sharing with a renderer, fine-tuning, persistence, HTTP of any kind, tuning speed or prompts before the first table is shown.

**Anti-goals (failure even if it runs):**

- A truth, clue, topic, another people's belief, a `never` truth or an `unaware_of` truth in any prompt.
- `theos-ai` depending on `theos-content` or knowing the content file format.
- The model, the embedder or any blocking call in a frame loop.
- Any network access at build or run time beyond `cargo` fetching crates. Model files are local GGUF, never in git.
- CUDA reaching the workspace gate (`--all-features` would switch it on in CI).

## 3. Fixed decisions (settled before this session — do not reopen)

- Local only. Models are GGUF files on disk, never in git.
- Inference through llama.cpp via the `llama-cpp-2` crate, on its own thread, deadline checked between tokens.
- Output constrained by a grammar through the crate's `llguidance` feature.
- The model never sits in the frame loop.
- `theos-ai` must not depend on `theos-content`. The backend crate and the harness live outside the workspace, like `plugins/theos-audio`.
- Retrieval: filter by the speaker rule (`fixtures/lore-poc/README.md`) before ranking by similarity.
- Measured per request: prompt tokens, time to first token, tokens per second, total time, record ids in the context, record ids the answer cites.
- Thinking off for the main run, plus one comparison run with thinking on.
- Stop and show the first results table before tuning anything.

## 4. Decided in this session

**Embedder (open point 1).** A GGUF embedding model run through the same llama.cpp backend as the generator: **Qwen3-Embedding-0.6B, Q8_0**. One native runtime, on the GPU, sharing the one llama.cpp backend init per process. fastembed is not used. `theos-ai` defines an `Embedder` trait and ships a fake for tests.

**Citations (open point 2).** The reply is a JSON object, constrained by a JSON schema through llguidance and parsed with `serde_json`. `sources` comes first, so the model commits to its records before it speaks; an empty list means "none".

```json
{
  "type": "object",
  "properties": {
    "sources": { "type": "array", "items": { "enum": ["<lore ids in this prompt>"] }, "maxItems": 5 },
    "reply":   { "type": "string", "minLength": 1, "maxLength": 600 }
  },
  "required": ["sources", "reply"],
  "additionalProperties": false
}
```

- The `enum` holds exactly the lore record ids in this prompt. Persona ids (speaker, faction, people) are not citable. With no lore in the prompt, `sources` is fixed to the empty array (`maxItems: 0`, no `items` enum).
- `theos-ai` builds the schema per request and parses the reply. The backend only applies the schema it is given.
- The spike checks that llguidance emits keys in schema order. If it does not, the backend wraps the schema in a short Lark rule that forces `sources` before `reply`.

**Persona bodies.** The bodies of the persona layers (speaker, faction, people) go into the prompt as written, fixture unchanged. An assembler option, exposed as a harness flag, leaves them out, so a second run shows which works better. (The npc bodies open with "Synthetic speaker for the PoC scenarios, not a character of the game", and the Concordance body speaks from outside the world. The comparison shows what that does.)

**Memory.** VRAM is measured on every request. **Limit: 6 GB** (read as 6 GiB) for everything the process holds on the GPU: generator, embedder, KV cache and compute buffers. Reported per request and as a pass/fail column; not tuned.

**Grounding for `unaware` and `deflect`.** These pass when the answer cites nothing or only `also_ok` records (s06, s13 and s22 list some).

## 5. Findings from recon (2026-11-13)

- **Machine:** RTX 5080 16 GB (Blackwell, compute 12.0), driver 610.88, 31 GB RAM. CUDA 13.5, CMake, LLVM 23.1.5 (`LIBCLANG_PATH` set as a user variable) and VS 2022 are now installed. Rust 1.99.1 is installed; the default `stable` is 1.97.1.
- **`llama-cpp-2` 0.1.161** is the newest release. Its bundled llama.cpp has the `qwen35` architecture and **no `lfm3`**, so **LFM3-1.7B is out** for this PoC. It comes back when a release bundles llama.cpp b11960 or newer; it is then one entry in `models.toml`.
- The crate's `apply_chat_template` uses llama.cpp's built-in template renderer, not Jinja, so it cannot pass `enable_thinking`. The backend appends the thinking prefix itself (see §8).
- The llguidance sampler is built from `LlamaSampler::llguidance_tok_env(model)` once per model, then a `ParserFactory` and a `Matcher` per request, converted into a `LlamaSampler`.
- **CUDA 13 risk:** CUDA 13 dropped old GPU architectures. If llama.cpp's default architecture list fails to build, pin `CMAKE_CUDA_ARCHITECTURES=120`.
- **`theos-content`** (FPGSchiba/theos-editor, private, commit `0a6f480`) needs Rust 1.99.1. `Index::open` writes a cache to `<project>/.theos/cache/index.sqlite`.

## 6. Files to download

Put both in **`D:\Projects\theos-engine\models\`** (gitignored; the harness also reads `THEOS_MODELS_DIR`).

| Model | Repository | File | Size |
|---|---|---|---|
| Generator: Qwen3.5-4B, 4-bit | https://huggingface.co/unsloth/Qwen3.5-4B-GGUF | `Qwen3.5-4B-Q4_K_M.gguf` | 2.74 GB |
| Embedder: Qwen3-Embedding-0.6B | https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF | `Qwen3-Embedding-0.6B-Q8_0.gguf` | 639 MB |

The `mmproj-*` vision files in the Qwen3.5 repository are not needed.

## 7. `theos-ai` (the part that survives)

### 7.1 Records

`theos-ai` has its own plain record types. They derive `serde` (`Serialize`, `Deserialize`) so the harness can write a corpus snapshot and the gate test can read it.

```rust
pub struct RecordId(String);                 // "B-944", "fx-binder", "throat-kin"

pub enum LoreKind { Belief, Truth, Clue, Topic }

pub struct LoreRecord {
    pub id: RecordId,
    pub kind: LoreKind,
    pub title: String,
    pub body: String,          // designer voice; safe for prompts in beliefs
    pub voice: Vec<String>,    // in-world lines of the voice blocks, header stripped
    pub held_by: Vec<RecordId>,// holder ids; empty for non-beliefs
    pub target: Option<String>,// a truth's reveal target as written (`confirmed`, `never`, …); None otherwise
}

pub struct PersonaLayer {      // the speaker, then its faction, then its people
    pub id: RecordId,
    pub kind: HolderKind,      // Npc | Faction | People
    pub title: String,
    pub role: Option<String>,
    pub body: String,
}

pub struct Lens {              // the effective lens, inheritance already resolved
    pub thinks_from: Option<String>,
    pub stance: Option<String>,
    pub speaks_to: Option<String>,
    pub tier: Option<String>,
    pub rejects: Vec<RecordId>,
    pub unaware_of: Vec<RecordId>,
}

pub struct Speaker { pub id: RecordId, pub layers: Vec<PersonaLayer>, pub lens: Lens }

pub struct Corpus { /* records by id; constructor rejects duplicate ids */ }
```

The harness resolves the speaker chain and effective lens with `Index::lens`, as the editor does. `theos-ai` decides which records a speaker may see (§7.2). Field names above are the intent; the builder may adjust names but not the split.

### 7.2 Knowledge filter (the speaker rule)

`visible(corpus, speaker) -> Visible { persona, held, rejected }`:

- **persona:** the speaker's layers.
- **held:** every `Belief` whose `held_by` contains the id of any layer.
- **rejected:** every id in `lens.rejects` that is a `Belief` in the corpus.
- Second guard: drop any record that is not a `Belief`, and any id in `lens.unaware_of`.

Only `held` and `rejected` records are retrievable. Truths, clues and topics are never embedded.

### 7.3 Embedder and retrieval

```rust
pub enum EmbedInput<'a> { Query(&'a str), Document(&'a str) }
pub trait Embedder { fn embed(&mut self, inputs: &[EmbedInput]) -> Result<Vec<Vec<f32>>, EmbedError>; }
```

The model-specific prompt format (Qwen3-Embedding's query instruction) lives in the backend's implementation, not in `theos-ai`.

- At start-up the retriever embeds every belief once (document text = title, body, voice lines).
- `LoreScope` per request: `None` (persona only), `AllVisible` (every held and rejected belief), `Retrieved { top_k }` (embed the question as a query, cosine against the speaker's visible beliefs only, highest `top_k`, ties by id).
- The filter runs before ranking. The ranking never sees a record outside `Visible`.
- `FakeEmbedder` (behind the `testkit` feature, like `theos-ecs`): deterministic, word-hashing bag of words, so similarity tests are meaningful.

### 7.4 Context assembler

It produces a `Prompt { system, user }`, the ordered list of lore ids in the context, and the reply schema for those ids.

- **Option `persona_bodies: bool`** (default `true`): whether each persona layer's body is included.
- **system:** who the speaker is (each layer's title, role and, when enabled, body; the lens's `thinks_from`, `stance`, `speaks_to`); the rules (speak in character to the player; you know only what is listed; if it does not cover the question, say you do not know, in voice, and invent nothing; argue against what you reject; never leave the world; do not say record ids in the reply; answer in the JSON format, sources first); then "What you know" (`[B-944] title`, body, voice lines) and "What you reject".
- **user:** the player's question.
- Not in the prompt: the forbidden word list (naming the words would plant them), truths, clues, topics.
- The exact wording is the builder's. It is written once and not tuned before the first table.

### 7.5 Reply

```rust
pub struct Reply { pub sources: Vec<RecordId>, pub text: String }
pub fn reply_schema(context_ids: &[RecordId]) -> serde_json::Value;
pub fn parse_reply(raw: &str, context_ids: &[RecordId]) -> Result<Reply, ReplyError>;
```

`parse_reply` checks that every source is in `context_ids` (the scripted backend is not grammar-bound) and drops duplicates.

### 7.6 Backend boundary

```rust
pub trait Backend {
    /// Tokenises and prefills; returns a decoder for this request.
    fn begin<'a>(&'a mut self, request: &GenerationRequest) -> Result<Box<dyn Decoder + 'a>, BackendError>;
}
pub struct GenerationRequest<'a> { pub prompt: &'a Prompt, pub schema: &'a serde_json::Value, pub thinking: Thinking }
pub trait Decoder {
    fn prompt_tokens(&self) -> u32;
    /// The next piece, or None at end of generation.
    fn next(&mut self) -> Result<Option<Piece>, BackendError>;
}
pub struct Piece { pub text: String, pub phase: Phase }   // Phase::Thinking | Phase::Answer
```

The backend stays thin: prefill and one token per `next`. `theos-ai` owns the loop, so the deadline, the token cap and the timings are the same for every backend and are tested with the scripted backend.

`ScriptedBackend` (behind `testkit`): returns canned pieces per request, with a set time per piece on an injected clock.

### 7.7 Request driver and metrics

One request: start the clock → retrieve → assemble → `begin` (prefill) → loop `next`, **checking the deadline before every token** → parse the answer pieces.

- **Deadline:** default 3 s, from request start. When it passes, generation stops and the request is marked `Deadline`; a cut-off reply is not parsed.
- **Token cap:** 256 answer tokens.
- **Clock:** a `Clock` trait with a real and a fake clock.
- **VRAM:** a `MemoryProbe` trait (`fn vram_bytes(&self) -> Option<u64>`), sampled before the request, after prefill and after the last token. `Metrics` keeps the highest sample. A probe that cannot read returns `None`, and the table shows "n/a" rather than a pass.

```rust
pub struct Metrics {
    pub prompt_tokens: u32,
    pub thinking_tokens: u32,
    pub answer_tokens: u32,
    pub retrieval: Duration,          // query embedding + ranking
    pub time_to_first_token: Duration,// request start → first generated token of any phase
    pub tokens_per_second: f32,       // (generated − 1) / (last token − first token)
    pub total: Duration,              // request start → end
    pub vram_peak_bytes: Option<u64>, // highest MemoryProbe sample during the request
    pub context_ids: Vec<RecordId>,
    pub cited_ids: Vec<RecordId>,
    pub stop: StopReason,             // Finished | Deadline | TokenCap
}
```

### 7.8 Worker thread

`NpcWorker::spawn(factory)` starts a dedicated `std::thread`. The factory runs on that thread and builds the backend, the embedder and the retriever (llama contexts are not `Send`). `submit(NpcRequest) -> Ticket`, non-blocking `poll() -> Option<(Ticket, NpcResponse)>` for a frame loop, blocking `wait` for the harness. Plain `std::sync::mpsc`, no async runtime.

```rust
pub struct NpcRequest { pub speaker: RecordId, pub question: String, pub scope: LoreScope, pub thinking: Thinking, pub deadline: Duration }
pub struct NpcResponse { pub reply: Result<Reply, ReplyError>, pub raw: String, pub metrics: Metrics }
```

### 7.9 Dependencies and housekeeping

`serde`, `serde_json`, `thiserror` (all workspace dependencies). Dev: `toml` (new workspace dependency) for reading `scenarios.toml`. Remove the template `add` function and its `it_works` test. Keep `components.rs` as is. No `unwrap`/`expect` in production code.

### 7.10 Gate tests that prove the rule

`theos-ai/tests/lore_poc.rs` reads `fixtures/lore-poc/expected/corpus.json` (theos-ai's own format, written by the harness, committed), `expected/visible.json` and `scenarios.toml`:

1. For all 8 speakers, `visible()` equals the golden `persona`, `held` and `rejected`.
2. For all 32 scenarios × `None` / `AllVisible` / `Retrieved{top_k}`, with the fake embedder **and with an adversarial embedder that ranks the scenario's `forbid` records closest:** every context id is in the speaker's golden visible set; no `forbid` id, truth, clue, topic, `never` truth or `unaware_of` id is in the context; and the rendered prompt text contains no id and no title of any **lore** record (truth, clue, topic, or belief) outside the speaker's visible set. World, people and faction ids and titles are left out of the text scan: words like "tower", "hearth" or "Hedge-Scribes" occur legitimately in visible beliefs.

Coverage of `theos-ai` ≥ 85% region (target 90%).

## 8. The llama.cpp backend crate (disposable)

- **Location:** `plugins/llm-poc/`, its own Cargo workspace with members `theos-ai-llama` (library) and `npc-poc` (harness binary). It has its own `Cargo.lock` and a `rust-toolchain.toml` pinning 1.99.1. It is not part of the root or the `plugins/` workspace, so the gate never builds it.
- **Dependency:** `llama-cpp-2 = "=0.1.161"` with features `cuda` and `llguidance`.
- **`LlamaRuntime`:** initialises the llama.cpp backend once, loads the generator and the embedder from paths.
- **Generator** (`impl Backend`): renders the prompt with the model's chat template through llama.cpp's built-in renderer, then appends the thinking prefix from the model's profile.
  - Qwen3.5 with thinking **off**: `<think>\n\n</think>\n\n`, then the JSON-schema sampler from the first token.
  - Thinking **on**: `<think>\n`, sampled without a grammar until `</think>`, then the `\n\n` separator, then the schema sampler.
  - Sampling per mode from the model card, fixed seed: off — temperature 0.7, top-p 0.8, top-k 20, presence penalty 1.5; on — temperature 1.0, top-p 0.95, top-k 20, presence penalty 1.5. Context 8192 tokens.
- **VRAM probe** (`impl MemoryProbe`): this process's own GPU memory, through DXGI `IDXGIAdapter3::QueryVideoMemoryInfo` (local segment, `CurrentUsage`). It is per process on Windows, where NVML's per-process figure reads "not available". The spike checks that it rises by roughly the model size on load. If DXGI does not see CUDA allocations, fall back to NVML device memory in use minus a baseline taken before loading, and note that in the results header.
- **Embedder** (`impl Embedder`): embeddings mode, last-token pooling, L2-normalised. A query gets Qwen3-Embedding's instruction prefix: `Instruct: Given a player's question to a character, retrieve the lore that character would draw on\nQuery: `. Documents get no prefix.

## 9. The harness (disposable)

`npc-poc` uses `theos-content` as a **git dependency pinned to `0a6f480`** (the commit `visible.json` was checked against), with `net.git-fetch-with-cli = true` in the workspace's `.cargo/config.toml`, because the repository is private. It also uses `theos-ai` (path dependency, `testkit` feature).

- **`npc-poc export`:** reads `fixtures/lore-poc/records` with `theos-content`, converts records and speakers into `theos-ai` types, writes `fixtures/lore-poc/expected/corpus.json`.
- **`npc-poc run [--models models.toml] [--deadline-ms 3000] [--no-persona-bodies]`:**
  - Refuses to run if a fresh conversion differs from the committed `corpus.json`, or if `visible()` differs from `visible.json`.
  - For each model in `models.toml`: one worker, then thinking off and thinking on, each over 32 scenarios × `None` / `AllVisible` / `Retrieved{top_k = 5}` = 192 requests per model. Same deadline for both thinking modes.
  - Writes `plugins/llm-poc/results/<timestamp>/` (gitignored): `results.md`, `results.csv`, `answers.md`.
- **`models.toml`** (committed): per model the file name, the thinking prefixes and the sampling; plus the embedder's file name. Paths resolve against `THEOS_MODELS_DIR`, default `<repo>/models`.

**Scoring per request** (from the fixture README):

| Check | Rule |
|---|---|
| Retrieval | `Retrieved` only: every `expect` and `rejects` id is in the context. |
| Leak | Every context id is in the speaker's entry of `visible.json`. |
| Forbidden | No `forbid` id is in the context. |
| Words | The reply contains no global or scenario forbidden word or phrase: whole words, any case, plurals not matched. |
| Grounding | `answer` and `rumour`: cites ⊆ `expect` ∪ `also_ok` ∪ `rejects`, and at least one `expect` id when `expect` is not empty. `unaware` and `deflect`: cites nothing, or only `also_ok` ids. |
| Deadline | Finished within the deadline. |
| VRAM | Peak VRAM of the request ≤ 6 GiB; "n/a" when the probe cannot read. |

`rumour` and `hand_scored` scenarios are flagged "read by hand"; the answers are in `answers.md`.

**`results.md`:**

- A run header: model, file size, parameter count, GPU, deadline, seed, persona bodies on or off, VRAM probe used.
- A summary per model × thinking × scope: pass count of each check; median and p95 of time to first token and total; median tokens per second; median prompt tokens; peak VRAM.
- One row per request.

`results.csv` has every field of every request, including the reply text.

## 10. Order of work

1. Branch and worktree. Commit `fixtures/lore-poc` (untracked today) and the `.gitignore` entries: `/models/`, `*.gguf`, `fixtures/**/.theos/cache/`, `plugins/llm-poc/results/`.
2. **Spike (first milestone):** the `theos-ai-llama` skeleton plus a `spike` example that loads Qwen3.5-4B on the GPU and prints tokens per second. In the same run it applies the reply schema once (checking key order), embeds one sentence and reads the VRAM probe before and after loading. **If it does not build, stop and report.** It is a check, not product code; it has no tests.
3. `theos-ai`, test-driven, through the gate.
4. Backend crate.
5. Harness: `export`, commit `corpus.json`, then `run`.
6. Whole-branch review by a fresh reviewer; after each round of fixes a new reviewer, until nothing is open.
7. Run the harness on the reviewed code, twice: persona bodies on, then `--no-persona-bodies`. Then **stop and show the first results tables.** No tuning before that.

## 11. Left to the builder

Exact names inside the shapes above; the system prompt's wording (written once, not tuned); how the harness reads record fields through `theos-content`; the fake embedder's hashing; the table layout beyond the listed columns; where the corpus snapshot's two-field wrapper struct is declared.

## 12. Calls made by the designer (not discussed with the owner)

1. LFM3 is dropped: the bundled llama.cpp has no `lfm3`.
2. "All lore" means every belief the speaker may see, never the whole corpus. The persona is in all three conditions.
3. Belief bodies go into the prompt next to title and voice lines. Only 7 of 53 beliefs have voice blocks.
4. The thinking-on run keeps the 3 s deadline. `--deadline-ms` lifts it for an extra run.
5. Time to first token and total time count from request start, including retrieval.
6. The gate test reads a committed `corpus.json` that the harness exports from the fixture; the harness refuses to run when the snapshot is stale.
7. Sampling from the model card with a fixed seed; context 8192; 256 answer tokens.
8. The spike also checks the schema's key order and the embedder.
9. The crate locations: `plugins/llm-poc/` as its own workspace, with its own toolchain pin.
10. `theos-content` comes in as a git dependency pinned to `0a6f480`.
11. Results are gitignored; the first tables are shown in chat.
12. "6 GB" is read as 6 GiB (6 × 1024³ bytes), and covers everything the process holds on the GPU, the embedder included.
13. VRAM is read per process through DXGI, with NVML device-minus-baseline as the fallback.
14. Persona bodies go in as written; the fixture is not edited.
15. The prompt-text leak scan covers lore records only; records carry a `target` field so the test can check `never` truths.
