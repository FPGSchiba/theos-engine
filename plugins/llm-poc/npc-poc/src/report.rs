//! The results tables: `results.md`, `results.csv` and `answers.md`.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use theos_ai::{LoreScope, StopReason, Thinking};

use crate::run::Row;
use crate::scenarios::Behaviour;
use crate::score::Checks;

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
    /// The unrecorded warm-up request, when one was sent.
    pub warm_up: Option<WarmUp>,
}

/// The one request sent before measuring, so the first real request is not cold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarmUp {
    /// Its total time.
    pub total: Duration,
    /// Its stop reason, or why it failed.
    pub stop: String,
}

/// Passes out of the rows a check applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tally {
    /// Rows that passed.
    pub pass: usize,
    /// Rows the check applies to.
    pub of: usize,
}

impl std::fmt::Display for Tally {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.of == 0 {
            f.write_str("n/a")
        } else {
            write!(f, "{}/{}", self.pass, self.of)
        }
    }
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
    /// Requests the driver failed outright: no measurements and no checks, so
    /// they are left out of every check's count and of the timings.
    pub errors: usize,
    /// Retrieval passes (only `Retrieved`).
    pub retrieval: Option<Tally>,
    /// Leak-check passes.
    pub leak: Tally,
    /// Forbidden-record passes.
    pub forbidden: Tally,
    /// Word-check passes; rows without a reply have no words to check.
    pub words: Tally,
    /// Grounding passes; a row without a reply fails.
    pub grounding: Tally,
    /// Deadline passes.
    pub deadline: Tally,
    /// VRAM passes out of requests with a sample.
    pub vram: Tally,
    /// Median time to first token, ms.
    pub ttft_median: Option<f64>,
    /// 95th percentile time to first token, ms.
    pub ttft_p95: Option<f64>,
    /// Median time to first answer token, ms.
    pub ttfa_median: Option<f64>,
    /// 95th percentile time to first answer token, ms.
    pub ttfa_p95: Option<f64>,
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

