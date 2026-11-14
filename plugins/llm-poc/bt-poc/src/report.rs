//! The results tables: `results.md` and `results.csv`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use theos_ai::Metrics;

use crate::run::Row;
use crate::score::PyScore;

/// What the run header of `results.md` says about the setup.
#[derive(Clone, Debug)]
pub struct RunHeader {
    /// Model display name.
    pub model: String,
    /// Parameter count of the model.
    pub n_params: u64,
    /// GPU name.
    pub gpu: String,
    /// How the deadline was set.
    pub deadline_policy: String,
    /// Sampler seed.
    pub seed: u32,
    /// Sampling settings, in words.
    pub sampling: String,
    /// Context size in tokens.
    pub n_ctx: u32,
    /// Where VRAM readings come from.
    pub probe: String,
    /// Thinking budgets, in words.
    pub thinking_budgets: String,
    /// Whether persona bodies were in the prompt.
    pub persona_bodies: bool,
}

/// One line of the summary: a grammar x thinking x scope cell.
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryRow {
    /// Grammar on or off.
    pub grammar: bool,
    /// Thinking on or off.
    pub thinking: bool,
    /// Lore scope name.
    pub scope: String,
    /// Requests in the cell.
    pub n: usize,
    /// The cells after `n`, in column order.
    pub cells: Vec<String>,
}

const SUMMARY_COLUMNS: [&str; 20] = [
    "grammar",
    "thinking",
    "scope",
    "n",
    "gate pass",
    "top gate rules",
    "behaviour pass",
    "checks passed %",
    "grounding (NPC)",
    "leak-free (NPC)",
    "budget pass",
    "total med/p95 ms",
    "ttft med ms",
    "nodes med",
    "depth med",
    "prompt tok med",
    "output tok med",
    "vram peak",
    "cut off",
    "errors",
];

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// The generation stopped before the model finished (deadline, token cap, full context).
fn is_cut_off(r: &Row) -> bool {
    r.metrics
        .as_ref()
        .is_some_and(|m| m.stop != theos_ai::StopReason::Finished)
}

fn opt_yes(b: Option<bool>) -> &'static str {
    b.map_or("n/a", yes)
}

fn yes(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn scope_rank(scope: &str) -> u8 {
    match scope {
        "none" => 0,
        "all-visible" => 1,
        _ => 2,
    }
}

/// The median (lower middle) and the nearest-rank 95th percentile of `v`.
fn med_p95(mut v: Vec<f64>) -> Option<(f64, f64)> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    let rank = (n as f64 * 0.95).ceil() as usize;
    let med = if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    };
    Some((med, v[rank.clamp(1, n) - 1]))
}

fn med(v: Vec<f64>) -> Option<f64> {
    med_p95(v).map(|(m, _)| m)
}

fn num(v: Option<f64>) -> String {
    v.map_or_else(|| "-".to_owned(), |x| format!("{x:.0}"))
}

