//! The results tables: `results.md`, `results.csv` and `answers.md`.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use theos_ai::{LoreScope, StopReason, Thinking};

use crate::run::Row;
use crate::scenarios::Behaviour;

/// What one run was made of, for the top of `results.md`.
#[derive(Clone, Debug)]
pub struct RunHeader {
    /// Model display name.
    pub model: String,
    /// Size of the GGUF file.
    pub file_bytes: u64,
    /// Parameter count of the model.
    pub n_params: u64,
    /// The GPU, as `nvidia-smi` names it.
    pub gpu: String,
    /// The per-request deadline.
    pub deadline: Duration,
    /// The sampler seed.
    pub seed: u32,
    /// Whether persona bodies went into the prompt.
    pub persona_bodies: bool,
    /// The VRAM probe in use, or `none`.
    pub probe: String,
}

/// Pass counts and timings for one thinking mode and scope.
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// Thinking mode.
    pub thinking: Thinking,
    /// Scope.
    pub scope: LoreScope,
    /// Requests in this group.
    pub n: usize,
    /// Retrieval passes (only `Retrieved`).
    pub retrieval: Option<usize>,
    /// Leak-check passes.
    pub leak: usize,
    /// Forbidden-record passes.
    pub forbidden: usize,
    /// Word-check passes.
    pub words: usize,
    /// Grounding passes.
    pub grounding: usize,
    /// Deadline passes.
    pub deadline: usize,
    /// VRAM passes and requests with a sample.
    pub vram: (usize, usize),
    /// Median time to first token, ms.
    pub ttft_median: Option<f64>,
    /// 95th percentile time to first token, ms.
    pub ttft_p95: Option<f64>,
    /// Median total time, ms.
    pub total_median: Option<f64>,
    /// 95th percentile total time, ms.
    pub total_p95: Option<f64>,
    /// Median tokens per second.
    pub tps_median: Option<f64>,
    /// Median prompt tokens.
    pub prompt_median: Option<f64>,
    /// Highest VRAM peak, bytes.
    pub vram_peak: Option<u64>,
}

fn thinking_label(t: Thinking) -> &'static str {
    match t {
        Thinking::Off => "off",
        _ => "on",
    }
}

fn scope_label(s: LoreScope) -> String {
    match s {
        LoreScope::None => "none".to_owned(),
        LoreScope::AllVisible => "all-visible".to_owned(),
        LoreScope::Retrieved { top_k } => format!("retrieved(top {top_k})"),
    }
}

fn stop_label(s: StopReason) -> &'static str {
    match s {
        StopReason::Finished => "finished",
        StopReason::Deadline => "deadline",
        StopReason::TokenCap => "token-cap",
        StopReason::ContextFull => "context-full",
        _ => "other",
    }
}

fn behaviour_label(b: Behaviour) -> &'static str {
    match b {
        Behaviour::Answer => "answer",
        Behaviour::Unaware => "unaware",
        Behaviour::Rumour => "rumour",
        Behaviour::Deflect => "deflect",
    }
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// Median of `v` (mean of the middle pair for an even count).
fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    Some(if v.len() % 2 == 1 {
        v[mid]
    } else {
        (v[mid - 1] + v[mid]) / 2.0
    })
}

/// Nearest-rank 95th percentile.
fn p95(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    // ceil(0.95 * n) is at least 1 and at most n.
    let rank = (v.len() * 95).div_ceil(100);
    Some(v[rank - 1])
}

/// One [`Summary`] per thinking mode and scope, in the order they first appear.
pub fn summarize(rows: &[Row]) -> Vec<Summary> {
    let mut keys: Vec<(Thinking, LoreScope)> = Vec::new();
    for r in rows {
        if !keys.contains(&(r.thinking, r.scope)) {
            keys.push((r.thinking, r.scope));
        }
    }
    keys.into_iter()
        .map(|(thinking, scope)| {
            let group: Vec<&Row> = rows
                .iter()
                .filter(|r| r.thinking == thinking && r.scope == scope)
                .collect();
            let count = |f: &dyn Fn(&Row) -> bool| group.iter().filter(|r| f(r)).count();
            let mut ttft: Vec<f64> = group
                .iter()
                .filter_map(|r| r.metrics.time_to_first_token.map(ms))
                .collect();
            let mut total: Vec<f64> = group.iter().map(|r| ms(r.metrics.total)).collect();
            let mut tps: Vec<f64> = group
                .iter()
                .map(|r| f64::from(r.metrics.tokens_per_second))
                .collect();
            let mut prompt: Vec<f64> = group
                .iter()
                .map(|r| f64::from(r.metrics.prompt_tokens))
                .collect();
            Summary {
                thinking,
                scope,
                n: group.len(),
                retrieval: match scope {
                    LoreScope::Retrieved { .. } => {
                        Some(count(&|r| r.checks.retrieval == Some(true)))
                    }
                    _ => None,
                },
                leak: count(&|r| r.checks.leak),
                forbidden: count(&|r| r.checks.forbidden),
                words: count(&|r| r.checks.words.is_empty()),
                grounding: count(&|r| r.checks.grounding),
                deadline: count(&|r| r.checks.deadline),
                vram: (
                    count(&|r| r.checks.vram == Some(true)),
                    count(&|r| r.checks.vram.is_some()),
                ),
                ttft_median: median(&mut ttft),
                ttft_p95: p95(&mut ttft),
                total_median: median(&mut total),
                total_p95: p95(&mut total),
                tps_median: median(&mut tps),
                prompt_median: median(&mut prompt),
                vram_peak: group.iter().filter_map(|r| r.metrics.vram_peak_bytes).max(),
            }
        })
        .collect()
}

