//! The review page the owner reads: every block, and the closest allowed texts.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Context;
use theos_ai::{KnnThresholds, ModerationSource, ModerationVerdict, Nearest};

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
    }
}

fn min_similarity(t: &KnnThresholds, s: ModerationSource) -> f32 {
    match s {
        ModerationSource::PlayerMessage => t.player.min_similarity,
        ModerationSource::ModelReply => t.reply.min_similarity,
    }
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
    let mut allowed: Vec<(&CheckRecord, f32)> = records
        .iter()
        .filter(|r| r.evidence.verdict == ModerationVerdict::Allow)
        .filter_map(|r| {
            r.evidence
                .nearest_banned
                .as_ref()
                .map(|n| (r, n.similarity - min_similarity(t, r.source)))
        })
        .collect();
    allowed.sort_by(|a, b| b.1.total_cmp(&a.1));

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
                ModerationVerdict::Allow => String::new(),
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
        "Allowed checks, by nearest banned similarity minus the source threshold, top {CLOSEST_ALLOWED}.\n"
    );
    if allowed.is_empty() {
        let _ = writeln!(out, "None.");
    } else {
        let _ = writeln!(
            out,
            "| Tag | Source | Gap | Deciding piece | Text | Nearest banned | Nearest allowed |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|");
        for (r, gap) in allowed.into_iter().take(CLOSEST_ALLOWED) {
            let _ = writeln!(
                out,
                "| {} | {} | {:+.3} | {} | {} | {} | {} |",
                md_cell(&r.tag),
                source_label(r.source),
                gap,
                md_cell(&r.evidence.piece),
                md_cell(&r.text),
                nearest_cell(&r.evidence.nearest_banned),
                nearest_cell(&r.evidence.nearest_allowed),
            );
        }
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
}
