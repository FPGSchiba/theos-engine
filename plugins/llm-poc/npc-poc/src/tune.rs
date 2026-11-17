//! The `moderation-tune` command: measure the nearest-neighbour moderator on the
//! held-out lines and propose thresholds. It writes nothing; the person running it
//! copies the values into `fixtures/moderation/moderation.toml`.
//!
//! Held-out lines are never examples, so there is no leave-one-out here: every
//! held-out line (banned and allowed) is evaluated as a player message against the
//! full example lists. The moderator is built with a threshold nothing can reach, so
//! the evidence is the closest banned example overall, whatever the current
//! `moderation.toml` says.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::anyhow;
use guard_kit::{load_lists, ModerationLists};
use theos_ai::{
    Embedder, KnnModerator, KnnThresholds, ModerationError, ModerationSource, SourceThreshold,
};
use theos_ai_llama::{models_dir, LlamaEmbedder, LlamaRuntime, ModelsFile};

use crate::run::build_knn;

/// The proposed margin: how much closer to a banned example than to any allowed one.
pub const MARGIN: f32 = 0.05;

/// Whether a held-out line belongs to a banned category or to the allowed list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// A held-out line of a banned category.
    Banned,
    /// A held-out line of `allowed.toml`.
    Allowed,
}

/// The evidence for one held-out line.
#[derive(Clone, Debug, PartialEq)]
pub struct TuneLine {
    /// The list the line came from: a category id, or `allowed`.
    pub list: String,
    /// Banned or allowed.
    pub kind: LineKind,
    /// The line.
    pub line: String,
    /// The closest banned category and its similarity.
    pub banned: Option<(String, f32)>,
    /// The similarity to the closest allowed example.
    pub allowed: Option<f32>,
}

/// How many held-out lines of one category a threshold catches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catch {
    /// The category id.
    pub category: String,
    /// Lines that would block.
    pub caught: usize,
    /// Held-out lines of the category.
    pub total: usize,
}

/// The thresholds [`propose`] suggests.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    /// `None` when there is no allowed held-out line to measure against.
    pub min_similarity: Option<f32>,
    /// Always [`MARGIN`].
    pub margin: f32,
    /// Banned held-out lines caught per category, in the order the categories first appear.
    pub caught: Vec<Catch>,
}

/// Rounds up to the next 0.01. A hundredth of a hundredth of slack absorbs the
/// float error of `0.43 + 0.01`, which would otherwise round `0.44` up to `0.45`.
fn round_up_hundredth(x: f32) -> f32 {
    let hundredths = (f64::from(x) * 100.0 - 1e-3).ceil();
    // The value is a small multiple of 0.01; the narrowing is exact enough for a threshold.
    (hundredths / 100.0) as f32
}

/// Proposes `min_similarity` as the highest nearest-banned similarity over the allowed
/// held-out lines, plus 0.01, rounded up to 0.01, so that none of them blocks; the
/// margin is [`MARGIN`]. Reports how many banned held-out lines of each category the
/// pair catches (similarity at least `min_similarity` and at least [`MARGIN`] ahead of
/// the closest allowed example).
pub fn propose(lines: &[TuneLine]) -> Proposal {
    let highest = lines
        .iter()
        .filter(|l| l.kind == LineKind::Allowed)
        .filter_map(|l| l.banned.as_ref().map(|(_, s)| *s))
        .fold(None, |top: Option<f32>, s| {
            Some(top.map_or(s, |t| t.max(s)))
        });
    let min_similarity = highest.map(|h| round_up_hundredth(h + 0.01));
    let mut caught: Vec<Catch> = Vec::new();
    for l in lines.iter().filter(|l| l.kind == LineKind::Banned) {
        let at = match caught.iter().position(|c| c.category == l.list) {
            Some(at) => at,
            None => {
                caught.push(Catch {
                    category: l.list.clone(),
                    caught: 0,
                    total: 0,
                });
                caught.len() - 1
            }
        };
        caught[at].total += 1;
        let blocks = match (min_similarity, &l.banned) {
            (Some(min), Some((_, sim))) => {
                *sim >= min && *sim - l.allowed.unwrap_or(f32::NEG_INFINITY) >= MARGIN
            }
            _ => false,
        };
        if blocks {
            caught[at].caught += 1;
        }
    }
    Proposal {
        min_similarity,
        margin: MARGIN,
        caught,
    }
}

