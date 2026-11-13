//! The `run` command: drive every scenario through a model and score it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use theos_ai::ContextOptions;
use theos_ai::{
    visible, BackendError, Corpus, EmbedError, LoreRecord, LoreScope, MemoryProbe, Metrics,
    NoProbe, Npc, NpcError, NpcParts, NpcRequest, NpcWorker, RecordId, Speaker, SystemClock,
    Thinking,
};
use theos_ai_llama::{
    models_dir, DxgiProbe, LlamaEmbedder, LlamaGenerator, LlamaRuntime, ModelEntry, ModelsFile,
};

use crate::inject::{InjectingBackend, NameSlot, ThinkingModes};
use crate::report::{self, RunHeader, WarmUp};
use crate::scenarios::{
    load_scenarios, load_visible, Behaviour, Scenarios, Severity, VisibleEntry,
};
use crate::score::{check, Checks};

/// One scored request.
#[derive(Clone, Debug)]
pub struct Row {
    /// Model display name.
    pub model: String,
    /// Whether thinking was on.
    pub thinking: Thinking,
    /// How much lore the request carried.
    pub scope: LoreScope,
    /// Scenario id.
    pub scenario_id: String,
    /// The speaker.
    pub npc: String,
    /// The scenario's expected behaviour.
    pub behaviour: Behaviour,
    /// What the player said.
    pub question: String,
    /// Why the scenario exists.
    pub note: String,
    /// Every check's outcome; `None` when the driver failed the request.
    pub checks: Option<Checks>,
    /// What the driver measured; `None` when the driver failed the request.
    pub metrics: Option<Metrics>,
    /// The reply text; empty when there is no usable reply.
    pub reply_text: String,
    /// Why there is no usable reply (unparsable output or a driver error), when there is none.
    pub reply_error: Option<String>,
    /// The raw model output.
    pub raw: String,
    /// A human should read this answer (`rumour` or `hand_scored`).
    pub hand_read: bool,
    /// The injection technique of the scenario, when it has one.
    pub technique: Option<String>,
    /// How bad it is when the injection works, when the scenario says.
    pub severity: Option<Severity>,
    /// The player name shown to the NPC in this request.
    pub player_name: Option<String>,
    /// What a good outcome looks like, when the scenario says.
    pub good_outcome: Option<String>,
}

/// The scopes every scenario runs under.
fn scopes(top_k: usize) -> [LoreScope; 3] {
    [
        LoreScope::None,
        LoreScope::AllVisible,
        LoreScope::Retrieved { top_k },
    ]
}

/// What [`run_model`] collected.
#[derive(Debug)]
pub struct RunOutcome {
    /// Every request answered or failed before the run ended.
    pub rows: Vec<Row>,
    /// The unrecorded request sent before measuring; `None` when the run ended
    /// before it could be sent.
    pub warm_up: Option<WarmUp>,
    /// Why the run ended early: a missing `visible.json` entry or a dead worker.
    pub fatal: Option<anyhow::Error>,
}

/// The file each finished row is appended to, one JSON object per line.
const ROWS_FILE: &str = "rows.jsonl";

/// Sends one unrecorded request so the first measured one is not cold: thinking
/// off, the first scenario's speaker and question, scope `Retrieved`.
fn warm_up(
    worker: &mut NpcWorker,
    scenarios: &Scenarios,
    deadline: Duration,
) -> anyhow::Result<Option<WarmUp>> {
    let Some(first) = scenarios.scenario.first() else {
        return Ok(None);
    };
    worker
        .submit(NpcRequest {
            speaker: RecordId::new(first.npc.as_str()),
            question: first.question.clone(),
            scope: LoreScope::Retrieved {
                top_k: scenarios.top_k,
            },
            thinking: Thinking::Off,
            deadline,
        })
        .map_err(|_| anyhow!("the NPC worker stopped"))?;
    let (_, outcome) = worker
        .wait()
        .ok_or_else(|| anyhow!("the NPC worker stopped"))?;
    Ok(Some(match outcome {
        Ok(r) => WarmUp {
            total: r.metrics.total,
            stop: report::stop_label(r.metrics.stop).to_owned(),
        },
        // The measured requests will show whether the worker is still usable.
        Err(e) => WarmUp {
            total: Duration::ZERO,
            stop: format!("failed: {e}"),
        },
    }))
}

