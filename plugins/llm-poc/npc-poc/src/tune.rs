//! The `moderation-tune` command: score the held-out lines with the real embedder and
//! grid-search thresholds under the moderator's own decision rule. It writes nothing;
//! the person running it copies the values into `fixtures/moderation/moderation.toml`.
//!
//! Held-out lines are never examples, so there is no leave-one-out: every held-out line
//! (banned and allowed) is scored as a player message against the full example lists.
//!
//! `KnnEvidence` only carries the deciding piece, so this module scores the pieces
//! itself: it splits a line with `theos_ai::moderation_pieces`, the very function
//! `KnnModerator` uses (the whole text plus each sentence of at least three words), embeds every piece as a document, and keeps per piece the best banned
//! similarity (with its category) and the best allowed similarity. The moderator's rule
//! is "block iff some piece and category has `best_c >= min_similarity` and
//! `best_c - best_a >= margin`"; `best_a` is the same for every category of a piece, so
//! the best category of a piece decides for the whole piece, and the stored numbers
//! reproduce the rule exactly for any threshold pair.
//!
//! The proposal is a tuning fit on the held-out lines: the thresholds are chosen on the
//! very lines they are then counted on, so the counts are in-sample for the threshold
//! choice, not a held-out estimate. A caught banned line counts as caught under its own
//! category or under another one (the category the moderator would report).

use std::fmt::Write as _;
use std::path::Path;

use anyhow::anyhow;
use guard_kit::{load_lists, ModerationLists};
use theos_ai::{cosine, moderation_pieces, EmbedError, EmbedInput, Embedder};
use theos_ai_llama::{models_dir, LlamaEmbedder, LlamaRuntime, ModelsFile};

/// Whether a held-out line belongs to a banned category or to the allowed list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// A held-out line of a banned category.
    Banned,
    /// A held-out line of `allowed.toml`.
    Allowed,
}

/// The scores of one piece of a line.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceScore {
    /// The closest banned category.
    pub category: String,
    /// Its similarity.
    pub banned: f32,
    /// The similarity to the closest allowed example.
    pub allowed: Option<f32>,
}

/// The scores of one held-out line.
#[derive(Clone, Debug, PartialEq)]
pub struct TuneLine {
    /// The list the line came from: a category id, or `allowed`.
    pub list: String,
    /// Banned or allowed.
    pub kind: LineKind,
    /// The line.
    pub line: String,
    /// One score per piece (the whole text plus each sentence).
    pub pieces: Vec<PieceScore>,
}

impl TuneLine {
    /// The piece with the highest banned similarity, for the table.
    pub fn best(&self) -> Option<&PieceScore> {
        self.pieces.iter().fold(None, |top, p| match top {
            Some(t) if t.banned >= p.banned => Some(t),
            _ => Some(p),
        })
    }

    /// Whether the moderator would block this line at `min_similarity` and `margin`.
    pub fn blocks(&self, min_similarity: f32, margin: f32) -> bool {
        self.blocked_category(min_similarity, margin).is_some()
    }

    /// The category the moderator would report for this line: that of the blocking piece
    /// with the highest banned similarity (the first of equals); `None` when no piece blocks.
    pub fn blocked_category(&self, min_similarity: f32, margin: f32) -> Option<&str> {
        let mut top: Option<&PieceScore> = None;
        for p in &self.pieces {
            let blocks = p.banned >= min_similarity
                && p.banned - p.allowed.unwrap_or(f32::NEG_INFINITY) >= margin;
            if blocks && top.is_none_or(|t| p.banned > t.banned) {
                top = Some(p);
            }
        }
        top.map(|p| p.category.as_str())
    }
}

/// How many held-out lines of one category a threshold pair catches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catch {
    /// The category id.
    pub category: String,
    /// Lines that would block and be reported as this very category.
    pub caught: usize,
    /// Lines that would block but be reported as another category, per that category
    /// in order of first appearance.
    pub other: Vec<(String, usize)>,
    /// Held-out lines of the category.
    pub total: usize,
}

