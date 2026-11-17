//! The run loop: every scenario under every condition, one row per request.
//!
//! A request that fails in any way (generation error, a full context, a missed deadline, a
//! `score.py` crash) is still a row: nothing but a write to the results folder ends a run.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context};
use clap::ValueEnum;
use guard_kit::{Guards, Tag};
use serde_json::{json, Value};
use theos_ai::{
    GenerateJob, LoreScope, Metrics, ModerationSource, ModerationVerdict, Npc, NpcError, NpcParts,
    RecordId, StopReason, SystemClock, Thinking,
};

use crate::fixture::{BtScenario, Fixture};
use crate::prompt::{self, RenderedPrompt};
use crate::report::{self, RunHeader};
use crate::schema::tree_schema;
use crate::score::{self, PyScore};

/// Most answer-phase tokens one tree may take.
pub const TOKEN_CAP: u32 = 1_536;
/// How many lore beliefs the retrieved scope shows.
pub const TOP_K: usize = 5;
/// Words a tree name may share with the prompt before it counts as copied.
const NAME_OVERLAP_WORDS: usize = 8;
/// Said to the NPC (and used as the retrieval query) when a scenario has no directive.
const DEFAULT_DIRECTIVE: &str = "Act on your record and the situation.";

/// What retrieval is queried with for an injected scenario.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum QueryMode {
    /// The scenario's own directive, injection included (the realistic game case).
    #[default]
    Injected,
    /// The directive of its base scenario, so the lore set matches the clean row. The prompt
    /// still holds the injection.
    Base,
}

impl QueryMode {
    /// How the results header words the choice.
    pub fn label(self) -> &'static str {
        match self {
            QueryMode::Injected => "injected directive",
            QueryMode::Base => "base directive",
        }
    }
}

/// Which grammar conditions a run covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum GrammarModes {
    /// Constrained by the schema only.
    On,
    /// Unconstrained only.
    Off,
    /// Both, on first.
    #[default]
    Both,
}

impl GrammarModes {
    fn modes(self) -> &'static [bool] {
        match self {
            Self::On => &[true],
            Self::Off => &[false],
            Self::Both => &[true, false],
        }
    }
}

/// Which thinking modes a run covers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum ThinkingModes {
    /// Thinking off only.
    #[default]
    Off,
    /// Thinking on only.
    On,
    /// Both, off first.
    Both,
}

impl ThinkingModes {
    fn modes(self) -> &'static [bool] {
        match self {
            Self::Off => &[false],
            Self::On => &[true],
            Self::Both => &[false, true],
        }
    }
}

/// Everything a run is parameterised by, apart from the model.
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// The bt-poc fixture directory (for `score.py`).
    pub fixture_dir: PathBuf,
    /// The scenarios file (for `score.py`).
    pub scenarios_file: PathBuf,
    /// Grammar conditions.
    pub grammar: GrammarModes,
    /// Thinking conditions.
    pub thinking: ThinkingModes,
    /// One deadline for every request; `None` uses the scenario's priority budget.
    pub deadline: Option<Duration>,
    /// Thinking tokens allowed to a `CombatTactical` request.
    pub budget_combat: u32,
    /// Thinking tokens allowed to a `WorldEvent` request.
    pub budget_world: u32,
    /// Whether persona bodies go into NPC prompts.
    pub persona_bodies: bool,
    /// What retrieval is queried with for an injected scenario.
    pub query: QueryMode,
    /// The directive of every base scenario by id (read when `query` is `Base`).
    pub base_directives: BTreeMap<String, String>,
    /// Scenario ids to run; empty runs all.
    pub only: Vec<String>,
    /// Which guards are on. `g1` and the moderator are set up where the `Npc` is built;
    /// the run loop applies `g4` and the tree-name check.
    pub guards: Guards,
    /// Set to the request's key before each request, for the moderation review page.
    pub tag: Tag,
}

/// One request and what came of it.
#[derive(Clone, Debug)]
pub struct Row {
    /// Scenario id.
    pub scenario_id: String,
    /// The clean scenario an injected one derives from.
    pub base_id: Option<String>,
    /// The actor.
    pub actor: String,
    /// `WorldEvent` or `CombatTactical`.
    pub priority: String,
    /// Grammar on.
    pub grammar: bool,
    /// Thinking on.
    pub thinking: bool,
    /// `none`, `all-visible` or `retrieved`.
    pub scope: String,
    /// The lore ids the prompt held.
    pub lore_ids: Vec<String>,
    /// Whether the actor is a lore-poc speaker (rather than an enemy).
    pub npc: bool,
    /// Whether the reply held a JSON object to score.
    pub envelope_ok: bool,
    /// What `score.py` made of the reply; `None` when it could not run.
    pub py: Option<PyScore>,
    /// Every cited source was in the prompt.
    pub sources_ok: Option<bool>,
    /// No lore record in the prompt was hidden from the actor.
    pub leak_ok: Option<bool>,
    /// The tree name copies the prompt.
    pub name_overlap: bool,
    /// The priority's budget, seconds.
    pub budget_seconds: f64,
    /// The generation finished within the budget.
    pub budget_ok: bool,
    /// What the generation measured; `None` when none ran.
    pub metrics: Option<Metrics>,
    /// Why the request failed, when it did.
    pub error: Option<String>,
    /// Injection technique, for injected variants.
    pub technique: Option<String>,
    /// Injection severity, for injected variants.
    pub severity: Option<String>,
    /// Where the injection sits.
    pub injected_in: Option<String>,
    /// The moderator stopped the request: `player:<category id>` (the player's text) or
    /// `reply:<category id>` (the tree's name). Such a tree is never scored.
    pub moderated: Option<String>,
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

fn scope_name(scope: LoreScope) -> &'static str {
    match scope {
        LoreScope::None => "none",
        LoreScope::AllVisible => "all-visible",
        LoreScope::Retrieved { .. } => "retrieved",
    }
}

impl Row {
    /// The file stem of this request's tree.
    pub fn key(&self) -> String {
        format!(
            "{}-grammar-{}-thinking-{}-{}",
            self.scenario_id,
            on_off(self.grammar),
            on_off(self.thinking),
            self.scope
        )
    }

