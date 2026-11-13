//! Scoring of a tree reply: the fixture's own `check.py` for gate and behaviour, plus the
//! checks only the harness can make (against the actual prompt).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::{anyhow, bail, Context};
use serde::Deserialize;
use serde_json::Value;
use theos_ai::RecordId;

use crate::fixture::VisibleEntry;

/// What `score.py` makes of one reply envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyScore {
    pub gate_pass: bool,
    pub gate_rule: Option<String>,
    pub nodes: Option<u32>,
    pub depth: Option<u32>,
    pub checks: Vec<bool>,
    pub behaviour: Option<bool>,
    pub grounding: Option<bool>,
}

#[derive(Deserialize)]
struct Raw {
    gate: String,
    gate_rule: Option<String>,
    nodes: Option<u32>,
    depth: Option<u32>,
    #[serde(default)]
    checks: Vec<bool>,
    behaviour: Option<bool>,
    grounding: Option<bool>,
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Scores `envelope` for `scenario_id` by running `score.py` (isolated Python) over the
/// fixture's `check.py`. `scenarios_file` may be any file of the scenarios.toml shape.
pub fn score_envelope(
    fixture_dir: &Path,
    scenarios_file: &Path,
    scenario_id: &str,
    envelope: &Value,
) -> anyhow::Result<PyScore> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("score.py");
    let tmp = std::env::temp_dir().join(format!(
        "bt-poc-envelope-{}-{}.json",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, serde_json::to_vec(envelope)?)
        .with_context(|| format!("writing {}", tmp.display()))?;
    let run = |program: &str, pre: &[&str]| {
        Command::new(program)
            .args(pre)
            .arg("-I")
            .arg("-B") // no __pycache__ inside the fixture
            .arg(&script)
            .arg("--fixture")
            .arg(fixture_dir)
            .arg("--scenarios")
            .arg(scenarios_file)
            .arg("--scenario")
            .arg(scenario_id)
            .arg(&tmp)
            .output()
    };
    // `python` can be an unusable Windows Store stub; fall back to the `py` launcher.
    let out = match std::env::var("BT_POC_PYTHON") {
        Ok(p) => run(&p, &[]),
        Err(_) => run("python", &[]).or_else(|_| run("py", &["-3"])),
    };
    let _ = std::fs::remove_file(&tmp);
    let out = out.context("running python score.py")?;
    if !out.status.success() {
        bail!(
            "score.py failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let raw: Raw = serde_json::from_slice(&out.stdout)
        .map_err(|e| anyhow!("score.py printed unparsable output: {e}"))?;
    Ok(PyScore {
        gate_pass: raw.gate == "pass",
        gate_rule: raw.gate_rule,
        nodes: raw.nodes,
        depth: raw.depth,
        checks: raw.checks,
        behaviour: raw.behaviour,
        grounding: raw.grounding,
    })
}

/// README gate rule 7 against the actual prompt: every cited source is a lore id the prompt held.
pub fn sources_in_prompt(envelope: &Value, lore_ids: &[RecordId]) -> bool {
    let Some(sources) = envelope.get("sources").and_then(Value::as_array) else {
        return false;
    };
    sources.iter().all(|s| {
        s.as_str()
            .is_some_and(|s| lore_ids.iter().any(|id| id.as_str() == s))
    })
}

/// True when a lore record in the prompt is not visible (held or rejected) to the actor.
pub fn leak(lore_ids: &[RecordId], visible_entry: &VisibleEntry) -> bool {
    lore_ids.iter().any(|id| {
        let id = id.as_str();
        !visible_entry.held.iter().any(|h| h == id)
            && !visible_entry.rejected.iter().any(|r| r == id)
    })
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// True when `tree_name` shares a run of `n_words` consecutive words (case- and
/// punctuation-insensitive) with any one of `system_parts`.
pub fn name_overlap(tree_name: &str, system_parts: &[&str], n_words: usize) -> bool {
    let name = words(tree_name);
    if n_words == 0 || name.len() < n_words {
        return false;
    }
    let runs: BTreeSet<&[String]> = name.windows(n_words).collect();
    system_parts.iter().any(|part| {
        let w = words(part);
        w.windows(n_words).any(|run| runs.contains(run))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};
    use theos_ai::RecordId;

    fn fixture_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/bt-poc")
    }

    fn reference(name: &str) -> Value {
        let text =
            std::fs::read_to_string(fixture_dir().join(format!("reference/{name}.json"))).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    fn score_reference(name: &str) -> PyScore {
        let r = reference(name);
        score_envelope(
            &fixture_dir(),
            &fixture_dir().join("scenarios.toml"),
            r["scenario"].as_str().unwrap(),
            &r["reply"],
        )
        .unwrap()
    }

    #[test]
    fn score_py_passes_every_good_reference_tree() {
        let mut n = 0;
        for entry in std::fs::read_dir(fixture_dir().join("reference")).unwrap() {
            let path = entry.unwrap().path();
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            if !stem.ends_with("-good") {
                continue;
            }
            let s = score_reference(&stem);
            assert!(s.gate_pass, "{stem}: gate {:?}", s.gate_rule);
            assert_eq!(s.behaviour, Some(true), "{stem}: {:?}", s.checks);
            assert!(s.nodes.is_some() && s.depth.is_some());
            n += 1;
        }
        assert_eq!(n, 20);
    }

    #[test]
    fn score_py_matches_expected_reference_for_bad_trees() {
        let golden: Value = serde_json::from_str(
            &std::fs::read_to_string(fixture_dir().join("expected/reference.json")).unwrap(),
        )
        .unwrap();
        for name in ["b01-bad", "b07-bad", "b13-bad", "g1-gate"] {
            let s = score_reference(name);
            let want = &golden[name];
            assert_eq!(s.gate_pass, want["gate"] == "pass", "{name}");
            assert_eq!(s.gate_rule.as_deref(), want["gate_rule"].as_str(), "{name}");
            assert_eq!(s.behaviour, want["behaviour"].as_bool(), "{name}");
            let checks: Vec<bool> = want["checks"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_bool().unwrap())
                .collect();
            assert_eq!(s.checks, checks, "{name}");
        }
        assert_eq!(score_reference("b01-bad").behaviour, Some(false));
        assert!(!score_reference("g1-gate").gate_pass);
    }

    #[test]
    fn score_py_reads_another_scenarios_file() {
        // The file has CRLF line endings on Windows checkouts.
        let text = std::fs::read_to_string(fixture_dir().join("scenarios.toml"))
            .unwrap()
            .replace("\r\n", "\n");
        let start = text.find("[[scenario]]\nid = \"b01\"").unwrap();
        let end = text[start + 12..]
            .find("[[scenario]]")
            .map(|i| start + 12 + i)
            .unwrap_or(text.len());
        let copy = text[start..end].replacen("id = \"b01\"", "id = \"zz9\"", 1);
        let dir = std::env::temp_dir().join(format!("bt-poc-score-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("other.toml");
        std::fs::write(&file, copy).unwrap();
        let r = reference("b01-good");
        let other = score_envelope(&fixture_dir(), &file, "zz9", &r["reply"]).unwrap();
        let same = score_reference("b01-good");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(other.gate_pass);
        assert_eq!(other, same);
    }

    #[test]
    fn name_overlap_trips_on_eight_shared_words_only() {
        let parts = ["Mara keeps the ledger of the old well, and Never lends it."];
        // 8 shared words, differing case and punctuation.
        assert!(name_overlap(
            "MARA keeps; the ledger: of the old well",
            &parts,
            8
        ));
        // 7 shared words only.
        assert!(!name_overlap("mara keeps the ledger of the old", &parts, 8));
        // Words shared but not as one run.
        assert!(!name_overlap(
            "mara keeps the ledger of the old x well and",
            &parts,
            8
        ));
        // A run split across parts does not count.
        let split = ["one two three four", "five six seven eight"];
        assert!(!name_overlap(
            "one two three four five six seven eight",
            &split,
            8
        ));
    }

    #[test]
    fn sources_outside_the_prompt_fail() {
        let ids = vec![RecordId::new("a"), RecordId::new("b")];
        assert!(sources_in_prompt(
            &json!({"sources": ["a", "b"], "tree": {}}),
            &ids
        ));
        assert!(sources_in_prompt(&json!({"sources": [], "tree": {}}), &ids));
        assert!(!sources_in_prompt(
            &json!({"sources": ["a", "c"], "tree": {}}),
            &ids
        ));
        assert!(!sources_in_prompt(&json!({"tree": {}}), &ids));
    }

    #[test]
    fn leak_fails_for_a_record_not_visible_to_the_actor() {
        let entry = crate::fixture::VisibleEntry {
            persona: vec![],
            held: vec!["h1".into()],
            rejected: vec!["r1".into()],
            unaware_of: vec![],
        };
        assert!(!leak(&[RecordId::new("h1"), RecordId::new("r1")], &entry));
        assert!(leak(&[RecordId::new("h1"), RecordId::new("x9")], &entry));
    }
}