impl Catch {
    /// Lines that would block under any category.
    pub fn blocked(&self) -> usize {
        self.caught + self.other.iter().map(|(_, n)| n).sum::<usize>()
    }
}

/// A threshold pair and what it does to the held-out lines.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// The minimum similarity.
    pub min_similarity: f32,
    /// The margin.
    pub margin: f32,
    /// Banned held-out lines caught, per category in order of first appearance.
    pub caught: Vec<Catch>,
    /// Banned held-out lines that would block in all, under whatever category.
    pub caught_total: usize,
    /// The allowed held-out lines this pair would block.
    pub blocked_allowed: Vec<String>,
}

/// The two proposals [`propose`] makes.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    /// The best pair that blocks no allowed held-out line; `None` when none exists.
    pub strict: Option<Choice>,
    /// The best pair that may block one allowed held-out line, for transparency.
    pub one_allowed: Option<Choice>,
}

/// Grid bounds in hundredths: min_similarity 0.30..=0.95, margin 0.00..=0.20.
const MIN_RANGE: std::ops::RangeInclusive<u32> = 30..=95;
const MARGIN_RANGE: std::ops::RangeInclusive<u32> = 0..=20;

fn hundredths(n: u32) -> f32 {
    // n is at most 95, so the conversion is exact; the division is correctly rounded,
    // like the literal `0.xx` in a config file.
    n as f32 / 100.0
}

fn evaluate_pair(lines: &[TuneLine], min_similarity: f32, margin: f32) -> Choice {
    let mut caught: Vec<Catch> = Vec::new();
    let mut blocked_allowed = Vec::new();
    for l in lines {
        let blocks = l.blocks(min_similarity, margin);
        match l.kind {
            LineKind::Allowed => {
                if blocks {
                    blocked_allowed.push(l.line.clone());
                }
            }
            LineKind::Banned => {
                let at = match caught.iter().position(|c| c.category == l.list) {
                    Some(at) => at,
                    None => {
                        caught.push(Catch {
                            category: l.list.clone(),
                            caught: 0,
                            other: Vec::new(),
                            total: 0,
                        });
                        caught.len() - 1
                    }
                };
                caught[at].total += 1;
                if let Some(reported) = l.blocked_category(min_similarity, margin) {
                    if reported == l.list {
                        caught[at].caught += 1;
                    } else if let Some(o) = caught[at].other.iter_mut().find(|(c, _)| c == reported)
                    {
                        o.1 += 1;
                    } else {
                        caught[at].other.push((reported.to_owned(), 1));
                    }
                }
            }
        }
    }
    Choice {
        min_similarity,
        margin,
        caught_total: caught.iter().map(Catch::blocked).sum(),
        caught,
        blocked_allowed,
    }
}

/// The pair with the most banned lines caught among those blocking at most
/// `max_blocked` allowed lines; ties go to the higher `min_similarity`, then the
/// higher margin.
fn best_pair(lines: &[TuneLine], max_blocked: usize) -> Option<Choice> {
    let mut best: Option<(Choice, u32, u32)> = None;
    for m in MIN_RANGE {
        for g in MARGIN_RANGE {
            let c = evaluate_pair(lines, hundredths(m), hundredths(g));
            if c.blocked_allowed.len() > max_blocked {
                continue;
            }
            // Iteration is ascending, so `>=` keeps the later (higher) pair on a tie.
            let better = best
                .as_ref()
                .is_none_or(|(b, _, _)| c.caught_total >= b.caught_total);
            if better {
                best = Some((c, m, g));
            }
        }
    }
    best.map(|(c, _, _)| c)
}