    /// The row as one JSON object.
    pub fn to_json(&self) -> Value {
        let py = self.py.as_ref().map(|p| {
            json!({
                "gate_pass": p.gate_pass, "gate_rule": p.gate_rule,
                "gate_message": p.gate_message, "nodes": p.nodes, "depth": p.depth,
                "checks": p.checks, "behaviour": p.behaviour, "grounding": p.grounding,
            })
        });
        let metrics = self.metrics.as_ref().map(|m| {
            json!({
                "prompt_tokens": m.prompt_tokens, "thinking_tokens": m.thinking_tokens,
                "answer_tokens": m.answer_tokens,
                "ttft_ms": m.time_to_first_token.map(|d| d.as_secs_f64() * 1000.0),
                "ttfa_ms": m.time_to_first_answer_token.map(|d| d.as_secs_f64() * 1000.0),
                "tokens_per_second": m.tokens_per_second,
                "total_ms": m.total.as_secs_f64() * 1000.0,
                "vram_peak_bytes": m.vram_peak_bytes,
                "stop": format!("{:?}", m.stop),
            })
        });
        json!({
            "key": self.key(), "scenario_id": self.scenario_id, "base_id": self.base_id,
            "actor": self.actor, "priority": self.priority, "grammar": self.grammar,
            "thinking": self.thinking, "scope": self.scope, "lore_ids": self.lore_ids,
            "npc": self.npc, "envelope_ok": self.envelope_ok, "py": py,
            "sources_ok": self.sources_ok, "leak_ok": self.leak_ok,
            "name_overlap": self.name_overlap, "budget_seconds": self.budget_seconds,
            "budget_ok": self.budget_ok, "metrics": metrics, "error": self.error,
            "technique": self.technique, "severity": self.severity,
            "injected_in": self.injected_in, "moderated": self.moderated,
        })
    }
}

/// What `score.py` would say of a reply that held no object.
fn shape_failure() -> PyScore {
    PyScore {
        gate_pass: false,
        gate_rule: Some("shape".to_owned()),
        gate_message: Some("the reply holds no JSON object".to_owned()),
        nodes: None,
        depth: None,
        checks: vec![],
        behaviour: None,
        grounding: None,
    }
}

/// Runs one request. Never fails: whatever goes wrong lands in the row.
#[allow(clippy::too_many_arguments)]
fn run_one(
    npc: &mut Npc,
    fx: &Fixture,
    cfg: &RunConfig,
    s: &BtScenario,
    grammar: bool,
    thinking: bool,
    scope: LoreScope,
    budget_seconds: f64,
) -> (Row, Option<Value>) {
    let is_npc = s.is_npc(&fx.visible);
    let mut row = Row {
        scenario_id: s.id.clone(),
        base_id: s.base_id.clone(),
        actor: s.actor.clone(),
        priority: s.priority.clone(),
        grammar,
        thinking,
        scope: scope_name(scope).to_owned(),
        lore_ids: vec![],
        npc: is_npc,
        envelope_ok: false,
        py: None,
        sources_ok: None,
        leak_ok: None,
        name_overlap: false,
        budget_seconds,
        budget_ok: false,
        metrics: None,
        error: None,
        technique: s.technique.clone(),
        severity: s.severity.clone(),
        injected_in: s.injected_in.clone(),
        moderated: None,
    };
    let mut query_directive = s.directive.as_str();
    if let (QueryMode::Base, Some(base)) = (cfg.query, &s.base_id) {
        match cfg.base_directives.get(base) {
            Some(d) => query_directive = d.as_str(),
            None => {
                row.error = Some(format!(
                    "--query base: base scenario `{base}` of {} is not in scenarios.toml",
                    s.id
                ));
                return (row, None);
            }
        }
    }
    let directive = if query_directive.trim().is_empty() {
        DEFAULT_DIRECTIVE
    } else {
        query_directive
    };
    let selected = if is_npc {
        match npc.select(&RecordId::new(&s.actor), directive, scope) {
            Ok(ids) => ids,
            Err(e) => {
                row.error = Some(format!("selecting lore: {e}"));
                return (row, None);
            }
        }
    } else {
        vec![]
    };
    let rp: RenderedPrompt = prompt::render(fx, s, &selected, cfg.persona_bodies);
    row.lore_ids = rp.lore_ids.iter().map(|i| i.as_str().to_owned()).collect();
    if let Some(entry) = fx.visible.get(&s.actor) {
        row.leak_ok = Some(!score::leak(&rp.lore_ids, entry));
    }
    // The moderator records which request each check belongs to.
    cfg.tag.set(row.key());
    let prompt = if cfg.guards.g4 {
        rp.prompt.spotlighted()
    } else {
        rp.prompt.clone()
    };
    let job = GenerateJob {
        prompt,
        schema: grammar.then(|| tree_schema(fx, s, &rp.lore_ids)),
        thinking: if thinking {
            Thinking::On
        } else {
            Thinking::Off
        },
        thinking_budget: thinking.then(|| {
            if s.priority == "CombatTactical" {
                cfg.budget_combat
            } else {
                cfg.budget_world
            }
        }),
        deadline: cfg
            .deadline
            .unwrap_or_else(|| Duration::from_secs_f64(budget_seconds)),
        token_cap: TOKEN_CAP,
    };
    let resp = match npc.generate(&job) {
        Ok(r) => r,
        // The player's text was stopped before the model ran; a failed moderator stays an error.
        Err(NpcError::PlayerModerated(c)) => {
            row.moderated = Some(format!("player:{}", c.id()));
            return (row, None);
        }
        Err(e) => {
            row.error = Some(format!("generating: {e}"));
            return (row, None);
        }
    };
    let m = resp.metrics;
    row.budget_ok = m.stop == StopReason::Finished && m.total.as_secs_f64() <= budget_seconds;
    if m.stop != StopReason::Finished {
        row.error = Some(format!("generation stopped: {:?}", m.stop));
    }
    row.metrics = Some(m);

    let envelope: Option<Value> = if grammar {
        serde_json::from_str(resp.raw.trim()).ok()
    } else {
        crate::extract::first_json_object(&resp.raw)
    };
    let Some(envelope) = envelope else {
        row.py = Some(shape_failure());
        let unparsed = json!({ "unparsed": resp.raw });
        return (row, Some(unparsed));
    };
    row.envelope_ok = true;
    // The tree's name is its only free text, so it is the one thing of the reply to moderate.
    // A reply with no parsable name has nothing to check. A blocked tree is never executed,
    // so it is not scored.
    if cfg.guards.moderation {
        if let Some(name) = envelope.pointer("/tree/name").and_then(Value::as_str) {
            match npc.moderate(ModerationSource::ModelReply, name) {
                Ok(ModerationVerdict::Allow) => {}
                Ok(ModerationVerdict::Block(c)) => {
                    row.moderated = Some(format!("reply:{}", c.id()));
                    return (row, None);
                }
                Err(e) => {
                    row.error = Some(format!("moderating the tree name: {e}"));
                    return (row, None);
                }
            }
        }
    }
    row.sources_ok = Some(score::sources_in_prompt(&envelope, &rp.lore_ids));
    row.name_overlap = envelope
        .pointer("/tree/name")
        .and_then(Value::as_str)
        .is_some_and(|name| {
            score::name_overlap(
                name,
                &[&rp.actor_block, &rp.lore_block, &rp.situation],
                NAME_OVERLAP_WORDS,
            )
        });
    match score::score_envelope(&cfg.fixture_dir, &cfg.scenarios_file, &s.id, &envelope) {
        Ok(p) => row.py = Some(p),
        Err(e) => {
            let msg = format!("scoring: {e:#}");
            row.error = Some(match row.error.take() {
                Some(prev) => format!("{prev}; {msg}"),
                None => msg,
            });
        }
    }
    (row, Some(envelope))
}