pub(crate) fn stop_label(s: StopReason) -> &'static str {
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
        Behaviour::Resist => "resist",
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
            let mut ttft: Vec<f64> = group
                .iter()
                .filter_map(|r| r.metrics.as_ref()?.time_to_first_token.map(ms))
                .collect();
            let mut ttfa: Vec<f64> = group
                .iter()
                .filter_map(|r| r.metrics.as_ref()?.time_to_first_answer_token.map(ms))
                .collect();
            let mut total: Vec<f64> = group
                .iter()
                .filter_map(|r| r.metrics.as_ref().map(|m| ms(m.total)))
                .collect();
            // Fewer than two generated tokens leave nothing to measure a rate on.
            let mut tps: Vec<f64> = group
                .iter()
                .filter_map(|r| r.metrics.as_ref())
                .filter(|m| m.thinking_tokens + m.answer_tokens >= 2)
                .map(|m| f64::from(m.tokens_per_second))
                .collect();
            let mut prompt: Vec<f64> = group
                .iter()
                .filter_map(|r| r.metrics.as_ref().map(|m| f64::from(m.prompt_tokens)))
                .collect();
            // Passes out of the rows where the check applies (`None` = does not).
            let tally = |f: &dyn Fn(&Checks) -> Option<bool>| {
                let outcomes: Vec<bool> = group
                    .iter()
                    .filter_map(|r| r.checks.as_ref())
                    .filter_map(f)
                    .collect();
                Tally {
                    pass: outcomes.iter().filter(|p| **p).count(),
                    of: outcomes.len(),
                }
            };
            Summary {
                thinking,
                scope,
                n: group.len(),
                errors: group.iter().filter(|r| r.checks.is_none()).count(),
                retrieval: match scope {
                    LoreScope::Retrieved { .. } => Some(tally(&|c| c.retrieval)),
                    _ => None,
                },
                leak: tally(&|c| Some(c.leak)),
                forbidden: tally(&|c| Some(c.forbidden)),
                words: tally(&|c| c.words.as_ref().map(Vec::is_empty)),
                grounding: tally(&|c| Some(c.grounding)),
                deadline: tally(&|c| Some(c.deadline)),
                vram: tally(&|c| c.vram),
                ttft_median: median(&mut ttft),
                ttft_p95: p95(&mut ttft),
                ttfa_median: median(&mut ttfa),
                ttfa_p95: p95(&mut ttfa),
                total_median: median(&mut total),
                total_p95: p95(&mut total),
                tps_median: median(&mut tps),
                prompt_median: median(&mut prompt),
                vram_peak: group
                    .iter()
                    .filter_map(|r| r.metrics.as_ref()?.vram_peak_bytes)
                    .max(),
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
        "| thinking | scope | n | errors | retrieval | leak | forbidden | words | grounding | deadline | vram | ttft med/p95 ms | ttfa med/p95 ms | total med/p95 ms | tok/s med | prompt tok med | vram peak |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for s in summarize(rows) {
        let _ =
            writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} | {}/{} | {}/{} | {} | {} | {} |",
            thinking_label(s.thinking),
            scope_label(s.scope),
            s.n,
            s.errors,
            s.retrieval.map_or_else(|| "n/a".to_owned(), |t| t.to_string()),
            s.leak,
            s.forbidden,
            s.words,
            s.grounding,
            s.deadline,
            s.vram,
            opt_ms(s.ttft_median),
            opt_ms(s.ttft_p95),
            opt_ms(s.ttfa_median),
            opt_ms(s.ttfa_p95),
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
    if let Some(w) = &header.warm_up {
        let _ = writeln!(
            out,
            "- warm-up: {:.0} ms (not in the table), stop: {}",
            ms(w.total),
            w.stop
        );
    }
    out.push_str("\n## Grounding rules\n\n");
    out.push_str(
        "- An empty cite list fails `answer` and `rumour`: an answer must rest on records.\n\
         - `unaware` and `deflect` pass with no cites or only `also_ok` cites.\n\
         - In scope `none` the schema forbids sources, so an `answer` scenario with a non-empty `expect` cannot ground there. That is the expected baseline, not a model defect.\n",
    );
    out.push_str("\n## Summary\n\n");
    out.push_str(
        "Each check shows passes/applicable. Requests the driver failed outright (errors) have no measurements and no checks, so they are left out of every check and of the timings. A request with no usable reply (deadline, token cap, malformed output) fails grounding; it has no text, so the words check does not apply to it (words is n/a, not a pass). The tok/s median leaves out requests with fewer than 2 generated tokens: no rate was measured for them. ttfa is the time to the first answer token (with thinking on, the first thinking token comes earlier: ttft).\n\n",
    );
    out.push_str(&summary_table(rows));
    out.push_str("\n## Requests\n\n");
    out.push_str(
        "| scenario | npc | behaviour | thinking | scope | retrieval | leak | forbidden | words | grounding | deadline | vram | ttft ms | ttfa ms | total ms | tok/s | prompt tok | stop | read by hand |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let c = r.checks.as_ref();
        let words = match c {
            None => "n/a".to_owned(),
            Some(c) => match &c.words {
                None => "n/a (no reply)".to_owned(),
                Some(w) if w.is_empty() => "pass".to_owned(),
                Some(w) => format!("FAIL ({})", w.join(", ")),
            },
        };
        let na = || "n/a".to_owned();
        let stop = match (&r.metrics, &r.reply_error, c) {
            (_, Some(e), None) => format!("ERROR: {e}"),
            (Some(m), _, _) => stop_label(m.stop).to_owned(),
            _ => na(),
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.scenario_id,
            r.npc,
            behaviour_label(r.behaviour),
            thinking_label(r.thinking),
            scope_label(r.scope),
            c.map_or_else(na, |c| mark_opt(c.retrieval).to_owned()),
            c.map_or_else(na, |c| mark(c.leak).to_owned()),
            c.map_or_else(na, |c| mark(c.forbidden).to_owned()),
            words,
            match c {
                None => "FAIL".to_owned(),
                Some(_) if r.reply_error.is_some() => "FAIL (no reply)".to_owned(),
                Some(c) => mark(c.grounding).to_owned(),
            },
            mark(c.is_some_and(|c| c.deadline)),
            c.map_or_else(na, |c| mark_opt(c.vram).to_owned()),
            r.metrics
                .as_ref()
                .and_then(|m| m.time_to_first_token)
                .map_or_else(na, |d| format!("{:.0}", ms(d))),
            r.metrics
                .as_ref()
                .and_then(|m| m.time_to_first_answer_token)
                .map_or_else(na, |d| format!("{:.0}", ms(d))),
            r.metrics.as_ref().map_or_else(na, |m| format!("{:.0}", ms(m.total))),
            r.metrics
                .as_ref()
                .map_or_else(na, |m| format!("{:.1}", m.tokens_per_second)),
            r.metrics.as_ref().map_or_else(na, |m| m.prompt_tokens.to_string()),
            stop,
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
        "ttfa_ms",
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
        let c = r.checks.as_ref();
        let m = r.metrics.as_ref();
        let na = || "n/a".to_owned();
        let num = |f: &dyn Fn(&theos_ai::Metrics) -> String| m.map(f).unwrap_or_default();
        w.write_record([
            r.model.clone(),
            thinking_label(r.thinking).to_owned(),
            scope_label(r.scope),
            r.scenario_id.clone(),
            r.npc.clone(),
            behaviour_label(r.behaviour).to_owned(),
            r.hand_read.to_string(),
            c.map_or_else(na, |c| mark_opt(c.retrieval).to_owned()),
            c.map_or_else(na, |c| mark(c.leak).to_owned()),
            c.map_or_else(na, |c| mark(c.forbidden).to_owned()),
            c.and_then(|c| c.words.as_ref())
                .map(|w| w.join(";"))
                .unwrap_or_default(),
            mark(c.is_some_and(|c| c.grounding)).to_owned(),
            mark(c.is_some_and(|c| c.deadline)).to_owned(),
            c.map_or_else(na, |c| mark_opt(c.vram).to_owned()),
            num(&|m| m.prompt_tokens.to_string()),
            num(&|m| m.thinking_tokens.to_string()),
            num(&|m| m.answer_tokens.to_string()),
            num(&|m| format!("{:.1}", ms(m.retrieval))),
            num(&|m| {
                m.time_to_first_token
                    .map(|d| format!("{:.1}", ms(d)))
                    .unwrap_or_default()
            }),
            num(&|m| {
                m.time_to_first_answer_token
                    .map(|d| format!("{:.1}", ms(d)))
                    .unwrap_or_default()
            }),
            num(&|m| format!("{:.2}", m.tokens_per_second)),
            num(&|m| format!("{:.1}", ms(m.total))),
            num(&|m| m.vram_peak_bytes.map(|b| b.to_string()).unwrap_or_default()),
            num(&|m| stop_label(m.stop).to_owned()),
            num(&|m| join_ids(&m.context_ids)),
            num(&|m| join_ids(&m.cited_ids)),
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
            let cites = r
                .metrics
                .as_ref()
                .map(|m| join_ids(&m.cited_ids))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "\nCites: {}",
                if cites.is_empty() {
                    "none".to_owned()
                } else {
                    cites.replace(';', ", ")
                }
            );
            if let Some(w) = r
                .checks
                .as_ref()
                .and_then(|c| c.words.as_ref())
                .filter(|w| !w.is_empty())
            {
                let _ = writeln!(out, "Forbidden words: {}", w.join(", "));
            }
        }
    }
    out
}