/// Grid-searches `min_similarity` 0.30..=0.95 and `margin` 0.00..=0.20 in steps of 0.01
/// under the moderator's rule, once with no allowed held-out line blocked and once
/// with at most one.
pub fn propose(lines: &[TuneLine]) -> Proposal {
    Proposal {
        strict: best_pair(lines, 0),
        one_allowed: best_pair(lines, 1),
    }
}

fn embed_all<T: AsRef<str>>(
    embedder: &mut dyn Embedder,
    texts: &[T],
) -> Result<Vec<Vec<f32>>, EmbedError> {
    let inputs: Vec<EmbedInput<'_>> = texts
        .iter()
        .map(|t| EmbedInput::Document(t.as_ref()))
        .collect();
    let out = embedder.embed(&inputs)?;
    if out.len() != inputs.len() {
        return Err(EmbedError::CountMismatch {
            expected: inputs.len(),
            got: out.len(),
        });
    }
    Ok(out)
}

/// The best similarity of `vector` among `examples`; NaN never wins.
fn best_of(examples: &[Vec<f32>], vector: &[f32]) -> Option<f32> {
    examples
        .iter()
        .map(|e| cosine(vector, e))
        .filter(|s| !s.is_nan())
        .fold(None, |top: Option<f32>, s| {
            Some(top.map_or(s, |t| t.max(s)))
        })
}

/// Scores every held-out line of `lists` as a player message.
///
/// # Errors
///
/// When the embedder fails.
pub fn score_lines(
    embedder: &mut dyn Embedder,
    lists: &ModerationLists,
) -> Result<Vec<TuneLine>, EmbedError> {
    let mut banned: Vec<(&str, Vec<Vec<f32>>)> = Vec::new();
    for c in &lists.banned {
        let texts: Vec<&str> = c.examples.iter().map(String::as_str).collect();
        let vectors = if texts.is_empty() {
            Vec::new()
        } else {
            embed_all(embedder, &texts)?
        };
        banned.push((c.category.id(), vectors));
    }
    let allowed_texts: Vec<&str> = lists.allowed.iter().map(String::as_str).collect();
    let allowed = if allowed_texts.is_empty() {
        Vec::new()
    } else {
        embed_all(embedder, &allowed_texts)?
    };

    let held_banned = lists.banned.iter().flat_map(|c| {
        c.held_out
            .iter()
            .map(move |l| (c.category.id(), LineKind::Banned, l))
    });
    let held_allowed = lists
        .allowed_held_out
        .iter()
        .map(|l| ("allowed", LineKind::Allowed, l));
    let mut out = Vec::new();
    for (list, kind, line) in held_banned.chain(held_allowed) {
        let texts = moderation_pieces(line);
        let vectors = if texts.is_empty() {
            Vec::new()
        } else {
            embed_all(embedder, &texts)?
        };
        let mut scores = Vec::new();
        for vector in &vectors {
            // The best category of the piece decides for the piece (see the module docs).
            let mut top: Option<(&str, f32)> = None;
            for (id, examples) in &banned {
                if let Some(s) = best_of(examples, vector) {
                    if top.is_none_or(|(_, t)| s > t) {
                        top = Some((id, s));
                    }
                }
            }
            if let Some((id, s)) = top {
                scores.push(PieceScore {
                    category: id.to_owned(),
                    banned: s,
                    allowed: best_of(&allowed, vector),
                });
            }
        }
        out.push(TuneLine {
            list: list.to_owned(),
            kind,
            line: line.clone(),
            pieces: scores,
        });
    }
    Ok(out)
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\r', '\n'], " ")
}

fn describe(out: &mut String, c: &Choice) {
    let _ = writeln!(
        out,
        "min_similarity = {:.2}, margin = {:.2}: catches {} banned held-out lines, blocks {} allowed held-out lines",
        c.min_similarity,
        c.margin,
        c.caught_total,
        c.blocked_allowed.len()
    );
    for k in &c.caught {
        let others = if k.other.is_empty() {
            "0".to_owned()
        } else {
            let parts: Vec<String> = k.other.iter().map(|(c, n)| format!("{n} as {c}")).collect();
            format!(
                "{} ({})",
                k.other.iter().map(|(_, n)| n).sum::<usize>(),
                parts.join(", ")
            )
        };
        let _ = writeln!(
            out,
            "- {}: {}/{} caught as {}, {} caught under another category",
            k.category, k.caught, k.total, k.category, others
        );
    }
}

