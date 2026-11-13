//! Converts the lore-poc content project into `theos-ai` records and speakers.

use std::path::Path;

use anyhow::{anyhow, Context};
use theos_ai::{HolderKind, Lens, LoreKind, LoreRecord, PersonaLayer, RecordId, Speaker};
use theos_content::yaml::Value;
use theos_content::{parts, read_record, Index, LensData, Project, Record, RecordSummary, Span};

/// Reads the content project at `records_dir` and converts it.
///
/// Records of kind belief, truth, clue and topic become [`LoreRecord`]s; every `npc` record
/// becomes a [`Speaker`] through [`Index::lens`].
pub fn load_fixture(records_dir: &Path) -> anyhow::Result<(Vec<LoreRecord>, Vec<Speaker>)> {
    let project = Project::open(records_dir).map_err(|e| anyhow!("{e}"))?;
    let registry = parts::builtin().map_err(|e| anyhow!("{e}"))?;
    let (index, _) = Index::open(&project, &registry).map_err(|e| anyhow!("{e}"))?;
    let summaries = index.records().map_err(|e| anyhow!("{e}"))?;

    let mut lore = Vec::new();
    let mut speakers = Vec::new();
    for summary in &summaries {
        let Some(kind) = summary.kind.as_deref() else {
            continue;
        };
        if let Some(lore_kind) = lore_kind(kind) {
            lore.push(convert_lore(&project, summary, lore_kind)?);
        } else if kind == "npc" {
            speakers.push(convert_speaker(&project, &index, summary)?);
        }
    }
    Ok((lore, speakers))
}

fn lore_kind(kind: &str) -> Option<LoreKind> {
    match kind {
        "belief" => Some(LoreKind::Belief),
        "truth" => Some(LoreKind::Truth),
        "clue" => Some(LoreKind::Clue),
        "topic" => Some(LoreKind::Topic),
        _ => None,
    }
}

fn read(project: &Project, path: &str) -> anyhow::Result<Record> {
    read_record(&project.root().join(path)).with_context(|| format!("reading record {path}"))
}

/// The text of the scalar field `key`, if the front matter has one.
fn scalar(record: &Record, key: &str) -> Option<String> {
    match &record.front_matter().ok()?.get(key)?.value {
        Value::Scalar(s) => Some(s.text.clone()),
        _ => None,
    }
}

/// The items of the list field `key`; a lone scalar counts as a one-item list.
fn list(record: &Record, key: &str) -> Vec<String> {
    let Some(field) = record.front_matter().ok().and_then(|f| f.get(key)) else {
        return Vec::new();
    };
    match &field.value {
        Value::List(l) => l.items.iter().map(|i| i.scalar.text.clone()).collect(),
        Value::Scalar(s) => vec![s.text.clone()],
        Value::Map(_) => Vec::new(),
    }
}

/// The body text with every callout cut out, trimmed.
fn body_text(record: &Record) -> String {
    let text = record.text();
    let body = record.body_span();
    let mut cuts: Vec<Span> = record.body().callouts().iter().map(|c| c.span).collect();
    cuts.sort();
    let mut out = String::new();
    let mut at = body.start;
    for cut in cuts {
        if cut.start < at {
            continue; // nested in a callout already cut
        }
        out.push_str(text.get(at..cut.start).unwrap_or_default());
        at = cut.end;
    }
    out.push_str(text.get(at..body.end).unwrap_or_default());
    out.trim().to_string()
}