/// One row as a JSON object: the line `rows.jsonl` gets as soon as the row exists.
pub fn row_json(r: &Row) -> serde_json::Value {
    use serde_json::json;
    let checks = r.checks.as_ref().map(|c| {
        json!({
            "retrieval": c.retrieval,
            "leak": c.leak,
            "forbidden": c.forbidden,
            "words": c.words,
            "grounding": c.grounding,
            "deadline": c.deadline,
            "vram": c.vram,
        })
    });
    let metrics = r.metrics.as_ref().map(|m| {
        json!({
            "prompt_tokens": m.prompt_tokens,
            "thinking_tokens": m.thinking_tokens,
            "answer_tokens": m.answer_tokens,
            "retrieval_ms": ms(m.retrieval),
            "ttft_ms": m.time_to_first_token.map(ms),
            "ttfa_ms": m.time_to_first_answer_token.map(ms),
            "tokens_per_second": m.tokens_per_second,
            "total_ms": ms(m.total),
            "vram_peak_bytes": m.vram_peak_bytes,
            "stop": stop_label(m.stop),
            "context_ids": m.context_ids.iter().map(theos_ai::RecordId::as_str).collect::<Vec<_>>(),
            "cited_ids": m.cited_ids.iter().map(theos_ai::RecordId::as_str).collect::<Vec<_>>(),
        })
    });
    json!({
        "model": r.model,
        "thinking": thinking_label(r.thinking),
        "scope": scope_label(r.scope),
        "scenario": r.scenario_id,
        "npc": r.npc,
        "behaviour": behaviour_label(r.behaviour),
        "hand_read": r.hand_read,
        "technique": r.technique,
        "severity": r.severity.map(|s| s.to_string()),
        "player_name": r.player_name,
        "good_outcome": r.good_outcome,
        "question": r.question,
        "checks": checks,
        "metrics": metrics,
        "reply_text": r.reply_text,
        "reply_error": r.reply_error,
        "raw": r.raw,
    })
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
            checks: Some(Checks {
                retrieval: matches!(scope, LoreScope::Retrieved { .. }).then_some(true),
                leak: true,
                forbidden: true,
                words: Some(vec![]),
                grounding,
                deadline: true,
                vram: Some(true),
                injection: None,
            }),
            metrics: Some(Metrics {
                prompt_tokens: 100,
                thinking_tokens: 0,
                answer_tokens: 20,
                retrieval: Duration::from_millis(5),
                time_to_first_token: Some(Duration::from_millis(total_ms / 2)),
                time_to_first_answer_token: Some(Duration::from_millis(total_ms / 2 + 5)),
                tokens_per_second: 40.0,
                total: Duration::from_millis(total_ms),
                vram_peak_bytes: Some(4 * 1024 * 1024 * 1024),
                context_ids: vec![RecordId::new("B-1")],
                cited_ids: vec![RecordId::new("B-1")],
                stop: StopReason::Finished,
            }),
            reply_text: "Well, it is so,\nbelieve me.".into(),
            reply_error: None,
            raw: "{\"raw\":1}".into(),
            hand_read: false,
            technique: None,
            severity: None,
            player_name: None,
            good_outcome: None,
        }
    }

    fn failed_row(scope: LoreScope) -> Row {
        let mut r = row(Thinking::Off, scope, true, 100);
        r.checks = None;
        r.metrics = None;
        r.reply_text.clear();
        r.raw.clear();
        r.reply_error = Some("backend failed: boom".into());
        r
    }

    #[test]
    fn tok_s_median_ignores_rows_with_fewer_than_two_generated_tokens() {
        let mut slow = row(Thinking::Off, LoreScope::None, true, 100);
        slow.metrics.as_mut().unwrap().tokens_per_second = 10.0;
        let mut unmeasured = row(Thinking::Off, LoreScope::None, true, 100);
        {
            let m = unmeasured.metrics.as_mut().unwrap();
            m.tokens_per_second = 0.0;
            m.thinking_tokens = 0;
            m.answer_tokens = 1;
        }
        let fast = row(Thinking::Off, LoreScope::None, true, 100);
        let s = summarize(&[slow, unmeasured, fast]);
        // 10 and 40 only; the unmeasured 0.0 would drag the median down to 10.
        assert_eq!(s[0].tps_median, Some(25.0));
        let md = render_results(&header(), &[row(Thinking::Off, LoreScope::None, true, 1)]);
        assert!(md.contains("fewer than 2 generated tokens"));
    }

    #[test]
    fn failed_rows_show_their_error_in_every_table_and_count_as_failures() {
        let rows = [
            row(Thinking::Off, LoreScope::None, true, 100),
            failed_row(LoreScope::None),
        ];
        let s = summarize(&rows);
        assert_eq!(
            (s[0].n, s[0].grounding, s[0].deadline, s[0].errors),
            (2, t(1, 1), t(1, 1), 1)
        );
        assert_eq!(s[0].leak, t(1, 1), "an unknown context is not applicable");
        assert_eq!(s[0].total_median, Some(100.0));
        assert!(render_results(&header(), &rows).contains("ERROR: backend failed: boom"));
        assert!(render_csv(&rows).unwrap().contains("backend failed: boom"));
        assert!(render_answers(&rows).contains("_No reply: backend failed: boom_"));
    }

    fn header() -> RunHeader {
        RunHeader {
            model: "Fake 4B".into(),
            file_bytes: 2_500_000_000,
            n_params: 4_000_000_000,
            gpu: "Test GPU".into(),
            deadline: Duration::from_secs(3),
            seed: 42,
            persona_bodies: true,
            probe: "DXGI".into(),
            warm_up: None,
        }
    }

    fn t(pass: usize, of: usize) -> Tally {
        Tally { pass, of }
    }

    /// The cell under column `name` in the first data row of a summary table.
    fn cell(table: &str, name: &str) -> String {
        let mut lines = table.lines();
        let head: Vec<&str> = lines.next().unwrap().split('|').map(str::trim).collect();
        let first: Vec<&str> = lines.nth(1).unwrap().split('|').map(str::trim).collect();
        first[head.iter().position(|h| *h == name).unwrap()].to_owned()
    }

    /// A request the model never answered: the deadline cut it off.
    fn no_reply_row(scope: LoreScope) -> Row {
        let mut r = row(Thinking::On, scope, false, 3000);
        {
            let c = r.checks.as_mut().unwrap();
            c.words = None;
            c.deadline = false;
        }
        r.metrics.as_mut().unwrap().stop = StopReason::Deadline;
        r.reply_text.clear();
        r.reply_error = Some("generation did not finish: Deadline".into());
        r
    }

    #[test]
    fn a_deadline_cut_row_is_not_a_words_pass() {
        let mut rows = vec![row(Thinking::On, LoreScope::None, true, 100)];
        rows.push(no_reply_row(LoreScope::None));
        let s = summarize(&rows);
        assert_eq!(s[0].words, t(1, 1), "the cut row has no words to check");
        assert_eq!(s[0].grounding, t(1, 2), "but it does fail grounding");
        assert_eq!(s[0].deadline, t(1, 2));
        let table = summary_table(&rows);
        assert_eq!(cell(&table, "words"), "1/1");
        assert_eq!(cell(&table, "grounding"), "1/2");
        assert!(render_results(&header(), &rows).contains("n/a (no reply)"));
    }

    #[test]
    fn a_driver_failed_row_does_not_lower_leak_or_forbidden() {
        let rows = [
            row(Thinking::Off, LoreScope::None, true, 100),
            failed_row(LoreScope::None),
        ];
        let s = summarize(&rows);
        assert_eq!((s[0].leak, s[0].forbidden), (t(1, 1), t(1, 1)));
        assert_eq!(s[0].errors, 1);
        let table = summary_table(&rows);
        assert_eq!(cell(&table, "leak"), "1/1");
        assert_eq!(cell(&table, "errors"), "1");
        assert_eq!(cell(&table, "n"), "2");
    }

    #[test]
    fn ttfa_median_and_p95_leave_out_rows_without_an_answer_token() {
        let mut a = row(Thinking::On, LoreScope::None, true, 100);
        a.metrics.as_mut().unwrap().time_to_first_answer_token = Some(Duration::from_millis(100));
        let mut b = row(Thinking::On, LoreScope::None, true, 100);
        b.metrics.as_mut().unwrap().time_to_first_answer_token = Some(Duration::from_millis(300));
        let mut c = row(Thinking::On, LoreScope::None, true, 100);
        c.metrics.as_mut().unwrap().time_to_first_answer_token = None;
        let rows = [a, b, c];
        let s = summarize(&rows);
        assert_eq!(s[0].ttfa_median, Some(200.0));
        assert_eq!(s[0].ttfa_p95, Some(300.0));
        assert_eq!(cell(&summary_table(&rows), "ttfa med/p95 ms"), "200/300");
        let md = render_results(&header(), &rows);
        assert!(md.contains("| ttft ms | ttfa ms |"));
        assert!(render_csv(&rows).unwrap().contains(",ttft_ms,ttfa_ms,"));
    }

    #[test]
    fn results_header_states_the_grounding_rules() {
        let md = render_results(&header(), &[row(Thinking::Off, LoreScope::None, true, 1)]);
        assert!(md.contains("An empty cite list fails `answer` and `rumour`"));
        assert!(md.contains("`unaware` and `deflect` pass with no cites or only `also_ok` cites"));
        assert!(md.contains("scope `none` the schema forbids sources"));
        assert!(
            md.contains("fewer than 2 generated tokens"),
            "tok/s note stays"
        );
    }

    #[test]
    fn results_header_shows_the_warm_up_request() {
        let mut h = header();
        let rows = [row(Thinking::Off, LoreScope::None, true, 1)];
        assert!(!render_results(&h, &rows).contains("warm-up"));
        h.warm_up = Some(WarmUp {
            total: Duration::from_millis(312),
            stop: "finished".into(),
        });
        let md = render_results(&h, &rows);
        assert!(md.contains("- warm-up: 312 ms (not in the table), stop: finished"));
    }

    #[test]
    fn row_json_carries_scenario_checks_and_metrics() {
        let v = row_json(&row(
            Thinking::On,
            LoreScope::Retrieved { top_k: 5 },
            true,
            100,
        ));
        assert_eq!(v["scenario"], "s01");
        assert_eq!(v["thinking"], "on");
        assert_eq!(v["scope"], "retrieved(top 5)");
        assert_eq!(v["checks"]["grounding"], true);
        assert_eq!(v["checks"]["words"], serde_json::json!([]));
        assert_eq!(v["metrics"]["ttfa_ms"], 55.0);
        assert_eq!(v["reply_text"], "Well, it is so,\nbelieve me.");
        let failed = row_json(&failed_row(LoreScope::None));
        assert!(failed["checks"].is_null() && failed["metrics"].is_null());
        assert_eq!(failed["reply_error"], "backend failed: boom");
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
        assert_eq!(s[0].grounding, t(2, 3));
        assert_eq!(s[0].retrieval, None);
        assert_eq!(s[0].total_median, Some(200.0));
        assert_eq!(s[0].total_p95, Some(300.0));
        assert_eq!(s[0].vram, t(3, 3));
        assert_eq!(s[1].retrieval, Some(t(1, 1)));
        assert_eq!(s[1].grounding, t(1, 1));
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
            warm_up: None,
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
