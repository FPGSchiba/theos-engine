//! The review page the owner reads: every block, and the closest allowed texts.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use theos_ai::{KnnThresholds, ModerationSource, ModerationVerdict, Nearest, SourceThreshold};

use crate::record::CheckRecord;

/// How many allowed records the "Closest allowed" section lists.
const CLOSEST_ALLOWED: usize = 40;

fn md_cell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace("\r\n", "<br>")
        .replace(['\r', '\n'], "<br>")
}

fn source_label(s: ModerationSource) -> &'static str {
    match s {
        ModerationSource::PlayerMessage => "player",
        ModerationSource::ModelReply => "reply",
        _ => "other",
    }
}

fn threshold(t: &KnnThresholds, s: ModerationSource) -> SourceThreshold {
    match s {
        ModerationSource::ModelReply => t.reply,
        _ => t.player,
    }
}

/// The nearest-rank percentile (`p` in 0..=1) of an ascending slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    // Rank is in 1..=len, so the index is in range.
    let rank = (p * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}

/// One "moderator alone" timing line per source that has records.
fn timing_lines(records: &[CheckRecord]) -> Vec<String> {
    let mut lines = Vec::new();
    for source in [
        ModerationSource::PlayerMessage,
        ModerationSource::ModelReply,
    ] {
        let mut ms: Vec<f64> = records
            .iter()
            .filter(|r| r.source == source)
            .map(|r| r.elapsed.as_secs_f64() * 1000.0)
            .collect();
        if ms.is_empty() {
            continue;
        }
        ms.sort_by(f64::total_cmp);
        lines.push(format!(
            "- {} checks: {}, median {:.1} ms, p95 {:.1} ms (moderator alone)",
            source_label(source),
            ms.len(),
            percentile(&ms, 0.5),
            percentile(&ms, 0.95)
        ));
    }
    lines
}

fn nearest_cell(n: &Option<Nearest>) -> String {
    match n {
        Some(n) => format!("{} ({:.3})", md_cell(&n.example), n.similarity),
        None => "-".to_owned(),
    }
}

/// Writes the review page to `path`.
///
/// # Errors
/// Fails when the file cannot be written.
pub fn write_review(
    path: &Path,
    records: &[CheckRecord],
    thresholds: &KnnThresholds,
) -> anyhow::Result<()> {
    std::fs::write(path, render(records, thresholds))
        .with_context(|| format!("writing {}", path.display()))
}