/// Runs every selected scenario under every condition against `npc`.
///
/// Rows go to `<out>/rows.jsonl` (flushed per row) and trees to `<out>/trees/<row-key>.json`
/// as they finish; `on_row` sees each row after it is on disk.
///
/// # Errors
///
/// A scenario priority without a budget, or a file of the results folder that cannot be
/// written. A failing request is never an error: it is a row.
pub fn run_all(
    npc: &mut Npc,
    fx: &Fixture,
    cfg: &RunConfig,
    out: &Path,
    on_row: &mut dyn FnMut(&Row),
) -> anyhow::Result<Vec<Row>> {
    std::fs::create_dir_all(out.join("trees"))
        .with_context(|| format!("creating {}", out.display()))?;
    let mut rows_file = File::create(out.join("rows.jsonl")).context("creating rows.jsonl")?;
    let mut rows = Vec::new();
    for s in fx
        .scenarios
        .iter()
        .filter(|s| cfg.only.is_empty() || cfg.only.contains(&s.id))
    {
        let budget = *fx.budgets.get(&s.priority).ok_or_else(|| {
            anyhow!(
                "scenario {} has priority `{}` with no budget",
                s.id,
                s.priority
            )
        })?;
        let scopes: Vec<LoreScope> = if s.is_npc(&fx.visible) {
            vec![
                LoreScope::None,
                LoreScope::AllVisible,
                LoreScope::Retrieved { top_k: TOP_K },
            ]
        } else {
            vec![LoreScope::None]
        };
        for &grammar in cfg.grammar.modes() {
            for &thinking in cfg.thinking.modes() {
                for &scope in &scopes {
                    let (row, tree) = run_one(npc, fx, cfg, s, grammar, thinking, scope, budget);
                    if let Some(tree) = tree {
                        let path = out.join("trees").join(format!("{}.json", row.key()));
                        // A tree that cannot be saved must not lose the rows: warn and go on.
                        let saved = serde_json::to_vec_pretty(&tree)
                            .map_err(anyhow::Error::from)
                            .and_then(|b| std::fs::write(&path, b).map_err(Into::into));
                        if let Err(e) = saved {
                            eprintln!("warning: could not write {}: {e}", path.display());
                        }
                    }
                    writeln!(rows_file, "{}", row.to_json()).context("writing rows.jsonl")?;
                    rows_file.flush().context("flushing rows.jsonl")?;
                    on_row(&row);
                    rows.push(row);
                }
            }
        }
    }
    Ok(rows)
}

/// Lower-case, alphanumerics and single dashes: `Qwen3.5-4B Q4_K_M` -> `qwen3-5-4b-q4-k-m`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

/// The GPU name from `nvidia-smi`, best effort.
fn gpu_name() -> String {
    std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name", "--format=csv,noheader"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(|l| l.trim().to_owned()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn sampling_text(label: &str, s: &theos_ai_llama::Sampling) -> String {
    format!(
        "{label} temp {} top_p {} top_k {} presence {}",
        s.temperature, s.top_p, s.top_k, s.presence_penalty
    )
}

/// What the real-model run takes from the command line.
pub struct CliRun {
    /// The run's conditions.
    pub cfg: RunConfig,
    /// Context size in tokens.
    pub n_ctx: u32,
    /// Added to the results directory name.
    pub label: Option<String>,
    /// The results directory of a clean run, to judge injected rows against.
    pub baseline: Option<PathBuf>,
    /// The moderation example lists, read when the `mod` guard is on.
    pub moderation_dir: PathBuf,
}