fn opt_ms(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.0}"))
}

fn gib(bytes: u64) -> String {
    // Precision loss above 2^53 bytes is irrelevant for a display value.
    format!("{:.2} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

/// The summary as a markdown table.
pub fn summary_table(rows: &[Row]) -> String {
    let mut out = String::from(
        "| thinking | scope | n | retrieval | leak | forbidden | words | grounding | deadline | vram | ttft med/p95 ms | total med/p95 ms | tok/s med | prompt tok med | vram peak |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for s in summarize(rows) {
        let _ =
            writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} | {}/{} | {} | {} | {} |",
            thinking_label(s.thinking),
            scope_label(s.scope),
            s.n,
            s.retrieval.map_or_else(|| "n/a".to_owned(), |v| format!("{v}/{}", s.n)),
            format_args!("{}/{}", s.leak, s.n),
            format_args!("{}/{}", s.forbidden, s.n),
            format_args!("{}/{}", s.words, s.n),
            format_args!("{}/{}", s.grounding, s.n),
            format_args!("{}/{}", s.deadline, s.n),
            if s.vram.1 == 0 { "n/a".to_owned() } else { format!("{}/{}", s.vram.0, s.vram.1) },
            opt_ms(s.ttft_median),
            opt_ms(s.ttft_p95),
            opt_ms(s.total_median),
            opt_ms(s.total_p95),
            s.tps_median.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.1}")),
            s.prompt_median.map_or_else(|| "n/a".to_owned(), |v| format!("{v:.0}")),
            s.vram_peak.map_or_else(|| "n/a".to_owned(), gib),
        );
    }
    out
}

fn mark(pass: bool) -> &'static str {
    if pass {
        "pass"
    } else {
        "FAIL"
    }
}

fn mark_opt(v: Option<bool>) -> &'static str {
    v.map_or("n/a", mark)
}

/// `results.md`.
pub fn render_results(header: &RunHeader, rows: &[Row]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# NPC PoC results: {}\n", header.model);
    let _ = writeln!(out, "- Model: {}", header.model);
    let _ = writeln!(out, "- File size: {}", gib(header.file_bytes));
    let _ = writeln!(out, "- Parameters: {:.2} B", header.n_params as f64 / 1e9);
    let _ = writeln!(out, "- GPU: {}", header.gpu);
    let _ = writeln!(out, "- Deadline: {} ms", header.deadline.as_millis());
    let _ = writeln!(out, "- Seed: {}", header.seed);
    let _ = writeln!(
        out,
        "- Persona bodies: {}",
        if header.persona_bodies { "on" } else { "off" }
    );
    let _ = writeln!(out, "- VRAM probe: {}", header.probe);
    out.push_str("\n## Summary\n\n");
    out.push_str(&summary_table(rows));
    out.push_str("\n## Requests\n\n");
    out.push_str(
        "| scenario | npc | behaviour | thinking | scope | retrieval | leak | forbidden | words | grounding | deadline | vram | ttft ms | total ms | tok/s | prompt tok | stop | read by hand |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let c = &r.checks;
        let words = if c.words.is_empty() {
            "pass".to_owned()
        } else {
            format!("FAIL ({})", c.words.join(", "))
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {:.0} | {:.1} | {} | {} | {} |",
            r.scenario_id,
            r.npc,
            behaviour_label(r.behaviour),
            thinking_label(r.thinking),
            scope_label(r.scope),
            mark_opt(c.retrieval),
            mark(c.leak),
            mark(c.forbidden),
            words,
            if r.reply_error.is_some() { "FAIL (no reply)" } else { mark(c.grounding) },
            mark(c.deadline),
            mark_opt(c.vram),
            r.metrics.time_to_first_token.map_or_else(|| "n/a".to_owned(), |d| format!("{:.0}", ms(d))),
            ms(r.metrics.total),
            r.metrics.tokens_per_second,
            r.metrics.prompt_tokens,
            stop_label(r.metrics.stop),
            if r.hand_read { "yes" } else { "" },
        );
    }
    out
}