/// Evaluates every held-out line of `lists` as a player message.
///
/// # Errors
///
/// When the embedder fails.
pub fn evaluate_lines(
    moderator: &mut KnnModerator,
    lists: &ModerationLists,
) -> Result<Vec<TuneLine>, ModerationError> {
    let mut out = Vec::new();
    let banned = lists
        .banned
        .iter()
        .flat_map(|c| c.held_out.iter().map(move |l| (c.category.id(), l)))
        .map(|(id, l)| (id, LineKind::Banned, l));
    let allowed = lists
        .allowed_held_out
        .iter()
        .map(|l| ("allowed", LineKind::Allowed, l));
    for (list, kind, line) in banned.chain(allowed) {
        let e = moderator.evaluate(ModerationSource::PlayerMessage, line)?;
        out.push(TuneLine {
            list: list.to_owned(),
            kind,
            line: line.clone(),
            banned: e
                .nearest_banned
                .and_then(|n| n.category.map(|c| (c.id().to_owned(), n.similarity))),
            allowed: e.nearest_allowed.map(|n| n.similarity),
        });
    }
    Ok(out)
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// The table and the proposal as text.
pub fn render(lines: &[TuneLine], proposal: &Proposal) -> String {
    let mut out = String::from(
        "| list | line | best banned | similarity | best allowed similarity |\n|---|---|---|---|---|\n",
    );
    for l in lines {
        let (cat, sim) = l.banned.as_ref().map_or_else(
            || ("n/a".to_owned(), "n/a".to_owned()),
            |(c, s)| (c.clone(), format!("{s:.3}")),
        );
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            l.list,
            cell(&l.line),
            cat,
            sim,
            l.allowed
                .map_or_else(|| "n/a".to_owned(), |s| format!("{s:.3}")),
        );
    }
    out.push('\n');
    match proposal.min_similarity {
        None => out.push_str(
            "No allowed held-out line to measure against: no min_similarity can be proposed.\n",
        ),
        Some(min) => {
            let _ = writeln!(
                out,
                "Proposed (highest nearest-banned similarity over the allowed held-out lines, plus 0.01, rounded up to 0.01):\n\n[player]\nmin_similarity = {min:.2}\nmargin = {:.2}\n\n[reply]\nmin_similarity = {min:.2}\nmargin = {:.2}\n",
                proposal.margin, proposal.margin
            );
        }
    }
    out.push_str("Banned held-out lines this would catch:\n\n");
    for c in &proposal.caught {
        let _ = writeln!(out, "- {}: {}/{}", c.category, c.caught, c.total);
    }
    out
}

/// The `moderation-tune` command.
///
/// # Errors
///
/// A missing or invalid list directory, a missing model file, or an embedder failure.
pub fn tune_cli(moderation_dir: &Path, models: Option<&Path>) -> anyhow::Result<()> {
    let lists = load_lists(moderation_dir)?;
    let models_path = models.map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../models.toml"),
        Path::to_path_buf,
    );
    let file = ModelsFile::load(&models_path).map_err(|e| anyhow!("{e}"))?;
    let path = models_dir().join(&file.embedder);
    if !path.is_file() {
        anyhow::bail!(
            "model file not found: {}\nPut the GGUF files in THEOS_MODELS_DIR (currently {}) or set THEOS_MODELS_DIR.",
            path.display(),
            models_dir().display()
        );
    }
    let rt = LlamaRuntime::init().map_err(|e| anyhow!("{e}"))?;
    let embedder = LlamaEmbedder::load(rt, &path).map_err(|e| anyhow!("{e}"))?;
    let mut moderator = build_knn(
        Box::new(embedder) as Box<dyn Embedder>,
        &lists,
        unreachable_thresholds(),
    )
    .map_err(|e| anyhow!("{e}"))?;
    let lines = evaluate_lines(&mut moderator, &lists).map_err(|e| anyhow!("{e}"))?;
    let proposal = propose(&lines);
    println!("{}", render(&lines, &proposal));
    println!(
        "current thresholds: player {} / {}, reply {} / {}",
        lists.thresholds.player.min_similarity,
        lists.thresholds.player.margin,
        lists.thresholds.reply.min_similarity,
        lists.thresholds.reply.margin
    );
    Ok(())
}

