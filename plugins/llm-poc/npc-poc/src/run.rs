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

use crate::report::{self, RunHeader};
use crate::scenarios::{load_scenarios, load_visible, Behaviour, Scenarios, VisibleEntry};
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
    /// Every check's outcome.
    pub checks: Checks,
    /// What the driver measured.
    pub metrics: Metrics,
    /// The reply text; empty when there is no usable reply.
    pub reply_text: String,
    /// Why there is no usable reply, when there is none.
    pub reply_error: Option<String>,
    /// The raw model output.
    pub raw: String,
    /// A human should read this answer (`rumour` or `hand_scored`).
    pub hand_read: bool,
}

/// The scopes every scenario runs under.
fn scopes(top_k: usize) -> [LoreScope; 3] {
    [
        LoreScope::None,
        LoreScope::AllVisible,
        LoreScope::Retrieved { top_k },
    ]
}

/// Runs thinking `[Off, On]` x scenarios x scopes, one request at a time.
///
/// # Errors
///
/// A scenario naming a speaker without a `visible.json` entry, a worker that
/// stopped, or a request the driver failed outright (backend or embedder
/// error, not an unusable reply, which is scored).
pub fn run_model(
    worker: &mut NpcWorker,
    model: &str,
    scenarios: &Scenarios,
    visible: &BTreeMap<String, VisibleEntry>,
    deadline: Duration,
) -> anyhow::Result<Vec<Row>> {
    let total = 2 * scenarios.scenario.len() * 3;
    let mut rows = Vec::with_capacity(total);
    for thinking in [Thinking::Off, Thinking::On] {
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
                worker
                    .submit(request)
                    .map_err(|_| anyhow!("the NPC worker stopped"))?;
                let (_, outcome) = worker
                    .wait()
                    .ok_or_else(|| anyhow!("the NPC worker stopped"))?;
                let response = outcome
                    .with_context(|| format!("{} {:?} {:?}", scenario.id, thinking, scope))?;
                let checks = check(
                    scenario,
                    &scenarios.forbid_words,
                    entry,
                    scope,
                    &response,
                    deadline,
                );
                let (reply_text, reply_error) = match &response.reply {
                    Ok(reply) => (reply.text.clone(), None),
                    Err(e) => (String::new(), Some(e.to_string())),
                };
                eprintln!(
                    "[{}/{}] {} {:?} {:?}: {}",
                    rows.len() + 1,
                    total,
                    scenario.id,
                    thinking,
                    scope,
                    response.metrics.total.as_millis()
                );
                rows.push(Row {
                    model: model.to_owned(),
                    thinking,
                    scope,
                    scenario_id: scenario.id.clone(),
                    npc: scenario.npc.clone(),
                    behaviour: scenario.behaviour,
                    question: scenario.question.clone(),
                    note: scenario.note.clone(),
                    checks,
                    metrics: response.metrics,
                    reply_text,
                    reply_error,
                    raw: response.raw,
                    hand_read: scenario.hand_scored || scenario.behaviour == Behaviour::Rumour,
                });
            }
        }
    }
    Ok(rows)
}

/// The rows whose leak or forbidden-record check failed: a defect of the engine,
/// not a weakness of the model.
pub fn defects(rows: &[Row]) -> Vec<String> {
    rows.iter()
        .filter(|r| !r.checks.leak || !r.checks.forbidden)
        .map(|r| {
            format!(
                "{} {:?} {:?}: {}",
                r.scenario_id,
                r.thinking,
                r.scope,
                match (r.checks.leak, r.checks.forbidden) {
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
fn spawn_worker(
    rt: &Arc<LlamaRuntime>,
    embedder_path: PathBuf,
    model_path: PathBuf,
    entry: &ModelEntry,
    corpus: &Corpus,
    speakers: &[Speaker],
    persona_bodies: bool,
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
        Npc::new(NpcParts {
            backend: Box::new(generator),
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
    models: Option<PathBuf>,
    deadline: Duration,
    persona_bodies: bool,
) -> anyhow::Result<()> {
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

    let (corpus, speakers, golden) = verify_fixture(fixture)?;
    let scenarios = load_scenarios(&fixture.join("scenarios.toml"))?;
    let rt = LlamaRuntime::init().map_err(|e| anyhow!("{e}"))?;
    let gpu = gpu_name();
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

    let mut all_defects = Vec::new();
    for entry in &file.models {
        let model_path = dir.join(&entry.file);
        let (mut worker, info) = spawn_worker(
            &rt,
            dir.join(&file.embedder),
            model_path.clone(),
            entry,
            &corpus,
            &speakers,
            persona_bodies,
        )?;
        let rows = run_model(&mut worker, &entry.name, &scenarios, &golden, deadline)?;
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
        };
        let suffix = if persona_bodies {
            ""
        } else {
            "-no-persona-bodies"
        };
        let out = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../results")
            .join(format!("{started}-{}{suffix}", slug(&entry.name)));
        report::write(&out, &header, &rows)?;
        println!("{}", report::summary_table(&rows));
        println!("results: {}", out.display());
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use theos_ai::testkit::{FakeClock, FakeEmbedder, FakeProbe, Script, ScriptedBackend};
    use theos_ai::{HolderKind, Lens, LoreKind, PersonaLayer, Phase, Piece};

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
            for _ in 0..18 {
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

        let rows = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
        )
        .unwrap();

        assert_eq!(rows.len(), 18);
        assert_eq!(
            rows.iter().filter(|r| r.thinking == Thinking::On).count(),
            9
        );
        assert_eq!(rows[0].scenario_id, "s1");
        assert_eq!(rows[0].scope, LoreScope::None);
        assert_eq!(rows[2].scope, LoreScope::Retrieved { top_k: 2 });
        assert_eq!(rows[0].checks.words, ["node"]);
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
        let rows = run_model(
            &mut worker,
            "fake",
            &scenarios,
            &golden,
            Duration::from_secs(3),
        )
        .unwrap();
        let found = defects(&rows);
        assert_eq!(
            found.len(),
            4,
            "AllVisible and Retrieved, thinking off and on: {found:?}"
        );
        assert!(found[0].contains("leak"));
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