fn render(records: &[CheckRecord], t: &KnnThresholds) -> String {
    let blocks: Vec<&CheckRecord> = records
        .iter()
        .filter(|r| matches!(r.evidence.verdict, ModerationVerdict::Block(_)))
        .collect();
    // Allowed texts with something to measure, nearest miss first; the rest
    // (no banned example at all) are listed at the end, never dropped.
    let (mut allowed, unmeasured): (Vec<&CheckRecord>, Vec<&CheckRecord>) = records
        .iter()
        .filter(|r| r.evidence.verdict == ModerationVerdict::Allow)
        .partition(|r| r.evidence.nearest_banned.is_some());
    allowed.sort_by(|a, b| b.evidence.distance.total_cmp(&a.evidence.distance));

    let mut out = String::new();
    let _ = writeln!(out, "# Moderation review\n");
    let _ = writeln!(
        out,
        "- player: min_similarity {:.3}, margin {:.3}",
        t.player.min_similarity, t.player.margin
    );
    let _ = writeln!(
        out,
        "- reply: min_similarity {:.3}, margin {:.3}",
        t.reply.min_similarity, t.reply.margin
    );
    let _ = writeln!(
        out,
        "- checks: {} total, {} blocked, {} allowed\n",
        records.len(),
        blocks.len(),
        records.len() - blocks.len()
    );
    for line in timing_lines(records) {
        let _ = writeln!(out, "{line}");
    }
    out.push('\n');

    let _ = writeln!(out, "## Blocked\n");
    if blocks.is_empty() {
        let _ = writeln!(out, "None.\n");
    } else {
        let _ = writeln!(
            out,
            "| Tag | Source | Category | Deciding piece | Text | Nearest banned | Nearest allowed |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|");
        for r in blocks {
            let category = match &r.evidence.verdict {
                ModerationVerdict::Block(c) => c.id().to_owned(),
                _ => String::new(),
            };
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} | {} | {} |",
                md_cell(&r.tag),
                source_label(r.source),
                md_cell(&category),
                md_cell(&r.evidence.piece),
                md_cell(&r.text),
                nearest_cell(&r.evidence.nearest_banned),
                nearest_cell(&r.evidence.nearest_allowed),
            );
        }
        out.push('\n');
    }

    let _ = writeln!(out, "## Closest allowed\n");
    let _ = writeln!(
        out,
        "Allowed checks, nearest miss first, top {CLOSEST_ALLOWED}. `distance` = min(`best_c - min_similarity`, `best_c - best_a - margin`); a text blocks when it reaches 0.\n"
    );
    if allowed.is_empty() {
        let _ = writeln!(out, "None.");
    } else {
        let _ = writeln!(
            out,
            "| Tag | Source | distance | best_c - min_similarity | best_c - best_a - margin | Deciding piece | Text | Nearest banned | Nearest allowed |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|");
        for r in allowed.into_iter().take(CLOSEST_ALLOWED) {
            let th = threshold(t, r.source);
            let best_c = r.evidence.nearest_banned.as_ref().map(|n| n.similarity);
            let best_a = r.evidence.nearest_allowed.as_ref().map(|n| n.similarity);
            let sim_term = best_c.map_or_else(
                || "-".to_owned(),
                |c| format!("{:+.3}", c - th.min_similarity),
            );
            let margin_term = match (best_c, best_a) {
                (Some(c), Some(a)) => format!("{:+.3}", c - a - th.margin),
                _ => "-".to_owned(),
            };
            let _ = writeln!(
                out,
                "| {} | {} | {:+.3} | {} | {} | {} | {} | {} | {} |",
                md_cell(&r.tag),
                source_label(r.source),
                r.evidence.distance,
                sim_term,
                margin_term,
                md_cell(&r.evidence.piece),
                md_cell(&r.text),
                nearest_cell(&r.evidence.nearest_banned),
                nearest_cell(&r.evidence.nearest_allowed),
            );
        }
    }

    if !unmeasured.is_empty() {
        let _ = writeln!(out, "\n## Allowed without a nearest banned example\n");
        let tags: Vec<String> = unmeasured.iter().map(|r| md_cell(&r.tag)).collect();
        let _ = writeln!(out, "{} checks: {}", unmeasured.len(), tags.join(", "));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use theos_ai::{KnnEvidence, ModerationCategory, SourceThreshold};

    fn thresholds() -> KnnThresholds {
        let t = SourceThreshold {
            min_similarity: 0.5,
            margin: 0.1,
        };
        KnnThresholds {
            player: t,
            reply: t,
        }
    }

    fn near(example: &str, similarity: f32) -> Option<Nearest> {
        Some(Nearest {
            category: None,
            example: example.to_owned(),
            similarity,
        })
    }

    fn record(text: &str, verdict: ModerationVerdict, banned_sim: f32) -> CheckRecord {
        CheckRecord {
            tag: "scn/scope".to_owned(),
            source: ModerationSource::PlayerMessage,
            text: text.to_owned(),
            evidence: KnnEvidence {
                verdict,
                piece: text.to_owned(),
                distance: (banned_sim - 0.5).min(banned_sim - 0.2 - 0.1),
                nearest_banned: near("bad example", banned_sim),
                nearest_allowed: near("fine example", 0.2),
            },
            elapsed: Duration::from_millis(1),
        }
    }

    #[test]
    fn review_lists_blocks_first_then_closest_allowed() {
        let records = vec![
            record("far allowed", ModerationVerdict::Allow, 0.1),
            record("near allowed", ModerationVerdict::Allow, 0.45),
            record(
                "blocked text",
                ModerationVerdict::Block(ModerationCategory::Hate),
                0.9,
            ),
        ];
        let page = render(&records, &thresholds());
        let blocked = page.find("## Blocked").unwrap();
        let closest = page.find("## Closest allowed").unwrap();
        assert!(blocked < closest);
        assert!(page.contains("min_similarity 0.500"));
        assert!(page.contains("1 blocked, 2 allowed"));
        let b = page.find("blocked text").unwrap();
        assert!(blocked < b && b < closest);
        assert!(page.contains("| hate |"));
        assert!(page.contains("bad example (0.900)"));
        let near_pos = page.find("near allowed").unwrap();
        let far_pos = page.find("far allowed").unwrap();
        assert!(closest < near_pos && near_pos < far_pos);
    }

    #[test]
    fn review_escapes_pipes_and_newlines() {
        let records = vec![record("a | b\nc \\ d", ModerationVerdict::Allow, 0.3)];
        let page = render(&records, &thresholds());
        assert!(page.contains("a \\| b<br>c \\\\ d"));
        assert!(!page.contains("a | b"));
    }

    #[test]
    fn write_review_writes_the_file() {
        let path = std::env::temp_dir().join(format!("guard-kit-review-{}.md", std::process::id()));
        write_review(&path, &[], &thresholds()).unwrap();
        let s = std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("0 total"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn closest_allowed_is_ordered_by_distance_not_by_banned_similarity() {
        // "high sim" has the larger best_c but a small margin to its allowed
        // example, so its distance is lower than "low sim"'s.
        let mut high = record("high sim", ModerationVerdict::Allow, 0.45);
        high.evidence.distance = -0.30;
        let mut low = record("low sim", ModerationVerdict::Allow, 0.40);
        low.evidence.distance = -0.10;
        let page = render(&[high, low], &thresholds());
        let high_pos = page.find("high sim").unwrap();
        let low_pos = page.find("low sim").unwrap();
        assert!(low_pos < high_pos, "{page}");
        assert!(page.contains("| distance | best_c - min_similarity | best_c - best_a - margin |"));
        assert!(page.contains("-0.300"));
        // best_c - min_similarity = 0.40 - 0.5; best_c - best_a - margin = 0.40 - 0.2 - 0.1
        assert!(page.contains("| -0.100 | -0.100 | +0.100 |"), "{page}");
    }

    #[test]
    fn allowed_records_without_a_nearest_banned_are_listed_at_the_end() {
        let mut none = record("no banned list", ModerationVerdict::Allow, 0.0);
        none.evidence.nearest_banned = None;
        none.evidence.distance = f32::NEG_INFINITY;
        none.tag = "scn/orphan".to_owned();
        let measured = record("measured", ModerationVerdict::Allow, 0.3);
        let page = render(&[none, measured], &thresholds());
        let end = page
            .find("## Allowed without a nearest banned example")
            .unwrap();
        assert!(page.find("measured").unwrap() < end);
        assert!(page[end..].contains("1 checks: scn/orphan"));
        // Not in the table.
        assert!(!page[..end].contains("no banned list"));
    }

    #[test]
    fn header_reports_check_count_median_and_p95_per_source() {
        let mut records = Vec::new();
        for ms in 1..=20_u64 {
            let mut r = record("t", ModerationVerdict::Allow, 0.1);
            r.elapsed = Duration::from_millis(ms);
            records.push(r);
        }
        let mut reply = record("r", ModerationVerdict::Allow, 0.1);
        reply.source = ModerationSource::ModelReply;
        reply.elapsed = Duration::from_millis(9);
        records.push(reply);
        let page = render(&records, &thresholds());
        assert!(
            page.contains("- player checks: 20, median 10.0 ms, p95 19.0 ms (moderator alone)"),
            "{page}"
        );
        assert!(
            page.contains("- reply checks: 1, median 9.0 ms, p95 9.0 ms (moderator alone)"),
            "{page}"
        );
    }
}