/// The in-world lines of every `[!voice]` callout, header line and quote markers removed.
fn voice_lines(record: &Record) -> Vec<String> {
    let text = record.text();
    record
        .body()
        .callouts()
        .iter()
        .filter(|c| c.name == "voice")
        .filter_map(|c| c.content_span.get(text))
        .flat_map(str::lines)
        .map(|line| line.trim_start_matches('>').trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn convert_lore(
    project: &Project,
    summary: &RecordSummary,
    kind: LoreKind,
) -> anyhow::Result<LoreRecord> {
    let record = read(project, &summary.path)?;
    Ok(LoreRecord {
        id: RecordId::new(&summary.id),
        kind,
        title: summary.title.clone().unwrap_or_default(),
        body: body_text(&record),
        voice: voice_lines(&record),
        held_by: list(&record, "held_by")
            .into_iter()
            .map(RecordId::new)
            .collect(),
        target: scalar(&record, "target"),
    })
}

fn holder_kind(kind: &str) -> anyhow::Result<HolderKind> {
    match kind {
        "npc" => Ok(HolderKind::Npc),
        "faction" => Ok(HolderKind::Faction),
        "people" => Ok(HolderKind::People),
        other => Err(anyhow!("unexpected holder kind `{other}`")),
    }
}

fn convert_speaker(
    project: &Project,
    index: &Index,
    summary: &RecordSummary,
) -> anyhow::Result<Speaker> {
    let effective = index
        .lens(&summary.id)
        .map_err(|e| anyhow!("{e}"))?
        .ok_or_else(|| anyhow!("no lens for speaker `{}`", summary.id))?;

    let mut layers = Vec::new();
    for layer in &effective.chain {
        let record = read(project, &layer.path)?;
        layers.push(PersonaLayer {
            id: RecordId::new(&layer.id),
            kind: holder_kind(&layer.kind)?,
            title: scalar(&record, "title").unwrap_or_default(),
            role: scalar(&record, "role"),
            body: body_text(&record),
        });
    }

    let mut lens = Lens::default();
    for field in &effective.fields {
        match (field.field.as_str(), &field.value) {
            ("thinks_from", LensData::Text(t)) => lens.thinks_from = Some(t.clone()),
            ("stance", LensData::Text(t)) => lens.stance = Some(t.clone()),
            ("speaks_to", LensData::Text(t)) => lens.speaks_to = Some(t.clone()),
            ("tier", LensData::Text(t)) => lens.tier = Some(t.clone()),
            ("rejects", LensData::Links(ids)) => {
                lens.rejects = ids.iter().map(RecordId::new).collect();
            }
            ("unaware_of", LensData::Links(ids)) => {
                lens.unaware_of = ids.iter().map(RecordId::new).collect();
            }
            _ => {}
        }
    }

    Ok(Speaker {
        id: RecordId::new(&summary.id),
        layers,
        lens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use theos_ai::LoreKind;

    fn load() -> (Vec<LoreRecord>, Vec<Speaker>) {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/lore-poc/records");
        load_fixture(&dir).unwrap()
    }

    fn count(records: &[LoreRecord], kind: LoreKind) -> usize {
        records.iter().filter(|r| r.kind == kind).count()
    }

    #[test]
    fn fixture_converts_to_beliefs_truths_clues_topics_and_eight_speakers() {
        let (records, speakers) = load();
        assert_eq!(count(&records, LoreKind::Belief), 53);
        assert_eq!(count(&records, LoreKind::Truth), 25);
        assert_eq!(count(&records, LoreKind::Clue), 10);
        assert_eq!(count(&records, LoreKind::Topic), 12);
        assert_eq!(records.len(), 100);
        assert_eq!(speakers.len(), 8);
    }

    #[test]
    fn binder_speaker_has_two_layers_and_throat_kin_lens() {
        let (_, speakers) = load();
        let binder = speakers
            .iter()
            .find(|s| s.id.as_str() == "fx-binder")
            .unwrap();
        let ids: Vec<&str> = binder.layers.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["fx-binder", "throat-kin"]);
        assert!(binder
            .lens
            .thinks_from
            .as_deref()
            .unwrap()
            .starts_with("The world is thick with Listeners"));
    }

    #[test]
    fn b942_voice_has_saying_without_header() {
        let (records, _) = load();
        let b = records.iter().find(|r| r.id.as_str() == "B-942").unwrap();
        assert!(b.voice.iter().any(|l| l.contains(
            "A binder casts nothing. A binder only sings until something stronger says yes."
        )));
        assert!(b.voice.iter().all(|l| !l.contains("[!voice]")));
    }

    #[test]
    fn t913_target_is_never() {
        let (records, _) = load();
        let t = records.iter().find(|r| r.id.as_str() == "T-913").unwrap();
        assert_eq!(t.target.as_deref(), Some("never"));
    }
}
