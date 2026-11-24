//! Per-request scoring, following the fixture README's "Scoring" section.
//!
//! Every `bool` field of [`Checks`] reads "passed".

use std::time::Duration;

use regex::Regex;
use theos_ai::{LoreScope, NpcResponse, RecordId, StopReason};

use crate::scenarios::{Behaviour, Scenario, VisibleEntry};

/// The VRAM budget: 6 GiB.
pub const VRAM_BUDGET_BYTES: u64 = 6 * 1024 * 1024 * 1024;

/// The outcome of every check for one request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checks {
    /// `Retrieved` only: every `expect` and `rejects` id is in the context.
    pub retrieval: Option<bool>,
    /// Every context id is in the speaker's `visible.json` entry.
    pub leak: bool,
    /// No `forbid` id is in the context.
    pub forbidden: bool,
    /// The forbidden words (global and per scenario) found in the reply;
    /// `None` when there is no reply to look in.
    pub words: Option<Vec<String>>,
    /// The reply is grounded in the right records.
    pub grounding: bool,
    /// Finished within the deadline.
    pub deadline: bool,
    /// Peak VRAM within budget; `None` when the probe gave no sample.
    pub vram: Option<bool>,
}

/// The entries of `list` that occur in `text` as whole words, in any case.
///
/// A phrase matches across any run of whitespace. `\b` does not match inside
/// `nodes`, so plurals are not found, as in the editor's rule.
pub fn forbidden_words(list: &[&str], text: &str) -> Vec<String> {
    list.iter()
        .filter(|phrase| {
            let parts: Vec<String> = phrase.split_whitespace().map(regex::escape).collect();
            if parts.is_empty() {
                return false;
            }
            let pattern = format!(r"(?i)\b{}\b", parts.join(r"\s+"));
            // invariant: every part is regex-escaped, so the pattern is valid.
            Regex::new(&pattern).is_ok_and(|re| re.is_match(text))
        })
        .map(|p| (*p).to_owned())
        .collect()
}

fn listed(set: &[String], id: &RecordId) -> bool {
    set.iter().any(|s| s == id.as_str())
}

/// The grounding rule.
///
/// - `answer` / `rumour`: cites are within `expect` + `also_ok` + `rejects` and
///   include an `expect` id when `expect` is not empty. An empty cite list
///   fails: the answer must rest on records. That includes scenarios with an
///   empty `expect`, where citing an `also_ok` or `rejects` record is enough.
/// - `unaware` / `deflect`: cites nothing, or only `also_ok` ids.
fn grounded(scenario: &Scenario, cites: &[RecordId]) -> bool {
    match scenario.behaviour {
        Behaviour::Unaware | Behaviour::Deflect => {
            cites.iter().all(|c| listed(&scenario.also_ok, c))
        }
        Behaviour::Answer | Behaviour::Rumour => {
            if cites.is_empty() {
                return false;
            }
            let allowed = cites.iter().all(|c| {
                listed(&scenario.expect, c)
                    || listed(&scenario.also_ok, c)
                    || listed(&scenario.rejects, c)
            });
            let uses_expect =
                scenario.expect.is_empty() || cites.iter().any(|c| listed(&scenario.expect, c));
            allowed && uses_expect
        }
    }
}