fn join_ids(ids: &[theos_ai::RecordId]) -> String {
    ids.iter()
        .map(theos_ai::RecordId::as_str)
        .collect::<Vec<_>>()
        .join(";")
}

/// `results.csv`: every field of every request, reply text included.
///
/// # Errors
///
/// A CSV serialisation failure.
pub fn render_csv(rows: &[Row]) -> anyhow::Result<String> {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record([
        "model",
        "thinking",
        "scope",
        "scenario",
        "npc",
        "behaviour",
        "hand_read",
        "retrieval",
        "leak",
        "forbidden",
        "words_found",
        "grounding",
        "deadline",
        "vram",
        "prompt_tokens",
        "thinking_tokens",
        "answer_tokens",
        "retrieval_ms",
        "ttft_ms",
        "tokens_per_second",
        "total_ms",
        "vram_peak_bytes",
        "stop",
        "context_ids",
        "cited_ids",
        "reply_error",
        "question",
        "reply_text",
        "raw",
    ])?;
    for r in rows {
        let c = &r.checks;
        let m = &r.metrics;
        w.write_record([
            r.model.clone(),
            thinking_label(r.thinking).to_owned(),
            scope_label(r.scope),
            r.scenario_id.clone(),
            r.npc.clone(),
            behaviour_label(r.behaviour).to_owned(),
            r.hand_read.to_string(),
            mark_opt(c.retrieval).to_owned(),
            mark(c.leak).to_owned(),
            mark(c.forbidden).to_owned(),
            c.words.join(";"),
            mark(c.grounding).to_owned(),
            mark(c.deadline).to_owned(),
            mark_opt(c.vram).to_owned(),
            m.prompt_tokens.to_string(),
            m.thinking_tokens.to_string(),
            m.answer_tokens.to_string(),
            format!("{:.1}", ms(m.retrieval)),
            m.time_to_first_token
                .map(|d| format!("{:.1}", ms(d)))
                .unwrap_or_default(),
            format!("{:.2}", m.tokens_per_second),
            format!("{:.1}", ms(m.total)),
            m.vram_peak_bytes.map(|b| b.to_string()).unwrap_or_default(),
            stop_label(m.stop).to_owned(),
            join_ids(&m.context_ids),
            join_ids(&m.cited_ids),
            r.reply_error.clone().unwrap_or_default(),
            r.question.clone(),
            r.reply_text.clone(),
            r.raw.clone(),
        ])?;
    }
    Ok(String::from_utf8(
        w.into_inner().map_err(|e| anyhow::anyhow!("{e}"))?,
    )?)
}

/// `answers.md`: per scenario, every condition's reply and cites.
pub fn render_answers(rows: &[Row]) -> String {
    let mut ids: Vec<&str> = Vec::new();
    for r in rows {
        if !ids.contains(&r.scenario_id.as_str()) {
            ids.push(&r.scenario_id);
        }
    }
    let mut out =
        String::from("# Answers\n\nScenarios marked \"read by hand\" need a human reader.\n");
    for id in ids {
        let group: Vec<&Row> = rows.iter().filter(|r| r.scenario_id == id).collect();
        let first = group[0];
        let _ = write!(
            out,
            "\n## {id}: {} ({}){}\n\n**Question:** {}\n",
            first.npc,
            behaviour_label(first.behaviour),
            if first.hand_read {
                " - read by hand"
            } else {
                ""
            },
            first.question,
        );
        if !first.note.is_empty() {
            let _ = writeln!(out, "\n**Note:** {}", first.note);
        }
        for r in group {
            let _ = write!(
                out,
                "\n### thinking {}, {}\n\n",
                thinking_label(r.thinking),
                scope_label(r.scope)
            );
            match &r.reply_error {
                Some(e) => {
                    let _ = writeln!(out, "_No reply: {e}_");
                }
                None => {
                    for line in r.reply_text.lines() {
                        let _ = writeln!(out, "> {line}");
                    }
                }
            }
            let cites = join_ids(&r.metrics.cited_ids);
            let _ = writeln!(
                out,
                "\nCites: {}",
                if cites.is_empty() {
                    "none".to_owned()
                } else {
                    cites.replace(';', ", ")
                }
            );
            if !r.checks.words.is_empty() {
                let _ = writeln!(out, "Forbidden words: {}", r.checks.words.join(", "));
            }
        }
    }
    out
}