/// Runs the thinking `modes` x scenarios x scopes, one request at a time.
///
/// One unrecorded warm-up request goes first (see [`WarmUp`]). Every finished
/// row is appended to `<results_dir>/rows.jsonl` and flushed at once, so a
/// native abort of the process loses at most the request in flight.
///
/// A request the driver fails outright (backend or embedder error) becomes a
/// failed row carrying the error text; the run goes on. Only a scenario naming a
/// speaker without a `visible.json` entry, or a worker that stopped, ends the
/// run early, and the rows collected so far are still returned.
///
/// `name` is the slot the worker's [`InjectingBackend`] reads: it is set to each
/// scenario's `player_name` (or cleared) before the request is submitted, and
/// cleared for the warm-up.
#[allow(clippy::too_many_arguments)]
pub fn run_model(
    worker: &mut NpcWorker,
    model: &str,
    scenarios: &Scenarios,
    visible: &BTreeMap<String, VisibleEntry>,
    deadline: Duration,
    results_dir: &Path,
    modes: &[Thinking],
    name: &NameSlot,
) -> RunOutcome {
    let total = modes.len() * scenarios.scenario.len() * 3;
    let mut rows = Vec::with_capacity(total);
    let mut warmed = None;
    let fatal = (|| {
        std::fs::create_dir_all(results_dir)
            .with_context(|| format!("creating {}", results_dir.display()))?;
        let path = results_dir.join(ROWS_FILE);
        let mut sink =
            std::fs::File::create(&path).with_context(|| format!("creating {}", path.display()))?;
        name.set(None);
        warmed = warm_up(worker, scenarios, deadline)?;
        run_requests(
            worker, model, scenarios, visible, deadline, total, modes, name, &mut rows, &mut sink,
        )
    })()
    .err();
    RunOutcome {
        rows,
        warm_up: warmed,
        fatal,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_requests(
    worker: &mut NpcWorker,
    model: &str,
    scenarios: &Scenarios,
    visible: &BTreeMap<String, VisibleEntry>,
    deadline: Duration,
    total: usize,
    modes: &[Thinking],
    name: &NameSlot,
    rows: &mut Vec<Row>,
    sink: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    let markers = scenarios.markers();
    for &thinking in modes {
        for scenario in &scenarios.scenario {
            let entry = visible
                .get(&scenario.npc)
                .ok_or_else(|| anyhow!("no visible.json entry for `{}`", scenario.npc))?;
            for scope in scopes(scenarios.top_k) {
                let request = NpcRequest {
                    speaker: RecordId::new(scenario.npc.as_str()),
                    question: scenario.question.clone(),
                    scope,
                    thinking,
                    deadline,
                };
                let mut row = Row {
                    model: model.to_owned(),
                    thinking,
                    scope,
                    scenario_id: scenario.id.clone(),
                    npc: scenario.npc.clone(),
                    behaviour: scenario.behaviour,
                    question: scenario.question.clone(),
                    note: scenario.note.clone(),
                    checks: None,
                    metrics: None,
                    reply_text: String::new(),
                    reply_error: None,
                    raw: String::new(),
                    hand_read: scenario.hand_scored || scenario.behaviour == Behaviour::Rumour,
                    technique: scenario.technique.clone(),
                    severity: scenario.severity,
                    player_name: scenario.player_name.clone(),
                    good_outcome: scenario.good_outcome.clone(),
                };
                name.set(scenario.player_name.clone());
                worker
                    .submit(request)
                    .map_err(|_| anyhow!("the NPC worker stopped"))?;
                let (_, outcome) = worker
                    .wait()
                    .ok_or_else(|| anyhow!("the NPC worker stopped"))?;
                match outcome {
                    Ok(response) => {
                        row.checks = Some(check(
                            scenario,
                            &scenarios.forbid_words,
                            &markers,
                            entry,
                            scope,
                            &response,
                            deadline,
                        ));
                        match &response.reply {
                            Ok(reply) => row.reply_text = reply.text.clone(),
                            Err(e) => row.reply_error = Some(e.to_string()),
                        }
                        eprintln!(
                            "[{}/{}] {} {:?} {:?}: {} ms",
                            rows.len() + 1,
                            total,
                            scenario.id,
                            thinking,
                            scope,
                            response.metrics.total.as_millis()
                        );
                        row.metrics = Some(response.metrics);
                        row.raw = response.raw;
                    }
                    Err(e) => {
                        eprintln!(
                            "[{}/{}] {} {:?} {:?}: FAILED {e}",
                            rows.len() + 1,
                            total,
                            scenario.id,
                            thinking,
                            scope
                        );
                        row.reply_error = Some(e.to_string());
                    }
                }
                // A failed write must not lose the row: the final tables still have it.
                if let Err(e) =
                    writeln!(sink, "{}", report::row_json(&row)).and_then(|()| sink.flush())
                {
                    eprintln!("warning: could not append the row to {ROWS_FILE}: {e}");
                }
                rows.push(row);
            }
        }
    }
    Ok(())
}

/// The rows whose leak or forbidden-record check failed: a defect of the engine,
/// not a weakness of the model. Failed requests have no known context and are not counted.
pub fn defects(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| r.checks.as_ref().map(|c| (r, c)))
        .filter(|(_, c)| !c.leak || !c.forbidden)
        .map(|(r, c)| {
            format!(
                "{} {:?} {:?}: {}",
                r.scenario_id,
                r.thinking,
                r.scope,
                match (c.leak, c.forbidden) {
                    (false, false) => "leak and forbidden record",
                    (false, true) => "leak",
                    _ => "forbidden record",
                }
            )
        })
        .collect()
}

/// `committed corpus.json`: the same shape `export` writes.
#[derive(Deserialize)]
struct Snapshot {
    records: Vec<LoreRecord>,
    speakers: Vec<Speaker>,
}

/// Converts the fixture afresh and compares it with the committed snapshots.
///
/// # Errors
///
/// When the fresh conversion differs from `expected/corpus.json`, or a
/// speaker's `visible()` differs from `expected/visible.json`.
pub fn verify_fixture(
    fixture: &Path,
) -> anyhow::Result<(Corpus, Vec<Speaker>, BTreeMap<String, VisibleEntry>)> {
    let (mut records, mut speakers) = crate::convert::load_fixture(&fixture.join("records"))?;
    records.sort_by(|a, b| a.id.cmp(&b.id));
    speakers.sort_by(|a, b| a.id.cmp(&b.id));

    let path = fixture.join("expected/corpus.json");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let committed: Snapshot =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    if committed.records != records || committed.speakers != speakers {
        bail!("the fixture no longer matches expected/corpus.json; run `npc-poc export` and review the diff");
    }

    let corpus = Corpus::new(records).map_err(|e| anyhow!("{e}"))?;
    let golden = load_visible(&fixture.join("expected/visible.json"))?;
    if golden.len() != speakers.len() {
        bail!(
            "expected/visible.json has {} speakers, the fixture has {}",
            golden.len(),
            speakers.len()
        );
    }
    for speaker in &speakers {
        let fresh = visible(&corpus, speaker);
        let entry = golden
            .get(speaker.id.as_str())
            .ok_or_else(|| anyhow!("expected/visible.json has no entry for `{}`", speaker.id))?;
        let strings = |v: &[RecordId]| v.iter().map(|i| i.as_str().to_owned()).collect::<Vec<_>>();
        if strings(&fresh.persona) != entry.persona
            || strings(&fresh.held) != entry.held
            || strings(&fresh.rejected) != entry.rejected
        {
            bail!(
                "visible() of `{}` differs from expected/visible.json",
                speaker.id
            );
        }
    }
    Ok((corpus, speakers, golden))
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

/// What the worker thread learned while loading, for the run header.
struct LoadInfo {
    n_params: u64,
    probe: bool,
}

/// Starts a worker whose thread loads the embedder, the generator and the probe.
///
/// llama contexts are not `Send`, so everything is built inside the factory.
#[allow(clippy::too_many_arguments)]
fn spawn_worker(
    rt: &Arc<LlamaRuntime>,
    embedder_path: PathBuf,
    model_path: PathBuf,
    entry: &ModelEntry,
    corpus: &Corpus,
    speakers: &[Speaker],
    persona_bodies: bool,
    secret: Option<String>,
    name: Option<NameSlot>,
) -> anyhow::Result<(NpcWorker, LoadInfo)> {
    let (info_tx, info_rx) = mpsc::channel::<LoadInfo>();
    let rt = Arc::clone(rt);
    let profile = entry.profile.clone();
    let corpus = corpus.clone();
    let speakers = speakers.to_vec();
    let worker = NpcWorker::spawn(move || {
        let embedder = LlamaEmbedder::load(Arc::clone(&rt), &embedder_path)
            .map_err(|e| NpcError::Embed(EmbedError::Backend(e.to_string())))?;
        let generator = LlamaGenerator::load(rt, &model_path, profile)
            .map_err(|e| NpcError::Backend(BackendError::Failed(e.to_string())))?;
        let probe = DxgiProbe::new();
        // The receiver outlives the spawn call; a failed send only loses the header info.
        let _ = info_tx.send(LoadInfo {
            n_params: generator.n_params(),
            probe: probe.is_some(),
        });
        let probe: Box<dyn MemoryProbe> = match probe {
            Some(p) => Box::new(p),
            None => Box::new(NoProbe),
        };
        let backend: Box<dyn theos_ai::Backend> = match name {
            Some(name) => Box::new(InjectingBackend::new(Box::new(generator), secret, name)),
            None => Box::new(generator),
        };
        Npc::new(NpcParts {
            backend,
            embedder: Box::new(embedder),
            corpus,
            speakers,
            options: ContextOptions { persona_bodies },
            clock: Box::new(SystemClock),
            probe,
        })
    })
    .map_err(|e| anyhow!("loading {}: {e}", model_path_display(&entry.file)))?;
    let info = info_rx
        .recv()
        .map_err(|_| anyhow!("the worker reported no load info"))?;
    Ok((worker, info))
}

fn model_path_display(file: &str) -> String {
    models_dir().join(file).display().to_string()
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

/// The `run` command.
///
/// # Errors
///
/// A missing model file (named with `THEOS_MODELS_DIR`), a fixture that drifted
/// from its snapshots, any load or driver failure, or, after the tables are
/// written, a leak or forbidden-record defect.
pub fn run_cli(
    fixture: &Path,
    scenarios_path: Option<PathBuf>,
    thinking: ThinkingModes,
    models: Option<PathBuf>,
    deadline: Duration,
    persona_bodies: bool,
) -> anyhow::Result<()> {
    // Everything that can be wrong with the inputs is reported before a model loads.
    let (corpus, speakers, golden) = verify_fixture(fixture)?;
    let scenarios =
        load_scenarios(&scenarios_path.unwrap_or_else(|| fixture.join("scenarios.toml")))?;
    for s in &scenarios.scenario {
        if !golden.contains_key(&s.npc) {
            bail!(
                "scenario `{}` names `{}`, which has no visible.json entry",
                s.id,
                s.npc
            );
        }
    }
    let modes = thinking.modes();
    let models_path =
        models.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../models.toml"));
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

    let markers = scenarios.markers();
    let wrap = markers.system_secret.is_some()
        || scenarios.scenario.iter().any(|s| s.player_name.is_some());
    let mut all_defects = Vec::new();
    let mut fatals = Vec::new();
    for entry in &file.models {
        let model_path = dir.join(&entry.file);
        let name = NameSlot::default();
        let (mut worker, info) = spawn_worker(
            &rt,
            dir.join(&file.embedder),
            model_path.clone(),
            entry,
            &corpus,
            &speakers,
            persona_bodies,
            markers.system_secret.clone(),
            wrap.then(|| name.clone()),
        )?;
        let suffix = if persona_bodies {
            ""
        } else {
            "-no-persona-bodies"
        };
        let out = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results")
            .join(format!("{started}-{}{suffix}", slug(&entry.name)));
        let RunOutcome {
            rows,
            warm_up,
            fatal,
        } = run_model(
            &mut worker,
            &entry.name,
            &scenarios,
            &golden,
            deadline,
            &out,
            &modes,
            &name,
        );
        drop(worker);

        let header = RunHeader {
            model: entry.name.clone(),
            file_bytes: std::fs::metadata(&model_path)?.len(),
            n_params: info.n_params,
            gpu: gpu.clone(),
            deadline,
            seed: entry.profile.seed,
            persona_bodies,
            probe: if info.probe {
                "DXGI".to_owned()
            } else {
                "none".to_owned()
            },
            warm_up,
        };
        report::write(&out, &header, &rows)?;
        println!("{}", report::summary_table(&rows));
        // An injection run also prints its stable summary, after the normal table.
        if rows.iter().any(|r| r.behaviour == Behaviour::Resist) {
            println!("{}", report::injection_summary(&rows));
        }
        println!("results: {}", out.display());
        if let Some(e) = fatal {
            println!(
                "FATAL: {}: run ended early with {} rows: {e:#}",
                entry.name,
                rows.len()
            );
            fatals.push(format!("{}: {e:#}", entry.name));
        }
        all_defects.extend(
            defects(&rows)
                .into_iter()
                .map(|d| format!("{}: {d}", entry.name)),
        );
    }

    if !all_defects.is_empty() {
        println!(
            "DEFECT: {} request(s) leaked or showed a forbidden record:",
            all_defects.len()
        );
        for d in &all_defects {
            println!("DEFECT  {d}");
        }
        bail!("engine defect: leak or forbidden-record check failed");
    }
    if !fatals.is_empty() {
        bail!(
            "run ended early (partial results written): {}",
            fatals.join("; ")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use theos_ai::testkit::{FakeClock, FakeEmbedder, FakeProbe, Script, ScriptedBackend};
    use theos_ai::{HolderKind, Lens, LoreKind, PersonaLayer, Phase, Piece};

    const BOTH: [Thinking; 2] = [Thinking::Off, Thinking::On];

    /// A results directory under the temp dir, removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            Scratch(std::env::temp_dir().join(format!("npc-poc-run-{tag}-{}", std::process::id())))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn jsonl(dir: &Scratch) -> Vec<serde_json::Value> {
        std::fs::read_to_string(dir.0.join("rows.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn belief(id: &str, held_by: &str) -> LoreRecord {
        LoreRecord {
            id: RecordId::new(id),
            kind: LoreKind::Belief,
            title: format!("title {id}"),
            body: format!("body of {id}"),
            voice: vec![],
            held_by: vec![RecordId::new(held_by)],
            target: None,
        }
    }

    fn speaker(id: &str) -> Speaker {
        Speaker {
            id: RecordId::new(id),
            layers: vec![PersonaLayer {
                id: RecordId::new(id),
                kind: HolderKind::Npc,
                title: id.to_owned(),
                role: None,
                body: "a speaker".into(),
            }],
            lens: Lens::default(),
        }
    }

    fn scenario(id: &str, behaviour: Behaviour) -> crate::scenarios::Scenario {
        crate::scenarios::Scenario {
            id: id.into(),
            npc: "n".into(),
            question: format!("question {id}"),
            behaviour,
            expect: vec!["B-1".into()],
            also_ok: vec![],
            rejects: vec![],
            forbid: vec![],
            forbid_words: vec![],
            hand_scored: false,
            note: String::new(),
            technique: None,
            severity: None,
            player_name: None,
            good_outcome: None,
        }
    }

    #[test]
    fn run_model_produces_one_row_per_thinking_scenario_and_scope() {
        let scenarios = Scenarios {
            forbid_words: vec!["node".into()],
            top_k: 2,
            scenario: vec![
                scenario("s1", Behaviour::Answer),
                scenario("s2", Behaviour::Unaware),
                scenario("s3", Behaviour::Rumour),
            ],
            canary: None,
            system_secret: None,
        };
        let golden = BTreeMap::from([(
            "n".to_owned(),
            VisibleEntry {
                persona: vec!["n".into()],
                held: vec!["B-1".into(), "B-2".into()],
                rejected: vec![],
                unaware_of: vec![],
            },
        )]);
        let mut worker = NpcWorker::spawn(|| {
            let clock = FakeClock::new();
            let backend = ScriptedBackend::new(clock.clone());
            for _ in 0..19 {
                backend.push(Script {
                    prompt_tokens: 12,
                    prefill: Duration::from_millis(20),
                    per_token: Duration::from_millis(2),
                    pieces: vec![Piece {
                        text: r#"{"sources":[],"reply":"a NODE hums"}"#.into(),
                        phase: Phase::Answer,
                    }],
                });
            }
            Npc::new(NpcParts {
                backend: Box::new(backend),
                embedder: Box::new(FakeEmbedder::default()),
                corpus: Corpus::new(vec![belief("B-1", "n"), belief("B-2", "n")]).unwrap(),
                speakers: vec![speaker("n")],
                options: ContextOptions::default(),
                clock: Box::new(clock),
                probe: Box::new(FakeProbe::new(vec![1000])),
            })
        })
        .unwrap();

        let dir = Scratch::new("model");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        let rows = run.rows;

        assert_eq!(rows.len(), 18);
        assert_eq!(
            rows.iter().filter(|r| r.thinking == Thinking::On).count(),
            9
        );
        assert_eq!(rows[0].scenario_id, "s1");
        assert_eq!(rows[0].scope, LoreScope::None);
        assert_eq!(rows[2].scope, LoreScope::Retrieved { top_k: 2 });
        assert_eq!(
            rows[0].checks.as_ref().unwrap().words,
            Some(vec!["node".to_owned()])
        );
        assert_eq!(rows[0].reply_text, "a NODE hums");
        assert!(
            rows[6..9].iter().all(|r| r.hand_read),
            "rumour is read by hand"
        );
        assert!(!rows[0].hand_read);
        assert!(defects(&rows).is_empty());
    }

    #[test]
    fn defects_name_rows_with_a_leak_or_forbidden_record() {
        let scenarios = Scenarios {
            forbid_words: vec![],
            top_k: 1,
            scenario: vec![scenario("s1", Behaviour::Answer)],
            canary: None,
            system_secret: None,
        };
        // visible.json says the speaker may see nothing, so every belief in the prompt leaks.
        let golden = BTreeMap::from([(
            "n".to_owned(),
            VisibleEntry {
                persona: vec!["n".into()],
                held: vec![],
                rejected: vec![],
                unaware_of: vec![],
            },
        )]);
        let mut worker = NpcWorker::spawn(|| {
            let clock = FakeClock::new();
            let backend = ScriptedBackend::new(clock.clone());
            for _ in 0..7 {
                backend.push(Script {
                    prompt_tokens: 1,
                    prefill: Duration::ZERO,
                    per_token: Duration::ZERO,
                    pieces: vec![Piece {
                        text: r#"{"sources":[],"reply":"x"}"#.into(),
                        phase: Phase::Answer,
                    }],
                });
            }
            Npc::new(NpcParts {
                backend: Box::new(backend),
                embedder: Box::new(FakeEmbedder::default()),
                corpus: Corpus::new(vec![belief("B-1", "n")]).unwrap(),
                speakers: vec![speaker("n")],
                options: ContextOptions::default(),
                clock: Box::new(clock),
                probe: Box::new(NoProbe),
            })
        })
        .unwrap();
        let dir = Scratch::new("defects");
        let rows = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        )
        .rows;
        let found = defects(&rows);
        assert_eq!(
            found.len(),
            4,
            "AllVisible and Retrieved, thinking off and on: {found:?}"
        );
        assert!(found[0].contains("leak"));
    }

    /// A backend that fails (or panics) on its `at`-th `begin`, and otherwise delegates.
    struct Flaky {
        inner: ScriptedBackend,
        calls: usize,
        at: usize,
        panic: bool,
    }

    impl theos_ai::Backend for Flaky {
        fn begin<'a>(
            &'a mut self,
            request: &theos_ai::GenerationRequest<'_>,
        ) -> Result<Box<dyn theos_ai::Decoder + 'a>, BackendError> {
            self.calls += 1;
            if self.calls == self.at {
                assert!(!self.panic, "scripted worker death");
                return Err(BackendError::Failed("boom".into()));
            }
            self.inner.begin(request)
        }
    }

    fn flaky_worker(at: usize, panic: bool) -> NpcWorker {
        NpcWorker::spawn(move || {
            let clock = FakeClock::new();
            let inner = ScriptedBackend::new(clock.clone());
            for _ in 0..7 {
                inner.push(Script {
                    prompt_tokens: 1,
                    prefill: Duration::ZERO,
                    per_token: Duration::ZERO,
                    pieces: vec![Piece {
                        text: r#"{"sources":[],"reply":"x"}"#.into(),
                        phase: Phase::Answer,
                    }],
                });
            }
            Npc::new(NpcParts {
                backend: Box::new(Flaky {
                    inner,
                    calls: 0,
                    at,
                    panic,
                }),
                embedder: Box::new(FakeEmbedder::default()),
                corpus: Corpus::new(vec![belief("B-1", "n")]).unwrap(),
                speakers: vec![speaker("n")],
                options: ContextOptions::default(),
                clock: Box::new(clock),
                probe: Box::new(NoProbe),
            })
        })
        .unwrap()
    }

    fn one_scenario() -> (Scenarios, BTreeMap<String, VisibleEntry>) {
        let scenarios = Scenarios {
            forbid_words: vec![],
            top_k: 1,
            scenario: vec![scenario("s1", Behaviour::Answer)],
            canary: None,
            system_secret: None,
        };
        let golden = BTreeMap::from([(
            "n".to_owned(),
            VisibleEntry {
                persona: vec!["n".into()],
                held: vec!["B-1".into()],
                rejected: vec![],
                unaware_of: vec![],
            },
        )]);
        (scenarios, golden)
    }

    #[test]
    fn a_failing_request_becomes_a_failed_row_and_the_run_continues() {
        let (scenarios, golden) = one_scenario();
        // Call 1 is the warm-up; call 4 is the third measured request.
        let mut worker = flaky_worker(4, false);
        let dir = Scratch::new("failing");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        assert_eq!(run.rows.len(), 6);
        let failed = &run.rows[2];
        assert!(failed.reply_error.as_deref().unwrap().contains("boom"));
        assert!(failed.metrics.is_none() && failed.checks.is_none());
        assert_eq!(
            run.rows.iter().filter(|r| r.metrics.is_some()).count(),
            5,
            "every other request still has its measurements"
        );
        assert!(defects(&run.rows).is_empty(), "an error is not a leak");
    }

    #[test]
    fn a_worker_that_dies_mid_run_returns_the_rows_so_far_and_a_fatal_error() {
        let (scenarios, golden) = one_scenario();
        let mut worker = flaky_worker(5, true);
        let dir = Scratch::new("dies");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_some());
        assert_eq!(
            run.rows.len(),
            4,
            "three answered, one lost with the worker"
        );
        assert!(run.rows[3].reply_error.is_some());
    }

    #[test]
    fn every_finished_row_is_on_disk_in_rows_jsonl() {
        let (scenarios, golden) = one_scenario();
        let mut worker = flaky_worker(99, false);
        let dir = Scratch::new("jsonl");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        let lines = jsonl(&dir);
        assert_eq!(lines.len(), run.rows.len());
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[0]["scenario"], "s1");
        assert_eq!(lines[0]["thinking"], "off");
        assert_eq!(lines[3]["thinking"], "on");
        assert_eq!(lines[5]["scope"], "retrieved(top 1)");
        assert_eq!(lines[0]["reply_text"], "x");
    }

    #[test]
    fn rows_finished_before_a_worker_dies_are_already_on_disk() {
        let (scenarios, golden) = one_scenario();
        let mut worker = flaky_worker(5, true);
        let dir = Scratch::new("partial");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_some());
        let lines = jsonl(&dir);
        assert_eq!(lines.len(), run.rows.len());
        assert_eq!(lines.len(), 4);
        assert!(lines[3]["reply_error"].is_string(), "the lost row too");
    }

    #[test]
    fn the_results_dir_is_created_before_the_first_request() {
        let (scenarios, golden) = one_scenario();
        let mut worker = flaky_worker(99, false);
        let dir = Scratch::new("nested");
        let nested = dir.0.join("a/b");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &nested,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        assert!(nested.join("rows.jsonl").is_file());
    }

    #[test]
    fn warm_up_is_not_a_row_and_its_time_reaches_the_outcome() {
        let (scenarios, golden) = one_scenario();
        let mut worker = NpcWorker::spawn(|| {
            let clock = FakeClock::new();
            let backend = ScriptedBackend::new(clock.clone());
            // The warm-up is the first request the backend sees.
            backend.push(Script {
                prompt_tokens: 1,
                prefill: Duration::from_millis(70),
                per_token: Duration::from_millis(7),
                pieces: vec![Piece {
                    text: r#"{"sources":[],"reply":"warmup"}"#.into(),
                    phase: Phase::Answer,
                }],
            });
            for _ in 0..6 {
                backend.push(Script {
                    prompt_tokens: 1,
                    prefill: Duration::ZERO,
                    per_token: Duration::ZERO,
                    pieces: vec![Piece {
                        text: r#"{"sources":[],"reply":"x"}"#.into(),
                        phase: Phase::Answer,
                    }],
                });
            }
            Npc::new(NpcParts {
                backend: Box::new(backend),
                embedder: Box::new(FakeEmbedder::default()),
                corpus: Corpus::new(vec![belief("B-1", "n")]).unwrap(),
                speakers: vec![speaker("n")],
                options: ContextOptions::default(),
                clock: Box::new(clock),
                probe: Box::new(NoProbe),
            })
        })
        .unwrap();
        let dir = Scratch::new("warm");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        assert_eq!(run.rows.len(), 6);
        assert!(run.rows.iter().all(|r| r.reply_text == "x"));
        assert_eq!(jsonl(&dir).len(), 6, "the warm-up is not persisted either");
        let warm = run.warm_up.unwrap();
        assert_eq!(warm.total, Duration::from_millis(77));
        assert_eq!(warm.stop, "finished");
    }

    #[test]
    fn a_failing_warm_up_is_reported_and_the_run_goes_on() {
        let (scenarios, golden) = one_scenario();
        let mut worker = flaky_worker(1, false);
        let dir = Scratch::new("warmfail");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
            &dir.0,
            &BOTH,
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        assert_eq!(run.rows.len(), 6);
        let warm = run.warm_up.unwrap();
        assert!(warm.stop.contains("boom"), "{}", warm.stop);
    }

    fn golden_n() -> BTreeMap<String, VisibleEntry> {
        BTreeMap::from([(
            "n".to_owned(),
            VisibleEntry {
                persona: vec!["n".into()],
                held: vec!["B-1".into()],
                rejected: vec![],
                unaware_of: vec![],
            },
        )])
    }

    /// A worker whose backend is wrapped in an [`InjectingBackend`]; returns the
    /// handle onto the scripted backend's recorded requests.
    fn injecting_worker(
        reply: &'static str,
        secret: Option<String>,
        slot: NameSlot,
    ) -> (NpcWorker, ScriptedBackend) {
        let clock = FakeClock::new();
        let backend = ScriptedBackend::new(clock.clone());
        for _ in 0..12 {
            backend.push(Script {
                prompt_tokens: 1,
                prefill: Duration::ZERO,
                per_token: Duration::ZERO,
                pieces: vec![Piece {
                    text: format!(r#"{{"sources":[],"reply":"{reply}"}}"#),
                    phase: Phase::Answer,
                }],
            });
        }
        let handle = backend.handle();
        let worker = NpcWorker::spawn(move || {
            Npc::new(NpcParts {
                backend: Box::new(InjectingBackend::new(Box::new(backend), secret, slot)),
                embedder: Box::new(FakeEmbedder::default()),
                corpus: Corpus::new(vec![belief("B-1", "n")]).unwrap(),
                speakers: vec![speaker("n")],
                options: ContextOptions::default(),
                clock: Box::new(clock),
                probe: Box::new(NoProbe),
            })
        })
        .unwrap();
        (worker, handle)
    }

    #[test]
    fn run_model_runs_only_the_requested_thinking_modes() {
        let scenarios = Scenarios {
            forbid_words: vec![],
            top_k: 1,
            scenario: vec![
                scenario("s1", Behaviour::Answer),
                scenario("s2", Behaviour::Answer),
            ],
            canary: None,
            system_secret: None,
        };
        let (mut worker, _) = injecting_worker("x", None, NameSlot::default());
        let dir = Scratch::new("offonly");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden_n(),
            Duration::from_secs(3),
            &dir.0,
            &[Thinking::Off],
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        assert_eq!(run.rows.len(), 6);
        assert!(run.rows.iter().all(|r| r.thinking == Thinking::Off));
    }

    #[test]
    fn run_model_sets_the_player_name_per_scenario_and_clears_it() {
        let mut named = scenario("s1", Behaviour::Answer);
        named.player_name = Some("Kai".into());
        let scenarios = Scenarios {
            forbid_words: vec![],
            top_k: 1,
            scenario: vec![named, scenario("s2", Behaviour::Answer)],
            canary: None,
            system_secret: None,
        };
        let slot = NameSlot::default();
        let (mut worker, seen) = injecting_worker("x", None, slot.clone());
        let dir = Scratch::new("names");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden_n(),
            Duration::from_secs(3),
            &dir.0,
            &[Thinking::Off],
            &slot,
        );
        assert!(run.fatal.is_none());
        let reqs = seen.requests();
        assert_eq!(reqs.len(), 7, "warm-up plus six");
        let has_name = |i: usize| {
            reqs[i]
                .prompt
                .system
                .contains("The traveller speaking to you")
        };
        assert!(!has_name(0), "the warm-up uses no name");
        assert!((1..=3).all(has_name));
        assert!((4..=6).all(|i| !has_name(i)));
        assert!(reqs[1].prompt.system.contains("called Kai."));
        assert_eq!(run.rows[0].player_name.as_deref(), Some("Kai"));
        assert_eq!(run.rows[3].player_name, None);
    }

    #[test]
    fn run_refuses_a_missing_scenarios_file() {
        let missing = std::env::temp_dir().join("npc-poc-no-such-scenarios.toml");
        let e = run_cli(
            &crate::default_fixture(),
            Some(missing),
            ThinkingModes::Both,
            None,
            Duration::from_secs(1),
            true,
        )
        .unwrap_err();
        assert!(
            format!("{e:#}").contains("npc-poc-no-such-scenarios.toml"),
            "{e:#}"
        );
    }

    #[test]
    fn resist_rows_carry_technique_severity_and_injection_checks() {
        let mut s = scenario("r1", Behaviour::Resist);
        s.technique = Some("direct".into());
        s.severity = Some(Severity::S3);
        s.good_outcome = Some("stays in voice".into());
        s.expect.clear();
        let scenarios = Scenarios {
            forbid_words: vec![],
            top_k: 1,
            scenario: vec![s],
            canary: Some("HONK-7731".into()),
            system_secret: None,
        };
        let (mut worker, _) = injecting_worker("honk 7731", None, NameSlot::default());
        let dir = Scratch::new("resist");
        let run = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden_n(),
            Duration::from_secs(3),
            &dir.0,
            &[Thinking::Off],
            &NameSlot::default(),
        );
        assert!(run.fatal.is_none());
        let row = &run.rows[0];
        assert_eq!(row.technique.as_deref(), Some("direct"));
        assert_eq!(row.severity, Some(Severity::S3));
        assert_eq!(row.good_outcome.as_deref(), Some("stays in voice"));
        let i = row.checks.as_ref().unwrap().injection.as_ref().unwrap();
        assert!(i.canary);
        assert_eq!(i.outcome, Some(Severity::S3));
    }

    #[test]
    fn run_refuses_a_scenario_naming_an_unknown_speaker() {
        let dir = Scratch::new("unknown");
        std::fs::create_dir_all(&dir.0).unwrap();
        let file = dir.0.join("s.toml");
        std::fs::write(
            &file,
            "forbid_words = []\ntop_k = 1\n[[scenario]]\nid = \"x\"\nnpc = \"nobody\"\nquestion = \"q\"\nbehaviour = \"answer\"\n",
        )
        .unwrap();
        let e = run_cli(
            &crate::default_fixture(),
            Some(file),
            ThinkingModes::Both,
            None,
            Duration::from_secs(1),
            true,
        )
        .unwrap_err();
        assert!(format!("{e:#}").contains("nobody"), "{e:#}");
    }

    #[test]
    fn slug_lowercases_and_joins_with_single_dashes() {
        assert_eq!(slug("Qwen3.5-4B Q4_K_M"), "qwen3-5-4b-q4-k-m");
    }

    #[test]
    fn fixture_matches_committed_snapshots() {
        let fixture = crate::default_fixture();
        let (_, speakers, golden) = verify_fixture(&fixture).unwrap();
        assert_eq!(speakers.len(), 8);
        assert_eq!(golden.len(), 8);
    }
}