/// Scores one response against its scenario.
///
/// A response whose reply is an error (unfinished or unparsable) fails
/// grounding; it has no text, so the word check does not apply (`words` is
/// `None`).
pub fn check(
    scenario: &Scenario,
    global_words: &[String],
    visible: &VisibleEntry,
    scope: LoreScope,
    response: &NpcResponse,
    deadline: Duration,
) -> Checks {
    let m = &response.metrics;
    let context = &m.context_ids;

    let retrieval = match scope {
        LoreScope::Retrieved { .. } => Some(
            scenario
                .expect
                .iter()
                .chain(&scenario.rejects)
                .all(|id| context.iter().any(|c| c.as_str() == id)),
        ),
        _ => None,
    };
    let leak = context
        .iter()
        .all(|c| listed(&visible.held, c) || listed(&visible.rejected, c));
    let forbidden = !context.iter().any(|c| listed(&scenario.forbid, c));

    let (words, grounding) = match &response.reply {
        Ok(reply) => {
            let list: Vec<&str> = global_words
                .iter()
                .chain(&scenario.forbid_words)
                .map(String::as_str)
                .collect();
            (
                Some(forbidden_words(&list, &reply.text)),
                grounded(scenario, &reply.sources),
            )
        }
        Err(_) => (None, false),
    };

    Checks {
        retrieval,
        leak,
        forbidden,
        words,
        grounding,
        deadline: m.stop != StopReason::Deadline && m.total <= deadline,
        vram: m.vram_peak_bytes.map(|b| b <= VRAM_BUDGET_BYTES),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use theos_ai::{Metrics, NpcResponse, RecordId, Reply, ReplyError};

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    fn scenario(behaviour: Behaviour) -> Scenario {
        Scenario {
            id: "s".into(),
            npc: "n".into(),
            question: "q".into(),
            behaviour,
            expect: ids(&["B-1"]),
            also_ok: ids(&["B-2"]),
            rejects: ids(&["B-3"]),
            forbid: ids(&["T-1"]),
            forbid_words: vec![],
            hand_scored: false,
            note: String::new(),
        }
    }

    fn visible() -> VisibleEntry {
        VisibleEntry {
            persona: ids(&["n"]),
            held: ids(&["B-1", "B-2"]),
            rejected: ids(&["B-3"]),
            unaware_of: vec![],
        }
    }

    fn response(context: &[&str], sources: &[&str], text: &str) -> NpcResponse {
        NpcResponse {
            reply: Ok(Reply {
                sources: sources.iter().map(|s| RecordId::new(*s)).collect(),
                text: text.into(),
            }),
            raw: String::new(),
            thinking: String::new(),
            metrics: Metrics {
                prompt_tokens: 10,
                thinking_tokens: 0,
                answer_tokens: 5,
                retrieval: Duration::ZERO,
                time_to_first_token: Some(Duration::from_millis(100)),
                time_to_first_answer_token: Some(Duration::from_millis(100)),
                tokens_per_second: 50.0,
                total: Duration::from_millis(500),
                vram_peak_bytes: None,
                context_ids: context.iter().map(|s| RecordId::new(*s)).collect(),
                cited_ids: sources.iter().map(|s| RecordId::new(*s)).collect(),
                stop: StopReason::Finished,
            },
        }
    }

    fn run(s: &Scenario, scope: LoreScope, r: &NpcResponse) -> Checks {
        check(
            s,
            &ids(&["node"]),
            &visible(),
            scope,
            r,
            Duration::from_secs(3),
        )
    }

    fn found(text: &str, list: &[&str]) -> Vec<String> {
        forbidden_words(list, text)
    }

    #[test]
    fn words_match_whole_words_in_any_case() {
        assert_eq!(found("A NODE hums", &["node"]), ["node"]);
    }

    #[test]
    fn words_do_not_match_plurals() {
        assert!(found("Sub-Clavicular Nodes", &["node"]).is_empty());
    }

    #[test]
    fn words_match_phrases_across_whitespace() {
        assert_eq!(found("out  of\nphase", &["out of phase"]), ["out of phase"]);
    }

    #[test]
    fn grounding_answer_needs_an_expect_id() {
        let s = scenario(Behaviour::Answer);
        let ok = run(
            &s,
            LoreScope::None,
            &response(&["B-1"], &["B-1", "B-2"], "x"),
        );
        assert!(ok.grounding);
        let only_also = run(&s, LoreScope::None, &response(&["B-2"], &["B-2"], "x"));
        assert!(!only_also.grounding);
        let none = run(&s, LoreScope::None, &response(&[], &[], "x"));
        assert!(!none.grounding);
    }

    #[test]
    fn grounding_answer_fails_on_cite_outside_allowed() {
        let s = scenario(Behaviour::Rumour);
        let r = run(
            &s,
            LoreScope::None,
            &response(&["B-1"], &["B-1", "B-9"], "x"),
        );
        assert!(!r.grounding);
    }

    #[test]
    fn grounding_answer_with_empty_expect_accepts_rejects_cite() {
        let mut s = scenario(Behaviour::Answer);
        s.expect.clear();
        let r = run(&s, LoreScope::None, &response(&["B-3"], &["B-3"], "x"));
        assert!(r.grounding);
        let outside = run(&s, LoreScope::None, &response(&["B-9"], &["B-9"], "x"));
        assert!(!outside.grounding);
        let empty = run(&s, LoreScope::None, &response(&[], &[], "x"));
        assert!(!empty.grounding, "an answer must rest on records");
    }

    #[test]
    fn grounding_unaware_passes_with_no_cites_or_only_also_ok() {
        let s = scenario(Behaviour::Unaware);
        assert!(run(&s, LoreScope::None, &response(&[], &[], "x")).grounding);
        assert!(run(&s, LoreScope::None, &response(&["B-2"], &["B-2"], "x")).grounding);
        assert!(!run(&s, LoreScope::None, &response(&["B-1"], &["B-1"], "x")).grounding);
    }

    #[test]
    fn grounding_deflect_fails_on_cite_outside_also_ok() {
        let s = scenario(Behaviour::Deflect);
        assert!(!run(&s, LoreScope::None, &response(&["B-3"], &["B-3"], "x")).grounding);
        assert!(run(&s, LoreScope::None, &response(&[], &[], "x")).grounding);
    }

    #[test]
    fn retrieval_needs_every_expect_and_rejects_id() {
        let s = scenario(Behaviour::Answer);
        let scope = LoreScope::Retrieved { top_k: 5 };
        assert_eq!(
            run(&s, scope, &response(&["B-1", "B-3"], &[], "x")).retrieval,
            Some(true)
        );
        assert_eq!(
            run(&s, scope, &response(&["B-1"], &[], "x")).retrieval,
            Some(false)
        );
        assert_eq!(
            run(&s, LoreScope::AllVisible, &response(&["B-1"], &[], "x")).retrieval,
            None
        );
    }

    #[test]
    fn leak_fails_on_id_outside_visible() {
        let s = scenario(Behaviour::Answer);
        assert!(run(&s, LoreScope::None, &response(&["B-1", "B-3"], &[], "x")).leak);
        assert!(!run(&s, LoreScope::None, &response(&["B-1", "B-7"], &[], "x")).leak);
    }

    #[test]
    fn forbidden_fails_when_a_forbid_id_is_in_the_context() {
        let s = scenario(Behaviour::Answer);
        assert!(!run(&s, LoreScope::None, &response(&["T-1"], &[], "x")).forbidden);
        assert!(run(&s, LoreScope::None, &response(&["B-1"], &[], "x")).forbidden);
    }

    #[test]
    fn vram_is_none_without_sample_and_fails_above_six_gib() {
        let s = scenario(Behaviour::Answer);
        let mut r = response(&[], &[], "x");
        assert_eq!(run(&s, LoreScope::None, &r).vram, None);
        r.metrics.vram_peak_bytes = Some(VRAM_BUDGET_BYTES);
        assert_eq!(run(&s, LoreScope::None, &r).vram, Some(true));
        r.metrics.vram_peak_bytes = Some(VRAM_BUDGET_BYTES + 1);
        assert_eq!(run(&s, LoreScope::None, &r).vram, Some(false));
    }

    #[test]
    fn an_unusable_reply_fails_grounding_and_has_no_words() {
        let s = scenario(Behaviour::Unaware);
        let mut r = response(&[], &[], "x");
        r.reply = Err(ReplyError::EmptyReply);
        let c = run(&s, LoreScope::None, &r);
        assert!(!c.grounding);
        assert_eq!(c.words, None, "no reply: words are not applicable");
    }

    #[test]
    fn a_reply_without_forbidden_words_has_an_empty_words_list() {
        let s = scenario(Behaviour::Answer);
        let c = run(&s, LoreScope::None, &response(&["B-1"], &["B-1"], "fine"));
        assert_eq!(c.words, Some(vec![]));
    }

    #[test]
    fn deadline_fails_on_deadline_stop_or_slow_total() {
        let s = scenario(Behaviour::Answer);
        let mut r = response(&[], &[], "x");
        assert!(run(&s, LoreScope::None, &r).deadline);
        r.metrics.total = Duration::from_millis(3001);
        assert!(!run(&s, LoreScope::None, &r).deadline);
        r.metrics.total = Duration::from_millis(10);
        r.metrics.stop = StopReason::Deadline;
        assert!(!run(&s, LoreScope::None, &r).deadline);
    }
}