fn frac(k: usize, n: usize) -> String {
    format!("{k}/{n}")
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// The summary, one entry per grammar x thinking x scope present in `rows`.
pub fn summary_rows(rows: &[Row]) -> Vec<SummaryRow> {
    let mut cells: BTreeMap<(bool, bool, u8), Vec<&Row>> = BTreeMap::new();
    for r in rows {
        // Grammar on sorts before off.
        cells
            .entry((!r.grammar, r.thinking, scope_rank(&r.scope)))
            .or_default()
            .push(r);
    }
    cells.into_values().map(|rs| summarise(&rs)).collect()
}

fn summarise(rs: &[&Row]) -> SummaryRow {
    let n = rs.len();
    let gate = rs
        .iter()
        .filter(|r| r.py.as_ref().is_some_and(|p| p.gate_pass))
        .count();
    let mut rules: BTreeMap<String, usize> = BTreeMap::new();
    for r in rs {
        let Some(p) = r.py.as_ref().filter(|p| !p.gate_pass) else {
            continue;
        };
        let rule = p.gate_rule.as_deref().unwrap_or("?");
        let label = if is_cut_off(r) && rule == "shape" {
            "shape (cut off)".to_owned()
        } else {
            rule.to_owned()
        };
        *rules.entry(label).or_default() += 1;
    }
    let cut_off = rs.iter().filter(|r| is_cut_off(r)).count();
    let mut rules: Vec<_> = rules.into_iter().collect();
    rules.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let top = if rules.is_empty() {
        "-".to_owned()
    } else {
        rules
            .iter()
            .take(3)
            .map(|(r, c)| format!("{r} x{c}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let behaviour = rs
        .iter()
        .filter(|r| r.py.as_ref().is_some_and(|p| p.behaviour == Some(true)))
        .count();
    let (passed, total) =
        rs.iter()
            .filter_map(|r| r.py.as_ref())
            .fold((0, 0), |(p, t), py: &PyScore| {
                (
                    p + py.checks.iter().filter(|c| **c).count(),
                    t + py.checks.len(),
                )
            });
    let checks = if total == 0 {
        "-".to_owned()
    } else {
        format!("{:.0}%", 100.0 * passed as f64 / total as f64)
    };
    let npc: Vec<_> = rs.iter().filter(|r| r.npc).collect();
    let grounding = if npc.is_empty() {
        "-".to_owned()
    } else {
        let grounded = npc
            .iter()
            .filter(|r| r.py.as_ref().is_some_and(|p| p.grounding == Some(true)))
            .count();
        frac(grounded, npc.len())
    };
    // Rows without a prompt (leak_ok None) are left out of the denominator.
    let leak_known: Vec<bool> = npc.iter().filter_map(|r| r.leak_ok).collect();
    let leak = if leak_known.is_empty() {
        "-".to_owned()
    } else {
        frac(
            leak_known.iter().filter(|ok| **ok).count(),
            leak_known.len(),
        )
    };
    let budget = rs.iter().filter(|r| r.budget_ok).count();
    let m = |f: fn(&Metrics) -> f64| -> Vec<f64> {
        rs.iter()
            .filter_map(|r| r.metrics.as_ref())
            .map(f)
            .collect()
    };
    let py = |f: fn(&PyScore) -> Option<u32>| -> Vec<f64> {
        rs.iter()
            .filter_map(|r| r.py.as_ref())
            .filter_map(f)
            .map(f64::from)
            .collect()
    };
    let total_ms = med_p95(m(|x| ms(x.total))).map_or_else(
        || "-".to_owned(),
        |(a, b)| format!("{}/{}", num(Some(a)), num(Some(b))),
    );
    let ttft = med(rs
        .iter()
        .filter_map(|r| r.metrics.as_ref()?.time_to_first_token)
        .map(ms)
        .collect());
    let vram = rs
        .iter()
        .filter_map(|r| r.metrics.as_ref()?.vram_peak_bytes)
        .max()
        .map_or_else(|| "-".to_owned(), |b| format!("{} MiB", b / (1024 * 1024)));
    let errors = rs.iter().filter(|r| r.error.is_some()).count();
    let cells = vec![
        frac(gate, n),
        top,
        frac(behaviour, n),
        checks,
        grounding,
        leak,
        frac(budget, n),
        total_ms,
        num(ttft),
        num(med(py(|p| p.nodes))),
        num(med(py(|p| p.depth))),
        num(med(m(|x| f64::from(x.prompt_tokens)))),
        num(med(m(|x| f64::from(x.answer_tokens)))),
        vram,
        cut_off.to_string(),
        errors.to_string(),
    ];
    SummaryRow {
        grammar: rs[0].grammar,
        thinking: rs[0].thinking,
        scope: rs[0].scope.clone(),
        n,
        cells,
    }
}

/// The summary as a Markdown table.
pub fn summary_table(rows: &[Row]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "| {} |", SUMMARY_COLUMNS.join(" | "));
    let _ = writeln!(out, "|{}|", vec!["---"; SUMMARY_COLUMNS.len()].join("|"));
    for s in summary_rows(rows) {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            on_off(s.grammar),
            on_off(s.thinking),
            s.scope,
            s.n,
            s.cells.join(" | ")
        );
    }
    out
}

/// The per-request table as Markdown.
pub fn request_table(rows: &[Row]) -> String {
    let mut out = String::from(
        "| scenario | actor | prio | grammar | thinking | scope | lore | gate | behaviour | checks | grounding | sources | leak ok | name overlap | budget | total ms | out tok | error |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let (gate, behaviour, checks, grounding) = match &r.py {
            Some(p) => (
                if p.gate_pass {
                    "pass".to_owned()
                } else {
                    p.gate_rule.clone().unwrap_or_else(|| "fail".to_owned())
                },
                p.behaviour.map_or("-", yes).to_owned(),
                format!(
                    "{}/{}",
                    p.checks.iter().filter(|c| **c).count(),
                    p.checks.len()
                ),
                p.grounding.map_or("-", yes).to_owned(),
            ),
            None => ("-".into(), "-".into(), "-".into(), "-".into()),
        };
        let (total, tokens) = r.metrics.as_ref().map_or(("-".into(), "-".into()), |m| {
            (format!("{:.0}", ms(m.total)), m.answer_tokens.to_string())
        });
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {gate} | {behaviour} | {checks} | {grounding} | {} | {} | {} | {} | {total} | {tokens} | {} |",
            r.scenario_id,
            r.actor,
            r.priority,
            on_off(r.grammar),
            on_off(r.thinking),
            r.scope,
            r.lore_ids.len(),
            opt_yes(r.sources_ok),
            opt_yes(r.leak_ok),
            yes(r.name_overlap),
            yes(r.budget_ok),
            r.error.as_deref().unwrap_or("").replace('|', "/"),
        );
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

const CSV_HEADER: &str = "scenario_id,base_id,actor,priority,grammar,thinking,scope,npc,lore_ids,envelope_ok,gate_pass,gate_rule,gate_message,nodes,depth,checks_passed,checks_total,behaviour,grounding,sources_ok,leak_ok,name_overlap,budget_seconds,budget_ok,prompt_tokens,thinking_tokens,answer_tokens,ttft_ms,ttfa_ms,tokens_per_second,total_ms,vram_peak_bytes,stop,error,technique,severity,injected_in";

fn opt<T: ToString>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

/// Every field of every row, one CSV line each.
pub fn csv(rows: &[Row]) -> String {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for r in rows {
        let p = r.py.as_ref();
        let m = r.metrics.as_ref();
        let msf = |d: Duration| format!("{:.1}", ms(d));
        let fields: Vec<String> = vec![
            r.scenario_id.clone(),
            opt(r.base_id.clone()),
            r.actor.clone(),
            r.priority.clone(),
            r.grammar.to_string(),
            r.thinking.to_string(),
            r.scope.clone(),
            r.npc.to_string(),
            r.lore_ids.join(";"),
            r.envelope_ok.to_string(),
            opt(p.map(|p| p.gate_pass)),
            opt(p.and_then(|p| p.gate_rule.clone())),
            opt(p.and_then(|p| p.gate_message.clone())),
            opt(p.and_then(|p| p.nodes)),
            opt(p.and_then(|p| p.depth)),
            opt(p.map(|p| p.checks.iter().filter(|c| **c).count())),
            opt(p.map(|p| p.checks.len())),
            opt(p.and_then(|p| p.behaviour)),
            opt(p.and_then(|p| p.grounding)),
            opt(r.sources_ok),
            opt(r.leak_ok),
            r.name_overlap.to_string(),
            r.budget_seconds.to_string(),
            r.budget_ok.to_string(),
            opt(m.map(|m| m.prompt_tokens)),
            opt(m.map(|m| m.thinking_tokens)),
            opt(m.map(|m| m.answer_tokens)),
            m.and_then(|m| m.time_to_first_token)
                .map(msf)
                .unwrap_or_default(),
            m.and_then(|m| m.time_to_first_answer_token)
                .map(msf)
                .unwrap_or_default(),
            opt(m.map(|m| m.tokens_per_second)),
            m.map(|m| msf(m.total)).unwrap_or_default(),
            opt(m.and_then(|m| m.vram_peak_bytes)),
            m.map(|m| format!("{:?}", m.stop)).unwrap_or_default(),
            opt(r.error.clone()),
            opt(r.technique.clone()),
            opt(r.severity.clone()),
            opt(r.injected_in.clone()),
        ];
        let line: Vec<String> = fields.iter().map(|f| csv_field(f)).collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

/// The header block of `results.md`.
pub fn header_block(h: &RunHeader) -> String {
    format!(
        "# Behaviour-tree PoC results\n\n\
         - Model: {}\n- Parameters: {:.2} B\n- GPU: {}\n- Deadline: {}\n- Seed: {}\n\
         - Sampling: {}\n- Thinking budgets: {}\n- n_ctx: {}\n- VRAM probe: {}\n- Persona bodies: {}\n\n\
         Output tok is the tree (answer-phase) tokens; thinking tokens are in results.csv. \
         Counting: rates count every request of the cell; an errored or cut-off request is a failure.\n\
         `cut off` counts requests whose generation stopped before the model finished (deadline, token cap or full context); \
         they are also in `errors`, and their gate `shape` failure is shown as `shape (cut off)` in `top gate rules`.\n\
         `leak-free (NPC)` is the number of NPC requests whose prompt held only records visible to the actor, out of the NPC requests that had a prompt (higher is better); \
         requests that failed before a prompt existed are n/a and left out. `sources ok` is n/a when no JSON object was parsed.\n\
         Medians are true medians (mean of the two middle values for an even count).\n\n",
        h.model,
        h.n_params as f64 / 1e9,
        h.gpu,
        h.deadline_policy,
        h.seed,
        h.sampling,
        h.thinking_budgets,
        h.n_ctx,
        h.probe,
        yes(h.persona_bodies),
    )
}

/// Writes `results.md` and `results.csv` into `dir`.
///
/// # Errors
///
/// A file that cannot be written.
pub fn write(dir: &Path, header: &RunHeader, rows: &[Row]) -> anyhow::Result<()> {
    let md = format!(
        "{}## Summary\n\n{}\n## Requests\n\n{}",
        header_block(header),
        summary_table(rows),
        request_table(rows)
    );
    std::fs::write(dir.join("results.md"), md).context("writing results.md")?;
    std::fs::write(dir.join("results.csv"), csv(rows)).context("writing results.csv")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::Row;

    fn row(grammar: bool, thinking: bool, scope: &str, gate_pass: bool) -> Row {
        Row {
            scenario_id: "b01".into(),
            base_id: None,
            actor: "a".into(),
            priority: "WorldEvent".into(),
            grammar,
            thinking,
            scope: scope.into(),
            lore_ids: vec![],
            npc: true,
            envelope_ok: true,
            py: Some(PyScore {
                gate_pass,
                gate_rule: (!gate_pass).then(|| "shape".to_owned()),
                gate_message: None,
                nodes: Some(5),
                depth: Some(2),
                checks: vec![true, false],
                behaviour: Some(false),
                grounding: Some(true),
            }),
            sources_ok: Some(true),
            leak_ok: Some(true),
            name_overlap: false,
            budget_seconds: 30.0,
            budget_ok: true,
            metrics: None,
            error: None,
            technique: None,
            severity: None,
            injected_in: None,
        }
    }

    #[test]
    fn summary_has_one_row_per_grammar_thinking_and_scope() {
        let mut rows = Vec::new();
        for grammar in [true, false] {
            for thinking in [false, true] {
                for scope in ["none", "all-visible", "retrieved"] {
                    rows.push(row(grammar, thinking, scope, true));
                    rows.push(row(grammar, thinking, scope, false));
                }
            }
        }
        let s = summary_rows(&rows);
        assert_eq!(s.len(), 12);
        assert!(s.iter().all(|r| r.n == 2));
        assert_eq!(
            (s[0].grammar, s[0].thinking, s[0].scope.as_str()),
            (true, false, "none")
        );
        assert_eq!(s[0].cells[0], "1/2", "gate pass");
        assert_eq!(s[0].cells[1], "shape x1", "top gate rules");
        assert_eq!(s[0].cells[3], "50%", "checks passed");
        let table = summary_table(&rows);
        assert_eq!(table.lines().count(), 2 + 12);
        assert!(table.lines().next().unwrap().contains("total med/p95 ms"));
    }

    #[test]
    fn csv_quotes_fields_with_commas_and_has_one_line_per_row() {
        let mut r = row(true, false, "none", false);
        r.error = Some("a, \"b\"".into());
        let text = csv(&[r]);
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("\"a, \"\"b\"\"\""));
    }

    #[test]
    fn percentile_is_nearest_rank() {
        let v: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(med_p95(v), Some((10.5, 19.0)), "even n: mean of middles");
        assert_eq!(med_p95(vec![1.0, 5.0, 9.0]), Some((5.0, 9.0)));
        assert_eq!(med_p95(vec![]), None);
    }

    fn metrics(stop: theos_ai::StopReason) -> Metrics {
        Metrics {
            prompt_tokens: 1,
            thinking_tokens: 0,
            answer_tokens: 1,
            retrieval: Duration::ZERO,
            time_to_first_token: None,
            time_to_first_answer_token: None,
            tokens_per_second: 0.0,
            total: Duration::from_millis(10),
            vram_peak_bytes: None,
            context_ids: vec![],
            cited_ids: vec![],
            stop,
        }
    }

    fn shape_row(stop: theos_ai::StopReason) -> Row {
        let mut r = row(true, false, "none", false);
        r.metrics = Some(metrics(stop));
        r
    }

    #[test]
    fn cut_off_rows_are_counted_apart_and_labelled_in_top_gate_rules() {
        let rows = vec![
            shape_row(theos_ai::StopReason::TokenCap),
            shape_row(theos_ai::StopReason::Finished),
        ];
        let s = &summary_rows(&rows)[0];
        assert_eq!(s.cells[1], "shape x1, shape (cut off) x1");
        let table = summary_table(&rows);
        let head = table.lines().next().unwrap();
        assert!(head.contains("| cut off |"));
        let cols: Vec<&str> = head.split('|').map(str::trim).collect();
        let at = cols.iter().position(|c| *c == "cut off").unwrap();
        let line: Vec<&str> = table
            .lines()
            .nth(2)
            .unwrap()
            .split('|')
            .map(str::trim)
            .collect();
        assert_eq!(line[at], "1");
    }

    #[test]
    fn leak_free_excludes_rows_without_a_prompt() {
        let mut none = row(true, false, "none", true);
        none.leak_ok = None;
        let mut leaky = row(true, false, "none", true);
        leaky.leak_ok = Some(false);
        let ok = row(true, false, "none", true);
        let rows = vec![none, leaky, ok];
        assert!(summary_table(&rows).contains("leak-free (NPC)"));
        assert!(!summary_table(&rows).contains("| leak (NPC)"));
        let s = &summary_rows(&rows)[0];
        assert_eq!(s.cells[5], "1/2");
        assert!(request_table(&rows).contains("| n/a |"));
        assert!(header_block(&test_header()).contains("leak-free"));
        assert!(header_block(&test_header()).contains("cut off"));
    }

    fn test_header() -> RunHeader {
        RunHeader {
            model: "m".into(),
            n_params: 1,
            gpu: "g".into(),
            deadline_policy: "d".into(),
            seed: 1,
            sampling: "s".into(),
            n_ctx: 1,
            probe: "p".into(),
            thinking_budgets: "t".into(),
            persona_bodies: true,
        }
    }
}