/// How many held-out lines have a verdict that depends on `min_similarity`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decides {
    /// Lines whose verdict at the proposed pair differs from the verdict at the grid floor
    /// (`min_similarity` 0.30) with the same margin.
    pub total: usize,
    /// Of those, banned held-out lines.
    pub banned: usize,
    /// Of those, allowed held-out lines.
    pub allowed: usize,
    /// All held-out lines.
    pub lines: usize,
}

/// Counts the lines `min_similarity` decides at `c`: the verdict (the real per-piece rule)
/// at the proposed pair against the verdict at the grid floor with the same margin.
pub fn min_similarity_decides(lines: &[TuneLine], c: &Choice) -> Decides {
    let floor = hundredths(*MIN_RANGE.start());
    let mut d = Decides {
        total: 0,
        banned: 0,
        allowed: 0,
        lines: lines.len(),
    };
    for l in lines {
        if l.blocks(c.min_similarity, c.margin) != l.blocks(floor, c.margin) {
            d.total += 1;
            match l.kind {
                LineKind::Banned => d.banned += 1,
                LineKind::Allowed => d.allowed += 1,
            }
        }
    }
    d
}

fn floor_note(lines: &[TuneLine], c: &Choice) -> String {
    let d = min_similarity_decides(lines, c);
    format!(
        "min_similarity decides {} of {} held-out lines (banned: {}, allowed: {}); the margin decides the rest.\n",
        d.total, d.lines, d.banned, d.allowed
    )
}