/// Writes `results.md`, `results.csv` and `answers.md` into `dir`.
///
/// # Errors
///
/// When the directory or a file cannot be written.
pub fn write(dir: &Path, header: &RunHeader, rows: &[Row]) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let files = [
        ("results.md", render_results(header, rows)),
        ("results.csv", render_csv(rows)?),
        ("answers.md", render_answers(rows)),
    ];
    for (name, text) in files {
        let path = dir.join(name);
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Checks;
    use theos_ai::{Metrics, RecordId};

    fn row(thinking: Thinking, scope: LoreScope, grounding: bool, total_ms: u64) -> Row {
        Row {
            model: "m".into(),
            thinking,
            scope,
            scenario_id: "s01".into(),
            npc: "n".into(),
            behaviour: Behaviour::Answer,
            question: "what, \"exactly\"?".into(),
            note: "a note".into(),
            checks: Checks {
                retrieval: matches!(scope, LoreScope::Retrieved { .. }).then_some(true),
                leak: true,
                forbidden: true,
                words: vec![],
                grounding,
                deadline: true,
                vram: Some(true),
            },
            metrics: Metrics {
                prompt_tokens: 100,
                thinking_tokens: 0,
                answer_tokens: 20,
                retrieval: Duration::from_millis(5),
                time_to_first_token: Some(Duration::from_millis(total_ms / 2)),
                tokens_per_second: 40.0,
                total: Duration::from_millis(total_ms),
                vram_peak_bytes: Some(4 * 1024 * 1024 * 1024),
                context_ids: vec![RecordId::new("B-1")],
                cited_ids: vec![RecordId::new("B-1")],
                stop: StopReason::Finished,
            },
            reply_text: "Well, it is so,\nbelieve me.".into(),
            reply_error: None,
            raw: "{\"raw\":1}".into(),
            hand_read: false,
        }
    }

    #[test]
    fn csv_quotes_replies_with_commas_and_newlines() {
        let rows = [row(Thinking::Off, LoreScope::None, true, 400)];
        let text = render_csv(&rows).unwrap();
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        let header = reader.headers().unwrap().clone();
        let record = reader.records().next().unwrap().unwrap();
        let col = |name: &str| {
            record
                .get(header.iter().position(|h| h == name).unwrap())
                .unwrap()
        };
        assert_eq!(col("reply_text"), "Well, it is so,\nbelieve me.");
        assert_eq!(col("question"), "what, \"exactly\"?");
        assert_eq!(col("raw"), "{\"raw\":1}");
        assert_eq!(reader.records().count(), 0);
    }

    #[test]
    fn summary_counts_passes_per_thinking_and_scope() {
        let rows = [
            row(Thinking::Off, LoreScope::None, true, 100),
            row(Thinking::Off, LoreScope::None, false, 300),
            row(Thinking::Off, LoreScope::None, true, 200),
            row(Thinking::On, LoreScope::Retrieved { top_k: 5 }, true, 900),
        ];
        let s = summarize(&rows);
        assert_eq!(s.len(), 2);
        assert_eq!(
            (s[0].thinking, s[0].scope, s[0].n),
            (Thinking::Off, LoreScope::None, 3)
        );
        assert_eq!(s[0].grounding, 2);
        assert_eq!(s[0].retrieval, None);
        assert_eq!(s[0].total_median, Some(200.0));
        assert_eq!(s[0].total_p95, Some(300.0));
        assert_eq!(s[0].vram, (3, 3));
        assert_eq!(s[1].retrieval, Some(1));
        assert_eq!(s[1].grounding, 1);
        assert_eq!(s[1].vram_peak, Some(4 * 1024 * 1024 * 1024));
    }

    #[test]
    fn answers_mark_hand_read_scenarios_and_missing_replies() {
        let mut a = row(Thinking::Off, LoreScope::None, true, 100);
        a.hand_read = true;
        let mut b = row(Thinking::On, LoreScope::None, false, 100);
        b.hand_read = true;
        b.reply_text.clear();
        b.reply_error = Some("generation did not finish: Deadline".into());
        let text = render_answers(&[a, b]);
        assert!(text.contains("## s01: n (answer) - read by hand"));
        assert!(text.contains("> Well, it is so,"));
        assert!(text.contains("_No reply: generation did not finish: Deadline_"));
        assert!(text.contains("Cites: B-1"));
    }

    #[test]
    fn write_creates_the_three_files() {
        let dir = std::env::temp_dir().join(format!("npc-poc-report-{}", std::process::id()));
        let header = RunHeader {
            model: "Fake 4B".into(),
            file_bytes: 2_500_000_000,
            n_params: 4_000_000_000,
            gpu: "Test GPU".into(),
            deadline: Duration::from_secs(3),
            seed: 42,
            persona_bodies: true,
            probe: "DXGI".into(),
        };
        let rows = [row(Thinking::Off, LoreScope::AllVisible, true, 400)];
        write(&dir, &header, &rows).unwrap();
        let md = std::fs::read_to_string(dir.join("results.md")).unwrap();
        assert!(md.contains("- Seed: 42"));
        assert!(md.contains("## Summary"));
        assert!(dir.join("results.csv").is_file());
        assert!(dir.join("answers.md").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