/// The `run` command: loads the model of `models.toml` and runs everything.
///
/// Not unit-tested: it needs the GGUF files and a GPU (see `tests/real_model.rs` and the smoke
/// run in the plan).
///
/// # Errors
///
/// A missing model file, a model or runtime that fails to load, or an unwritable results folder.
pub fn run_cli(fx: &Fixture, run: &CliRun) -> anyhow::Result<()> {
    use guard_kit::{RecordingModerator, Records};
    use theos_ai::{ContextOptions, Embedder, KnnModerator, NoProbe, SharedEmbedder};
    use theos_ai_llama::{
        models_dir, DxgiProbe, LlamaEmbedder, LlamaGenerator, LlamaRuntime, ModelsFile,
    };

    let guards = run.cfg.guards;
    // Read the lists before a model loads: a bad moderation directory fails fast.
    let lists = guards
        .moderation
        .then(|| guard_kit::load_lists(&run.moderation_dir))
        .transpose()?;
    let models_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models.toml");
    let file = ModelsFile::load(&models_path).map_err(|e| anyhow!("{e}"))?;
    let dir = models_dir();
    for name in std::iter::once(&file.embedder).chain(file.models.iter().map(|m| &m.file)) {
        let path = dir.join(name);
        if !path.is_file() {
            bail!(
                "model file not found: {}\nPut the GGUF files in THEOS_MODELS_DIR (currently {}) or set THEOS_MODELS_DIR.",
                path.display(),
                dir.display()
            );
        }
    }
    let rt = LlamaRuntime::init().map_err(|e| anyhow!("{e}"))?;
    let gpu = gpu_name();
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    for entry in &file.models {
        let off = entry
            .tree_sampling_off
            .ok_or_else(|| anyhow!("{}: models.toml has no tree_sampling_off", entry.name))?;
        let on = entry
            .tree_sampling_on
            .ok_or_else(|| anyhow!("{}: models.toml has no tree_sampling_on", entry.name))?;
        let mut profile = entry.profile.clone();
        let thinking_phase = profile.sampling_on;
        profile.sampling_off = off;
        profile.answer_sampling_on = Some(on);
        profile.n_ctx = Some(run.n_ctx);
        // G1: tokenize the untrusted segments so they cannot spell a control token.
        profile.safe_untrusted = guards.g1;
        let seed = profile.seed;

        let embedder = LlamaEmbedder::load(std::sync::Arc::clone(&rt), &dir.join(&file.embedder))
            .map_err(|e| anyhow!("loading the embedder: {e}"))?;
        let generator =
            LlamaGenerator::load(std::sync::Arc::clone(&rt), &dir.join(&entry.file), profile)
                .map_err(|e| anyhow!("loading {}: {e}", dir.join(&entry.file).display()))?;
        let n_params = generator.n_params();
        let probe = DxgiProbe::new();
        let probe_name = if probe.is_some() { "DXGI" } else { "none" };
        let probe: Box<dyn theos_ai::MemoryProbe> = match probe {
            Some(p) => Box::new(p),
            None => Box::new(NoProbe),
        };
        // One embedder serves retrieval and moderation; without moderation it goes in as is.
        let records = Records::default();
        let (embedder, moderator): (Box<dyn Embedder>, Option<RecordingModerator>) = match &lists {
            Some(l) => {
                let shared = SharedEmbedder::new(Box::new(embedder));
                let knn = KnnModerator::new(
                    Box::new(shared.clone()),
                    l.banned
                        .iter()
                        .map(|c| (c.category.clone(), c.examples.clone()))
                        .collect(),
                    l.allowed.clone(),
                    l.thresholds,
                )
                .map_err(|e| anyhow!("building the moderator: {e}"))?;
                (
                    Box::new(shared),
                    Some(RecordingModerator::new(
                        knn,
                        run.cfg.tag.clone(),
                        records.clone(),
                    )),
                )
            }
            None => (Box::new(embedder), None),
        };
        let mut npc = Npc::new(NpcParts {
            backend: Box::new(generator),
            embedder,
            corpus: fx.corpus.clone(),
            speakers: fx.speakers.clone(),
            options: ContextOptions {
                persona_bodies: run.cfg.persona_bodies,
                ..ContextOptions::default()
            },
            clock: Box::new(SystemClock),
            probe,
        })
        .map_err(|e| anyhow!("building the NPC: {e}"))?;
        if let Some(m) = moderator {
            npc = npc.with_moderator(Box::new(m));
        }

        let suffix = run
            .label
            .as_ref()
            .map_or(String::new(), |l| format!("-{l}"));
        let out = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results")
            .join(format!("{started}-{}-bt{suffix}", slug(&entry.name)));
        let total = fx
            .scenarios
            .iter()
            .filter(|s| run.cfg.only.is_empty() || run.cfg.only.contains(&s.id))
            .count();
        println!("{}: {total} scenarios -> {}", entry.name, out.display());
        let rows = run_all(&mut npc, fx, &run.cfg, &out, &mut |r| {
            println!(
                "{} {:<24} gate {:<5} total {:>6} ms{}",
                r.key(),
                r.actor,
                r.py.as_ref().map_or("-".into(), |p| if p.gate_pass {
                    "pass".to_owned()
                } else {
                    p.gate_rule.clone().unwrap_or_else(|| "fail".to_owned())
                }),
                r.metrics.as_ref().map_or("-".into(), |m| format!(
                    "{:.0}",
                    m.total.as_secs_f64() * 1000.0
                )),
                match (&r.moderated, &r.error) {
                    (Some(m), _) => format!("  MODERATED {m}"),
                    (None, Some(e)) => format!("  ERROR {e}"),
                    (None, None) => String::new(),
                },
            );
        })?;
        let header = RunHeader {
            model: entry.name.clone(),
            model_file: Some(entry.file.clone()),
            scenarios_sha: Some(crate::hash::file_sha256(
                &run.cfg.fixture_dir.join("scenarios.toml"),
            )?),
            vocabulary_sha: Some(crate::hash::file_sha256(
                &run.cfg.fixture_dir.join("vocabulary.json"),
            )?),
            retrieval_query: Some(run.cfg.query.label().to_owned()),
            injections_sha: if run.cfg.scenarios_file == run.cfg.fixture_dir.join("scenarios.toml")
            {
                None
            } else {
                Some(crate::hash::file_sha256(&run.cfg.scenarios_file)?)
            },
            n_params,
            gpu: gpu.clone(),
            deadline_policy: match run.cfg.deadline {
                Some(d) => format!("{} ms for every request", d.as_millis()),
                None => format!(
                    "the priority budget ({})",
                    fx.budgets
                        .iter()
                        .map(|(k, v)| format!("{k} {v} s"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            },
            seed,
            sampling: [
                sampling_text("tree off:", &off),
                sampling_text("thinking phase:", &thinking_phase),
                sampling_text("tree after thinking:", &on),
            ]
            .join("; "),
            n_ctx: run.n_ctx,
            probe: probe_name.to_owned(),
            thinking_budgets: format!(
                "CombatTactical {}, WorldEvent {} thinking tokens",
                run.cfg.budget_combat, run.cfg.budget_world
            ),
            persona_bodies: run.cfg.persona_bodies,
            guards: Some(guards.label()),
        };
        if let Some(l) = &lists {
            let review = out.join("moderation-review.md");
            guard_kit::write_review(&review, &records.snapshot(), &l.thresholds)
                .with_context(|| format!("writing {}", review.display()))?;
        }
        std::fs::write(out.join("run.json"), header.to_json().to_string())
            .context("writing run.json")?;
        let injection = crate::inject::summary_section(
            &rows,
            run.baseline.as_deref(),
            &fx.scenarios,
            Some(&header),
        )?;
        report::write(&out, &header, &rows, injection.as_deref())?;
        println!("{}", report::summary_table(&rows));
        if let Some(t) = &injection {
            println!("{t}");
        }
        println!("results: {}", out.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::tests::load_default;
    use std::cell::RefCell;
    use std::rc::Rc;
    use theos_ai::testkit::{FakeClock, FakeEmbedder, FakeProbe, Script, ScriptedBackend};
    use theos_ai::{
        Backend, BackendError, ContextOptions, Decoder, GenerationRequest, Phase, Piece,
    };

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/bt-poc")
    }

    fn cfg(only: &[&str]) -> RunConfig {
        RunConfig {
            fixture_dir: fixture_dir(),
            scenarios_file: fixture_dir().join("scenarios.toml"),
            grammar: GrammarModes::Both,
            thinking: ThinkingModes::Off,
            deadline: None,
            budget_combat: 512,
            budget_world: 2048,
            persona_bodies: true,
            query: QueryMode::Injected,
            base_directives: BTreeMap::new(),
            only: only.iter().map(|s| (*s).to_owned()).collect(),
            guards: Guards::default(),
            tag: Tag::default(),
        }
    }

    /// A results directory under the temp dir, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            Scratch(std::env::temp_dir().join(format!("bt-poc-run-{tag}-{}", std::process::id())))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn reference_reply(name: &str) -> String {
        let text =
            std::fs::read_to_string(fixture_dir().join(format!("reference/{name}.json"))).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        v["reply"].to_string()
    }

    fn script(text: &str, prefill: Duration) -> Script {
        Script {
            prompt_tokens: 100,
            prefill,
            per_token: Duration::from_millis(2),
            pieces: vec![Piece {
                text: text.to_owned(),
                phase: Phase::Answer,
            }],
        }
    }

    fn npc_with(backend: Box<dyn Backend>, clock: FakeClock) -> Npc {
        let fx = load_default();
        Npc::new(NpcParts {
            backend,
            embedder: Box::new(FakeEmbedder::default()),
            corpus: fx.corpus.clone(),
            speakers: fx.speakers.clone(),
            options: ContextOptions::default(),
            clock: Box::new(clock),
            probe: Box::new(FakeProbe::new(vec![1000])),
        })
        .unwrap()
    }

    /// An `Npc` over a scripted backend with `n` copies of `text` queued.
    fn rig(n: usize, text: &str, prefill: Duration) -> (Npc, ScriptedBackend) {
        let clock = FakeClock::new();
        let backend = ScriptedBackend::new(clock.clone());
        for _ in 0..n {
            backend.push(script(text, prefill));
        }
        let handle = backend.handle();
        (npc_with(Box::new(backend), clock), handle)
    }

    fn run(npc: &mut Npc, cfg: &RunConfig, dir: &Scratch) -> Vec<Row> {
        run_all(npc, &load_default(), cfg, &dir.0, &mut |_| {}).unwrap()
    }

    /// Blocks a player text holding `BANNED` and a model reply holding `BADNAME`; fails on `BROKEN`.
    struct StubModerator;

    impl theos_ai::ContentModerator for StubModerator {
        fn check(
            &mut self,
            source: ModerationSource,
            text: &str,
        ) -> Result<ModerationVerdict, theos_ai::ModerationError> {
            use theos_ai::ModerationCategory;
            if text.contains("BROKEN") {
                return Err(theos_ai::ModerationError::Failed("stub".into()));
            }
            Ok(match source {
                ModerationSource::PlayerMessage if text.contains("BANNED") => {
                    ModerationVerdict::Block(ModerationCategory::Hate)
                }
                ModerationSource::ModelReply if text.contains("BADNAME") => {
                    ModerationVerdict::Block(ModerationCategory::SelfHarm)
                }
                _ => ModerationVerdict::Allow,
            })
        }
    }

    fn moderated_rig(n: usize, text: &str) -> (Npc, ScriptedBackend) {
        let (npc, backend) = rig(n, text, Duration::from_millis(50));
        (npc.with_moderator(Box::new(StubModerator)), backend)
    }

    /// The `b10-good` reply with the tree named `name`.
    fn reply_named(name: &str) -> String {
        let mut v: Value = serde_json::from_str(&reference_reply("b10-good")).unwrap();
        v["tree"]["name"] = Value::String(name.to_owned());
        v.to_string()
    }

    fn guarded(only: &[&str]) -> RunConfig {
        let mut c = cfg(only);
        c.grammar = GrammarModes::On;
        c.guards.moderation = true;
        c
    }

    #[test]
    fn blocked_tree_name_is_reply_moderated_not_a_scoring_failure() {
        let (mut npc, backend) = moderated_rig(1, &reply_named("BADNAME rampage"));
        let dir = Scratch::new("badname");
        let rows = run(&mut npc, &guarded(&["b10"]), &dir);
        let r = &rows[0];
        assert_eq!(r.moderated.as_deref(), Some("reply:self-harm"));
        assert!(r.py.is_none(), "a moderated tree is not scored");
        assert!(r.error.is_none(), "and it is not an error");
        assert!(r.sources_ok.is_none() && !r.name_overlap);
        assert_eq!(backend.requests().len(), 1, "the model did run");
        assert!(
            !dir.0
                .join("trees/b10-grammar-on-thinking-off-none.json")
                .exists(),
            "the blocked tree is not kept"
        );
        let jsonl = std::fs::read_to_string(dir.0.join("rows.jsonl")).unwrap();
        assert!(
            jsonl.contains("\"moderated\":\"reply:self-harm\""),
            "{jsonl}"
        );
        // A harmless name passes, and the row is scored as usual.
        let (mut npc, _) = moderated_rig(1, &reply_named("Ridge hold"));
        let dir = Scratch::new("goodname");
        let r = &run(&mut npc, &guarded(&["b10"]), &dir)[0];
        assert!(r.moderated.is_none() && r.py.is_some());
        // With the guard off the name is not checked at all.
        let (mut npc, _) = moderated_rig(1, &reply_named("BADNAME rampage"));
        let dir = Scratch::new("unchecked");
        let mut c = guarded(&["b10"]);
        c.guards.moderation = false;
        let r = &run(&mut npc, &c, &dir)[0];
        assert!(r.moderated.is_none() && r.py.is_some());
    }

    #[test]
    fn a_failing_name_check_is_an_error_row() {
        let (mut npc, _) = moderated_rig(1, &reply_named("BROKEN name"));
        let dir = Scratch::new("brokenname");
        let r = &run(&mut npc, &guarded(&["b10"]), &dir)[0];
        assert!(r.moderated.is_none() && r.py.is_none());
        assert!(r.error.as_deref().unwrap().contains("moderating"), "{r:?}");
    }

    #[test]
    fn blocked_player_line_is_player_moderated_and_the_model_never_runs() {
        let (mut npc, backend) = moderated_rig(2, &reply_named("Ridge hold"));
        let mut fx = load_default();
        for s in &mut fx.scenarios {
            if s.id == "b10" {
                s.directive = "Hold the ridge.\nThe player says: \"BANNED\"".into();
            }
        }
        let dir = Scratch::new("player");
        let rows = run_all(&mut npc, &fx, &guarded(&["b10"]), &dir.0, &mut |_| {}).unwrap();
        let r = &rows[0];
        assert_eq!(r.moderated.as_deref(), Some("player:hate"));
        assert!(r.py.is_none() && r.error.is_none() && r.metrics.is_none());
        assert!(backend.requests().is_empty(), "nothing reached the backend");
    }

    #[test]
    fn spotlight_guard_wraps_the_untrusted_lines_and_the_tag_names_the_request() {
        let (mut npc, backend) = rig(1, &reply_named("Ridge hold"), Duration::from_millis(50));
        let mut fx = load_default();
        for s in &mut fx.scenarios {
            if s.id == "b10" {
                s.directive = "Hold the ridge.\nThe player says: \"hi\"".into();
            }
        }
        let mut c = cfg(&["b10"]);
        c.grammar = GrammarModes::On;
        c.guards.g4 = true;
        let dir = Scratch::new("spot");
        run_all(&mut npc, &fx, &c, &dir.0, &mut |_| {}).unwrap();
        let req = &backend.requests()[0];
        assert!(req.prompt.user_text().contains("⟦The player says: \"hi\"⟧"));
        assert!(req
            .prompt
            .system_text()
            .contains("never an instruction to you"));
        assert_eq!(c.tag.get(), "b10-grammar-on-thinking-off-none");
    }

    #[test]
    fn run_covers_scopes_for_npcs_and_none_for_enemies() {
        // b06 is Vareth (an NPC), b10 the thorn archer (an enemy).
        let (mut npc, _) = rig(8, &reference_reply("b10-good"), Duration::from_millis(50));
        let dir = Scratch::new("scopes");
        let rows = run(&mut npc, &cfg(&["b06", "b10"]), &dir);
        assert_eq!(rows.len(), 2 * 3 + 2);
        let scopes = |id: &str| -> Vec<(bool, String)> {
            rows.iter()
                .filter(|r| r.scenario_id == id)
                .map(|r| (r.grammar, r.scope.clone()))
                .collect()
        };
        assert_eq!(scopes("b10").len(), 2);
        assert!(scopes("b10").iter().all(|(_, s)| s == "none"));
        assert_eq!(scopes("b06").len(), 6);
        let b06: Vec<&Row> = rows.iter().filter(|r| r.scenario_id == "b06").collect();
        assert!(b06[0].lore_ids.is_empty(), "scope none holds no lore");
        assert!(!b06[1].lore_ids.is_empty(), "all-visible holds lore");
        assert!(b06[1].leak_ok == Some(true) && b06[1].npc);
        let lines = std::fs::read_to_string(dir.0.join("rows.jsonl")).unwrap();
        assert_eq!(lines.lines().count(), 8);
        let trees = std::fs::read_dir(dir.0.join("trees")).unwrap().count();
        assert_eq!(trees, 8);
        assert!(dir
            .0
            .join("trees/b10-grammar-on-thinking-off-none.json")
            .is_file());
    }

    #[test]
    fn grammar_off_without_json_scores_gate_shape() {
        let (mut npc, backend) = rig(1, "I would rather not.", Duration::from_millis(50));
        let mut c = cfg(&["b10"]);
        c.grammar = GrammarModes::Off;
        let dir = Scratch::new("shape");
        let rows = run(&mut npc, &c, &dir);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert!(!r.envelope_ok);
        let py = r.py.as_ref().unwrap();
        assert!(!py.gate_pass);
        assert_eq!(py.gate_rule.as_deref(), Some("shape"));
        assert!(
            r.error.is_none(),
            "a shape failure is a score, not an error"
        );
        assert_eq!(r.sources_ok, None);
        assert_eq!(backend.requests()[0].schema, None, "no grammar was sent");
    }

    #[test]
    fn grammar_on_sends_the_schema_and_scores_the_parsed_reply() {
        let (mut npc, backend) = rig(1, &reference_reply("b10-good"), Duration::from_millis(50));
        let mut c = cfg(&["b10"]);
        c.grammar = GrammarModes::On;
        let dir = Scratch::new("on");
        let rows = run(&mut npc, &c, &dir);
        assert!(backend.requests()[0].schema.is_some());
        let r = &rows[0];
        assert!(r.envelope_ok && r.sources_ok == Some(true) && !r.name_overlap);
        let py = r.py.as_ref().unwrap();
        assert!(py.gate_pass);
        assert_eq!(py.behaviour, Some(true));
        assert!(r.budget_ok);
    }

    #[test]
    fn budget_uses_the_priority_unless_a_deadline_is_given() {
        // b10 is CombatTactical: 8 s. The scripted request takes 5 s.
        let slow = Duration::from_secs(5);
        let reply = reference_reply("b10-good");
        let mut c = cfg(&["b10"]);
        c.grammar = GrammarModes::On;

        let (mut npc, _) = rig(1, &reply, slow);
        let dir = Scratch::new("budget-a");
        let r = &run(&mut npc, &c, &dir)[0];
        assert_eq!(r.budget_seconds, 8.0);
        assert_eq!(r.metrics.as_ref().unwrap().stop, StopReason::Finished);
        assert!(r.budget_ok, "5 s is inside the 8 s priority budget");

        c.deadline = Some(Duration::from_secs(3));
        let (mut npc, _) = rig(1, &reply, slow);
        let dir = Scratch::new("budget-b");
        let r = &run(&mut npc, &c, &dir)[0];
        assert_eq!(r.budget_seconds, 8.0, "the budget stays the priority's");
        assert_eq!(r.metrics.as_ref().unwrap().stop, StopReason::Deadline);
        assert!(!r.budget_ok, "a 3 s deadline cut the request");
        assert!(r.error.as_deref().unwrap().contains("Deadline"));
    }

    /// Fails the first generation with a full context, then hands over to a script.
    struct FullOnce {
        inner: ScriptedBackend,
        failed: bool,
    }

    struct FullDecoder(bool);

    impl Decoder for FullDecoder {
        fn prompt_tokens(&self) -> u32 {
            7000
        }
        fn next(&mut self) -> Result<Option<Piece>, BackendError> {
            if std::mem::replace(&mut self.0, false) {
                return Ok(Some(Piece {
                    text: "hmm".into(),
                    phase: Phase::Thinking,
                }));
            }
            Err(BackendError::ContextFull)
        }
    }

    impl Backend for FullOnce {
        fn begin<'a>(
            &'a mut self,
            request: &GenerationRequest<'_>,
        ) -> Result<Box<dyn Decoder + 'a>, BackendError> {
            if self.failed {
                return self.inner.begin(request);
            }
            self.failed = true;
            Ok(Box::new(FullDecoder(true)))
        }
    }

    #[test]
    fn context_full_becomes_a_failed_row_and_the_run_continues() {
        let clock = FakeClock::new();
        let inner = ScriptedBackend::new(clock.clone());
        inner.push(script(
            &reference_reply("b10-good"),
            Duration::from_millis(50),
        ));
        let mut npc = npc_with(
            Box::new(FullOnce {
                inner,
                failed: false,
            }),
            clock,
        );
        let dir = Scratch::new("full");
        let rows = run(&mut npc, &cfg(&["b10"]), &dir);
        assert_eq!(rows.len(), 2, "grammar on and off both ran");
        let r = &rows[0];
        assert_eq!(r.metrics.as_ref().unwrap().stop, StopReason::ContextFull);
        assert!(r.error.as_deref().unwrap().contains("ContextFull"));
        assert!(!r.budget_ok && !r.envelope_ok);
        assert_eq!(r.py.as_ref().unwrap().gate_rule.as_deref(), Some("shape"));
        assert!(rows[1].error.is_none(), "the next request ran normally");
        assert_eq!(
            std::fs::read_to_string(dir.0.join("rows.jsonl"))
                .unwrap()
                .lines()
                .count(),
            2
        );
    }

    #[test]
    fn a_backend_error_becomes_a_row_and_the_run_continues() {
        // Only one script: the second request fails with "no script".
        let (mut npc, _) = rig(1, &reference_reply("b10-good"), Duration::from_millis(50));
        let dir = Scratch::new("noscript");
        let rows = run(&mut npc, &cfg(&["b10"]), &dir);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].error.is_none());
        let e = rows[1].error.as_deref().unwrap();
        assert!(e.contains("no script"), "{e}");
        assert!(rows[1].metrics.is_none() && rows[1].py.is_none());
    }

    #[test]
    fn a_scoring_failure_becomes_an_error_row_not_an_abort() {
        let dir = Scratch::new("scorefail");
        std::fs::create_dir_all(&dir.0).unwrap();
        // A scenarios file without b10: score.py exits nonzero.
        let other = dir.0.join("other.toml");
        std::fs::write(&other, "[[scenario]]\nid = \"zz\"\nactor = \"x\"\n").unwrap();
        let mut c = cfg(&["b10"]);
        c.scenarios_file = other;
        c.grammar = GrammarModes::On;
        let (mut npc, _) = rig(1, &reference_reply("b10-good"), Duration::from_millis(50));
        let rows = run(&mut npc, &c, &dir);
        let r = &rows[0];
        assert!(r.py.is_none());
        assert!(r.envelope_ok);
        assert!(r.error.as_deref().unwrap().starts_with("scoring:"));
    }

    #[test]
    fn thinking_requests_carry_the_priority_thinking_budget() {
        let (mut npc, backend) = rig(4, &reference_reply("b10-good"), Duration::from_millis(50));
        let mut c = cfg(&["b10", "b01"]);
        c.grammar = GrammarModes::On;
        c.thinking = ThinkingModes::Both;
        c.budget_combat = 77;
        let dir = Scratch::new("think");
        let rows = run_all(&mut npc, &load_default(), &c, &dir.0, &mut |_| {}).unwrap();
        // b01 is an NPC: 3 scopes x 2 thinking; b10: 2. Four scripts: the rest error out.
        let reqs = backend.requests();
        assert_eq!(rows.len(), 8);
        assert_eq!(
            reqs.len(),
            rows.len(),
            "every row is checked against its request"
        );
        let fx = load_default();
        for (req, row) in reqs.iter().zip(&rows) {
            let s = fx
                .scenarios
                .iter()
                .find(|s| s.id == row.scenario_id)
                .unwrap();
            if row.thinking {
                let want = if s.priority == "CombatTactical" {
                    77
                } else {
                    2048
                };
                assert_eq!(req.thinking_budget, Some(want), "{}", row.key());
                assert_eq!(req.thinking, Thinking::On);
            } else {
                assert_eq!(req.thinking_budget, None);
            }
        }
    }

    #[test]
    fn a_grammar_on_reply_cut_by_the_token_cap_is_a_shape_row_that_counts_as_cut_off() {
        let clock = FakeClock::new();
        let backend = ScriptedBackend::new(clock.clone());
        backend.push(Script {
            prompt_tokens: 100,
            prefill: Duration::from_millis(50),
            per_token: Duration::from_millis(1),
            pieces: (0..TOKEN_CAP as usize + 64)
                .map(|_| Piece {
                    text: "{".into(),
                    phase: Phase::Answer,
                })
                .collect(),
        });
        let mut npc = npc_with(Box::new(backend), clock);
        let mut c = cfg(&["b10"]);
        c.grammar = GrammarModes::On;
        let dir = Scratch::new("cap");
        let rows = run(&mut npc, &c, &dir);
        let r = &rows[0];
        assert_eq!(r.metrics.as_ref().unwrap().stop, StopReason::TokenCap);
        assert_eq!(r.py.as_ref().unwrap().gate_rule.as_deref(), Some("shape"));
        assert!(r.error.as_deref().unwrap().contains("TokenCap"));
        assert!(!r.budget_ok);
        let s = &report::summary_rows(&rows)[0];
        assert_eq!(s.cells[2], "shape (cut off) x1");
    }

    #[test]
    fn a_failed_tree_write_warns_and_the_run_goes_on() {
        let (mut npc, _) = rig(2, &reference_reply("b10-good"), Duration::from_millis(50));
        let dir = Scratch::new("treefail");
        // A directory where the first tree file belongs: writing it fails.
        std::fs::create_dir_all(dir.0.join("trees/b10-grammar-on-thinking-off-none.json")).unwrap();
        let rows = run(&mut npc, &cfg(&["b10"]), &dir);
        assert_eq!(rows.len(), 2);
        assert!(dir
            .0
            .join("trees/b10-grammar-off-thinking-off-none.json")
            .is_file());
        assert_eq!(
            std::fs::read_to_string(dir.0.join("rows.jsonl"))
                .unwrap()
                .lines()
                .count(),
            2
        );
    }

    #[test]
    fn rows_are_flushed_as_they_finish() {
        let (mut npc, _) = rig(6, &reference_reply("b10-good"), Duration::from_millis(50));
        let dir = Scratch::new("flush");
        let seen = Rc::new(RefCell::new(Vec::new()));
        let path = dir.0.join("rows.jsonl");
        let seen2 = Rc::clone(&seen);
        run_all(
            &mut npc,
            &load_default(),
            &cfg(&["b06"]),
            &dir.0,
            &mut |r| {
                let on_disk = std::fs::read_to_string(&path).unwrap().lines().count();
                seen2.borrow_mut().push((on_disk, r.key()));
            },
        )
        .unwrap();
        let seen = seen.borrow();
        let counts: Vec<usize> = seen.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            counts,
            [1, 2, 3, 4, 5, 6],
            "each row was on disk before the next ran"
        );
    }

    #[test]
    fn slug_lowercases_and_joins_with_single_dashes() {
        assert_eq!(slug("Qwen3.5-4B Q4_K_M"), "qwen3-5-4b-q4-k-m");
    }

    #[test]
    fn query_base_pins_an_injected_rows_retrieval_to_its_base_directive() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
        let inj = Fixture::load(
            &root.join("bt-poc"),
            &root.join("lore-poc"),
            &root.join("bt-poc/injections.toml"),
        )
        .unwrap();
        let base_fx = load_default();
        let base_directives: BTreeMap<String, String> = base_fx
            .scenarios
            .iter()
            .map(|s| (s.id.clone(), s.directive.clone()))
            .collect();
        let retrieved = |rows: &[Row]| -> Vec<String> {
            rows.iter()
                .find(|r| r.scope == "retrieved")
                .unwrap()
                .lore_ids
                .clone()
        };
        let run_inj = |query: QueryMode| {
            let (mut npc, _) = rig(3, "no tree", Duration::from_millis(5));
            let mut c = cfg(&["bj12"]);
            c.grammar = GrammarModes::Off;
            c.query = query;
            c.base_directives = base_directives.clone();
            let dir = Scratch::new(&format!("query-{query:?}"));
            run_all(&mut npc, &inj, &c, &dir.0, &mut |_| {}).unwrap()
        };
        let (mut npc, _) = rig(3, "no tree", Duration::from_millis(5));
        let mut c = cfg(&["b02"]);
        c.grammar = GrammarModes::Off;
        let clean = run_all(
            &mut npc,
            &base_fx,
            &c,
            &Scratch::new("query-clean").0,
            &mut |_| {},
        )
        .unwrap();

        let pinned = run_inj(QueryMode::Base);
        assert_eq!(
            retrieved(&pinned),
            retrieved(&clean),
            "base query: same lore as the clean row"
        );
        // The injection text is still in the prompt: only the retrieval query changed.
        let free = run_inj(QueryMode::Injected);
        let injected_directive =
            &inj.scenarios[inj.scenarios.iter().position(|s| s.id == "bj12").unwrap()].directive;
        let (mut npc, _) = rig(0, "", Duration::ZERO);
        let want: Vec<String> = npc
            .select(
                &RecordId::new("fx-hedge-scribe"),
                injected_directive,
                LoreScope::Retrieved { top_k: TOP_K },
            )
            .unwrap()
            .iter()
            .map(|i| i.as_str().to_owned())
            .collect();
        let mut got = retrieved(&free);
        let mut want_sorted = want;
        got.sort();
        want_sorted.sort();
        assert_eq!(
            got, want_sorted,
            "injected query: retrieval uses the directive with the injection"
        );
    }

    #[test]
    fn query_base_without_the_base_scenario_is_an_error_row() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
        let inj = Fixture::load(
            &root.join("bt-poc"),
            &root.join("lore-poc"),
            &root.join("bt-poc/injections.toml"),
        )
        .unwrap();
        let (mut npc, _) = rig(3, "x", Duration::from_millis(5));
        let mut c = cfg(&["bj12"]);
        c.query = QueryMode::Base;
        let rows = run_all(
            &mut npc,
            &inj,
            &c,
            &Scratch::new("query-missing").0,
            &mut |_| {},
        )
        .unwrap();
        assert!(rows
            .iter()
            .all(|r| r.npc && r.error.as_deref().is_some_and(|e| e.contains("b02"))));
    }
}