/// Thresholds nothing reaches (cosine similarity is at most 1), so the evidence is
/// always the closest banned example overall.
fn unreachable_thresholds() -> KnnThresholds {
    let t = SourceThreshold {
        min_similarity: 2.0,
        margin: 0.0,
    };
    KnnThresholds {
        player: t,
        reply: t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use guard_kit::CategoryList;
    use theos_ai::testkit::FakeEmbedder;
    use theos_ai::ModerationCategory;

    fn line(list: &str, kind: LineKind, banned: Option<f32>, allowed: Option<f32>) -> TuneLine {
        TuneLine {
            list: list.into(),
            kind,
            line: format!("a {list} line"),
            banned: banned.map(|s| ("hate".to_owned(), s)),
            allowed,
        }
    }

    #[test]
    fn proposal_is_the_highest_allowed_banned_similarity_plus_a_hundredth() {
        let lines = [
            line("allowed", LineKind::Allowed, Some(0.30), Some(0.9)),
            line("allowed", LineKind::Allowed, Some(0.43), Some(0.9)),
            line("allowed", LineKind::Allowed, Some(0.10), Some(0.9)),
        ];
        let p = propose(&lines);
        assert_eq!(p.min_similarity, Some(0.44));
        assert_eq!(p.margin, 0.05);
    }

    #[test]
    fn proposal_rounds_up_to_the_next_hundredth() {
        let lines = [line("allowed", LineKind::Allowed, Some(0.4321), None)];
        assert_eq!(propose(&lines).min_similarity, Some(0.45));
    }

    #[test]
    fn proposal_ignores_banned_lines_when_choosing_the_threshold() {
        let lines = [
            line("allowed", LineKind::Allowed, Some(0.2), None),
            line("hate", LineKind::Banned, Some(0.9), None),
        ];
        assert_eq!(propose(&lines).min_similarity, Some(0.21));
    }

    #[test]
    fn proposal_without_allowed_lines_has_no_threshold_and_catches_nothing() {
        let lines = [line("hate", LineKind::Banned, Some(0.99), None)];
        let p = propose(&lines);
        assert_eq!(p.min_similarity, None);
        assert_eq!(
            p.caught,
            [Catch {
                category: "hate".into(),
                caught: 0,
                total: 1
            }]
        );
    }

    #[test]
    fn caught_counts_banned_lines_per_category_that_clear_threshold_and_margin() {
        let lines = [
            line("allowed", LineKind::Allowed, Some(0.50), Some(0.9)),
            // threshold 0.51
            line("hate", LineKind::Banned, Some(0.80), Some(0.40)), // caught
            line("hate", LineKind::Banned, Some(0.50), Some(0.10)), // below threshold
            line("self-harm", LineKind::Banned, Some(0.70), Some(0.68)), // margin too small
            line("self-harm", LineKind::Banned, Some(0.70), None),  // no allowed: caught
        ];
        let p = propose(&lines);
        assert_eq!(p.min_similarity, Some(0.51));
        assert_eq!(
            p.caught,
            [
                Catch {
                    category: "hate".into(),
                    caught: 1,
                    total: 2
                },
                Catch {
                    category: "self-harm".into(),
                    caught: 1,
                    total: 2
                }
            ]
        );
    }

    fn lists() -> ModerationLists {
        let t = SourceThreshold {
            min_similarity: 0.8,
            margin: 0.05,
        };
        ModerationLists {
            banned: vec![CategoryList {
                category: ModerationCategory::Hate,
                examples: vec!["we hate the northern people".into()],
                held_out: vec!["we hate the northern people so much".into()],
            }],
            allowed: vec!["good morning dear friend".into()],
            allowed_held_out: vec!["good morning friend".into(), "what is the weather".into()],
            thresholds: KnnThresholds {
                player: t,
                reply: t,
            },
        }
    }

    #[test]
    fn evaluate_lines_covers_every_held_out_line_with_the_real_lists_shape() {
        let lists = lists();
        let mut m = build_knn(
            Box::new(FakeEmbedder::default()),
            &lists,
            unreachable_thresholds(),
        )
        .unwrap();
        let lines = evaluate_lines(&mut m, &lists).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].kind, LineKind::Banned);
        assert_eq!(lines[0].list, "hate");
        let (cat, sim) = lines[0].banned.clone().unwrap();
        assert_eq!(cat, "hate");
        assert!(sim > 0.5, "shares most words with its example: {sim}");
        assert!(lines[1..].iter().all(|l| l.kind == LineKind::Allowed));
        assert!(lines[1].allowed.unwrap() > lines[1].banned.as_ref().unwrap().1);
        // The unreachable thresholds never block, so the closest banned example is reported
        // for every line.
        assert!(lines.iter().all(|l| l.banned.is_some()));
    }

    #[test]
    fn render_shows_the_table_the_proposal_and_the_catch_counts() {
        let lines = [
            line("allowed", LineKind::Allowed, Some(0.30), Some(0.9)),
            line("hate", LineKind::Banned, Some(0.80), Some(0.2)),
        ];
        let text = render(&lines, &propose(&lines));
        assert!(text.contains("| allowed | a allowed line | hate | 0.300 | 0.900 |"));
        assert!(text.contains("[player]\nmin_similarity = 0.31\nmargin = 0.05"));
        assert!(text.contains("[reply]\nmin_similarity = 0.31"));
        assert!(text.contains("- hate: 1/1"));
    }

    #[test]
    fn render_without_a_threshold_says_so() {
        let text = render(&[], &propose(&[]));
        assert!(text.contains("no min_similarity can be proposed"));
    }

    #[test]
    fn rounding_survives_float_error() {
        assert_eq!(round_up_hundredth(0.44), 0.44);
        assert_eq!(round_up_hundredth(0.4401), 0.45);
        assert_eq!(round_up_hundredth(0.0), 0.0);
    }
}