/// The table and the proposals as text.
pub fn render(lines: &[TuneLine], proposal: &Proposal) -> String {
    let mut out = String::from(
        "| list | line | best banned | similarity | best allowed similarity (same piece) |\n|---|---|---|---|---|\n",
    );
    for l in lines {
        let (cat, sim, alw) = l.best().map_or_else(
            || ("n/a".to_owned(), "n/a".to_owned(), "n/a".to_owned()),
            |p| {
                (
                    p.category.clone(),
                    format!("{:.3}", p.banned),
                    p.allowed
                        .map_or_else(|| "n/a".to_owned(), |s| format!("{s:.3}")),
                )
            },
        );
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            l.list,
            cell(&l.line),
            cat,
            sim,
            alw
        );
    }
    out.push('\n');
    match &proposal.strict {
        None => out.push_str(
            "No pair in the grid blocks zero allowed held-out lines: nothing can be proposed.\n",
        ),
        Some(c) => {
            out.push_str("Tuning fit on the held-out lines (in-sample for the threshold choice, not a held-out estimate): most banned held-out lines caught, zero allowed blocked; ties go to the higher min_similarity, then the higher margin:\n\n");
            let _ = writeln!(
                out,
                "[player]\nmin_similarity = {:.2}\nmargin = {:.2}\n\n[reply]\nmin_similarity = {:.2}\nmargin = {:.2}\n",
                c.min_similarity, c.margin, c.min_similarity, c.margin
            );
            out.push_str("The [reply] values are copied from the player side: they are untuned (there are no held-out model replies).\n\n");
            describe(&mut out, c);
            out.push('\n');
            out.push_str(&floor_note(lines, c));
        }
    }
    out.push_str("\nFor transparency, the best pair if one allowed held-out line may block:\n\n");
    match &proposal.one_allowed {
        None => out.push_str("none\n"),
        Some(c) => {
            describe(&mut out, c);
            for l in &c.blocked_allowed {
                let _ = writeln!(out, "- would block allowed line: {}", cell(l));
            }
        }
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
    let mut embedder = LlamaEmbedder::load(rt, &path).map_err(|e| anyhow!("{e}"))?;
    let lines = score_lines(&mut embedder, &lists).map_err(|e| anyhow!("{e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use guard_kit::CategoryList;
    use theos_ai::testkit::FakeEmbedder;
    use theos_ai::{KnnThresholds, ModerationCategory, SourceThreshold};

    /// A line with one piece per `(category, banned, allowed)`.
    fn tl(list: &str, kind: LineKind, pieces: &[(&str, f32, Option<f32>)]) -> TuneLine {
        TuneLine {
            list: list.into(),
            kind,
            line: format!("a {list} line {}", pieces.len()),
            pieces: pieces
                .iter()
                .map(|(c, b, a)| PieceScore {
                    category: (*c).into(),
                    banned: *b,
                    allowed: *a,
                })
                .collect(),
        }
    }

    fn allowed(banned: f32, alw: f32) -> TuneLine {
        tl("allowed", LineKind::Allowed, &[("hate", banned, Some(alw))])
    }

    fn banned(list: &str, b: f32, a: f32) -> TuneLine {
        tl(list, LineKind::Banned, &[(list, b, Some(a))])
    }

    #[test]
    fn margin_separates_what_the_threshold_cannot() {
        // The allowed line is closer to a banned example (0.85) than the banned line is
        // (0.70), so no threshold alone separates them; the margin does.
        let lines = [allowed(0.85, 0.84), banned("hate", 0.70, 0.40)];
        let p = propose(&lines);
        let c = p.strict.unwrap();
        assert_eq!((c.min_similarity, c.margin), (0.70, 0.20));
        assert_eq!(c.caught_total, 1);
        assert!(c.blocked_allowed.is_empty());
    }

    #[test]
    fn ties_go_to_the_higher_min_similarity_then_the_higher_margin() {
        let lines = [banned("hate", 0.90, 0.10)];
        let c = propose(&lines).strict.unwrap();
        // Every pair with min <= 0.90 and margin <= 0.20 catches the line; the highest wins.
        assert_eq!((c.min_similarity, c.margin), (0.90, 0.20));
    }

    #[test]
    fn a_line_blocks_through_any_piece() {
        let line = tl(
            "hate",
            LineKind::Banned,
            &[("hate", 0.90, Some(0.95)), ("hate", 0.60, Some(0.10))],
        );
        assert!(
            line.blocks(0.60, 0.20),
            "the second piece passes the margin"
        );
        assert!(!line.blocks(0.61, 0.20));
        assert!(!line.blocks(0.50, 0.60));
    }

    #[test]
    fn no_allowed_similarity_means_only_the_threshold_matters() {
        let line = tl("hate", LineKind::Banned, &[("hate", 0.5, None)]);
        assert!(line.blocks(0.5, 0.2));
    }

    #[test]
    fn the_one_allowed_pair_names_the_line_it_would_block() {
        // Blocking the allowed line (0.80, 0.79) is the only way to catch the second
        // banned line (0.80, 0.795): their numbers are too alike for any threshold or margin.
        let a = allowed(0.80, 0.79);
        let lines = [
            a.clone(),
            banned("hate", 0.90, 0.10),
            banned("hate", 0.80, 0.795),
        ];
        let p = propose(&lines);
        assert_eq!(p.strict.as_ref().unwrap().caught_total, 1);
        let relaxed = p.one_allowed.unwrap();
        assert_eq!(relaxed.caught_total, 2);
        assert_eq!(relaxed.blocked_allowed, [a.line]);
    }

    #[test]
    fn caught_is_counted_per_category() {
        let lines = [
            allowed(0.30, 0.20),
            banned("hate", 0.80, 0.10),
            banned("hate", 0.20, 0.10),
            banned("self-harm", 0.80, 0.10),
        ];
        let c = propose(&lines).strict.unwrap();
        assert_eq!(
            c.caught,
            [
                Catch {
                    category: "hate".into(),
                    caught: 1,
                    other: Vec::new(),
                    total: 2
                },
                Catch {
                    category: "self-harm".into(),
                    caught: 1,
                    other: Vec::new(),
                    total: 1
                }
            ]
        );
    }

    #[test]
    fn caught_separates_the_own_category_from_another_category() {
        // The second "hate" line is blocked, but its closest banned example is a
        // self-harm one: the moderator would report self-harm for it.
        let cross = tl("hate", LineKind::Banned, &[("self-harm", 0.80, Some(0.10))]);
        let lines = [
            allowed(0.30, 0.20),
            banned("hate", 0.80, 0.10),
            cross.clone(),
            banned("hate", 0.20, 0.10),
        ];
        assert_eq!(cross.blocked_category(0.5, 0.0), Some("self-harm"));
        assert_eq!(cross.blocked_category(0.9, 0.0), None);
        let c = propose(&lines).strict.unwrap();
        assert_eq!(
            c.caught,
            [Catch {
                category: "hate".into(),
                caught: 1,
                other: vec![("self-harm".into(), 1)],
                total: 3
            }]
        );
        assert_eq!(c.caught_total, 2, "any block counts for the search");
        let text = render(&lines, &propose(&lines));
        assert!(
            text.contains(
                "- hate: 1/3 caught as hate, 1 (1 as self-harm) caught under another category"
            ),
            "{text}"
        );
    }

    #[test]
    fn the_reported_category_is_that_of_the_best_blocking_piece() {
        let line = tl(
            "hate",
            LineKind::Banned,
            &[("hate", 0.70, Some(0.1)), ("self-harm", 0.90, Some(0.1))],
        );
        assert_eq!(line.blocked_category(0.5, 0.0), Some("self-harm"));
        // The 0.90 piece fails the margin: the other one reports.
        let line = tl(
            "hate",
            LineKind::Banned,
            &[("hate", 0.70, Some(0.1)), ("self-harm", 0.90, Some(0.85))],
        );
        assert_eq!(line.blocked_category(0.5, 0.2), Some("hate"));
    }

    #[test]
    fn render_labels_the_fit_in_sample_and_gives_the_floor() {
        let lines = [allowed(0.30, 0.29), banned("hate", 0.70, 0.40)];
        let text = render(&lines, &propose(&lines));
        assert!(
            text.contains("in-sample for the threshold choice"),
            "{text}"
        );
        assert!(text.contains("not a held-out estimate"));
        assert!(text.contains("[reply] values are copied from the player side"));
        assert!(text.contains("untuned"));
        assert!(text.contains("min_similarity decides "), "{text}");
        assert!(text.contains("the margin decides the rest"), "{text}");
        assert!(!text.contains("both conditions decide"));
    }

    fn choice(min_similarity: f32, margin: f32) -> Choice {
        Choice {
            min_similarity,
            margin,
            caught: Vec::new(),
            caught_total: 0,
            blocked_allowed: Vec::new(),
        }
    }

    #[test]
    fn min_similarity_decides_a_line_the_floor_would_block() {
        // The allowed line (0.45 vs 0.10) passes the margin and would block at the grid floor
        // 0.30; only min_similarity 0.60 lets it through. The banned line blocks either way.
        let lines = [
            allowed(0.45, 0.10),
            banned("hate", 0.80, 0.20),
            allowed(0.20, 0.10),
        ];
        let d = min_similarity_decides(&lines, &choice(0.60, 0.10));
        assert_eq!(
            d,
            Decides {
                total: 1,
                banned: 0,
                allowed: 1,
                lines: 3
            }
        );
    }

    #[test]
    fn min_similarity_decides_a_banned_line_it_makes_the_moderator_miss() {
        // A banned line at 0.50 blocks at the floor but not at 0.60: min_similarity loses it.
        let lines = [banned("hate", 0.50, 0.10), banned("hate", 0.90, 0.10)];
        let d = min_similarity_decides(&lines, &choice(0.60, 0.10));
        assert_eq!((d.total, d.banned, d.allowed), (1, 1, 0));
    }

    #[test]
    fn min_similarity_decides_nothing_when_the_margin_separates_every_line() {
        // Every line is either far above both thresholds or fails the margin at the floor too.
        let lines = [
            allowed(0.85, 0.84),
            banned("hate", 0.90, 0.10),
            allowed(0.10, 0.05),
        ];
        let d = min_similarity_decides(&lines, &choice(0.60, 0.20));
        assert_eq!(d.total, 0);
        let text = render(&lines, &propose(&lines));
        assert!(
            text.contains("min_similarity decides 0 of 3 held-out lines (banned: 0, allowed: 0); the margin decides the rest."),
            "{text}"
        );
    }

    #[test]
    fn no_strict_pair_when_an_allowed_line_beats_the_whole_grid() {
        let lines = [allowed(0.99, 0.0), banned("hate", 0.9, 0.1)];
        let p = propose(&lines);
        assert!(p.strict.is_none());
        assert!(p.one_allowed.is_some());
    }

    #[test]
    fn tune_uses_the_shared_piece_split() {
        let line = "We hate the northern people so much. Is it?";
        let mut e = FakeEmbedder::default();
        let mut l = lists();
        l.banned[0].held_out = vec![line.into()];
        let lines = score_lines(&mut e, &l).unwrap();
        assert_eq!(
            lines[0].pieces.len(),
            moderation_pieces(line).len(),
            "no piece for the two-word sentence"
        );
        assert_eq!(lines[0].pieces.len(), 2);
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
                held_out: vec![
                    "we hate the northern people so much. good morning to you all".into(),
                ],
            }],
            allowed: vec!["good morning dear friend".into()],
            allowed_held_out: vec!["good morning friend".into()],
            thresholds: KnnThresholds {
                player: t,
                reply: t,
            },
        }
    }

    #[test]
    fn score_lines_scores_every_piece_of_every_held_out_line() {
        let mut e = FakeEmbedder::default();
        let lines = score_lines(&mut e, &lists()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].kind, LineKind::Banned);
        assert_eq!(lines[0].list, "hate");
        assert_eq!(lines[0].pieces.len(), 3, "whole text and two sentences");
        let first_sentence = &lines[0].pieces[1];
        assert!(first_sentence.banned > 0.7, "{first_sentence:?}");
        let second_sentence = &lines[0].pieces[2];
        assert!(second_sentence.allowed.unwrap() > second_sentence.banned);
        assert_eq!(lines[1].kind, LineKind::Allowed);
        assert_eq!(lines[1].pieces.len(), 1);
        assert_eq!(lines[0].best().unwrap().category, "hate");
    }

    #[test]
    fn render_shows_the_table_the_pair_the_catches_and_the_relaxed_pair() {
        let a = allowed(0.80, 0.79);
        let lines = [a, banned("hate", 0.90, 0.10), banned("hate", 0.80, 0.795)];
        let text = render(&lines, &propose(&lines));
        assert!(text.contains("| allowed | a allowed line 1 | hate | 0.800 | 0.790 |"));
        assert!(text.contains("[player]\nmin_similarity = "));
        assert!(text.contains("[reply]\nmin_similarity = "));
        assert!(text.contains("- hate: 1/2"));
        assert!(text.contains("one allowed held-out line may block"));
        assert!(text.contains("- would block allowed line: a allowed line 1"));
    }

    #[test]
    fn render_without_a_strict_pair_says_so() {
        let lines = [allowed(0.99, 0.0)];
        let text = render(&lines, &propose(&lines));
        assert!(text.contains("nothing can be proposed"));
    }
}
