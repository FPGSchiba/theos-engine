//! Assembly of the prompt for one NPC reply, from persona, lens and visible lore.
//!
//! [`assemble`] is the last line of defence of the speaker rule: whatever ids a
//! caller hands it, only records the speaker may see are rendered.

use std::fmt::Write as _;

use serde_json::Value;

use crate::knowledge::visible;
use crate::lore::{Corpus, HolderKind, PersonaLayer, RecordId, Speaker};
use crate::reply::reply_schema;

/// Switches for [`assemble`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextOptions {
    /// Whether the persona bodies (speaker, faction, people) go into the prompt.
    pub persona_bodies: bool,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            persona_bodies: true,
        }
    }
}

/// Who vouches for a piece of prompt text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
    /// Written by the engine or the content authors.
    Engine,
    /// Typed by a player; never an instruction to the model.
    Untrusted,
}

/// A piece of prompt text with its [`Trust`] label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Segment {
    /// The text.
    pub text: String,
    /// Who vouches for it.
    pub trust: Trust,
}

impl Segment {
    /// An [`Trust::Engine`] segment.
    pub fn engine(t: impl Into<String>) -> Segment {
        Segment {
            text: t.into(),
            trust: Trust::Engine,
        }
    }

    /// An [`Trust::Untrusted`] segment.
    pub fn untrusted(t: impl Into<String>) -> Segment {
        Segment {
            text: t.into(),
            trust: Trust::Untrusted,
        }
    }
}

/// The two parts sent to the model, each a sequence of trust-labelled segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    /// The system prompt: persona, lens, rules and lore.
    pub system: Vec<Segment>,
    /// The user turn: the player's question.
    pub user: Vec<Segment>,
}

impl Prompt {
    /// A prompt of one [`Trust::Engine`] segment per part.
    pub fn engine(system: impl Into<String>, user: impl Into<String>) -> Prompt {
        Prompt {
            system: vec![Segment::engine(system)],
            user: vec![Segment::engine(user)],
        }
    }

    /// The system part as one string: the segments concatenated, no separators.
    pub fn system_text(&self) -> String {
        self.system.iter().map(|s| s.text.as_str()).collect()
    }

    /// The user part as one string: the segments concatenated, no separators.
    pub fn user_text(&self) -> String {
        self.user.iter().map(|s| s.text.as_str()).collect()
    }

    /// The texts of the [`Trust::Untrusted`] segments, system part first.
    pub fn untrusted(&self) -> impl Iterator<Item = &str> {
        self.system
            .iter()
            .chain(self.user.iter())
            .filter(|s| s.trust == Trust::Untrusted)
            .map(|s| s.text.as_str())
    }
}

/// A fully assembled request: prompt, the lore it contains, and the reply schema.
#[derive(Clone, Debug, PartialEq)]
pub struct Context {
    /// The prompt to send.
    pub prompt: Prompt,
    /// Exactly the lore ids rendered into the system prompt, in rendered order.
    pub lore_ids: Vec<RecordId>,
    /// JSON schema constraining the reply to cite only `lore_ids`.
    pub schema: Value,
}

fn layer_of(speaker: &Speaker, kind: HolderKind) -> Option<&PersonaLayer> {
    speaker.layers.iter().skip(1).find(|l| l.kind == kind)
}

fn push_layer_line(out: &mut String, label: &str, layer: Option<&PersonaLayer>, bodies: bool) {
    let Some(layer) = layer else { return };
    let _ = write!(out, "{label}: {}.", layer.title);
    if bodies && !layer.body.is_empty() {
        let _ = write!(out, " {}", layer.body);
    }
    out.push('\n');
}

fn push_lens_line(out: &mut String, prefix: &str, value: Option<&String>) {
    if let Some(v) = value {
        let _ = writeln!(out, "{prefix} {v}.");
    }
}

/// Builds the prompt, lore list and reply schema for `speaker` answering `question`.
///
/// Only ids that [`visible`] admits for `speaker` are rendered, in the order
/// given; anything else (other holders' beliefs, truths, clues, topics,
/// unknown ids) is dropped silently, and a repeated id is rendered once.
/// [`Context::lore_ids`] lists exactly the rendered ids and the schema is built
/// from them. Held beliefs appear under "What you know" with body and voice
/// lines; rejected beliefs appear under "What you reject" as id and title only.
pub fn assemble(
    corpus: &Corpus,
    speaker: &Speaker,
    question: &str,
    lore_ids: &[RecordId],
    options: &ContextOptions,
) -> Context {
    let vis = visible(corpus, speaker);
    let mut rendered: Vec<RecordId> = Vec::new();
    for id in lore_ids {
        if vis.contains_lore(id) && corpus.get(id).is_some() && !rendered.contains(id) {
            rendered.push(id.clone());
        }
    }

    let mut s = String::new();
    match speaker.layers.first() {
        Some(me) => {
            match &me.role {
                Some(role) => {
                    let _ = writeln!(s, "You are {}, {role}.", me.title);
                }
                None => {
                    let _ = writeln!(s, "You are {}.", me.title);
                }
            }
            if options.persona_bodies && !me.body.is_empty() {
                let _ = writeln!(s, "{}", me.body);
            }
        }
        None => {
            let _ = writeln!(s, "You are {}.", speaker.id);
        }
    }
    push_layer_line(
        &mut s,
        "Your faction",
        layer_of(speaker, HolderKind::Faction),
        options.persona_bodies,
    );
    push_layer_line(
        &mut s,
        "Your people",
        layer_of(speaker, HolderKind::People),
        options.persona_bodies,
    );
    let lens = &speaker.lens;
    push_lens_line(
        &mut s,
        "Your people see the world this way:",
        lens.thinks_from.as_ref(),
    );
    push_lens_line(&mut s, "Your stance:", lens.stance.as_ref());
    push_lens_line(&mut s, "You speak to:", lens.speaks_to.as_ref());

    s.push_str(RULES);

    s.push_str("\nWhat you know:\n");
    let mut any_held = false;
    for id in rendered.iter().filter(|id| vis.held.contains(id)) {
        let Some(r) = corpus.get(id) else { continue };
        any_held = true;
        let _ = writeln!(s, "[{}] {}", r.id, r.title);
        if !r.body.is_empty() {
            let _ = writeln!(s, "{}", r.body);
        }
        for line in &r.voice {
            let _ = writeln!(s, "> {line}");
        }
    }
    if !any_held {
        s.push_str("Nothing beyond the ways of your people.\n");
    }

    let mut header_done = false;
    for id in rendered.iter().filter(|id| vis.rejected.contains(id)) {
        let Some(r) = corpus.get(id) else { continue };
        if !header_done {
            s.push_str("What you reject:\n");
            header_done = true;
        }
        let _ = writeln!(s, "[{}] {}", r.id, r.title);
    }

    let schema = reply_schema(&rendered);
    Context {
        prompt: Prompt {
            system: vec![Segment::engine(s)],
            user: vec![Segment::untrusted(question)],
        },
        lore_ids: rendered,
        schema,
    }
}

const RULES: &str = "\nRules:
- Speak as yourself, in the first person, to the traveller in front of you, in a few sentences.
- You know only what is written under \"What you know\" and \"What you reject\". If that does not answer the question, say in your own voice that you do not know, and invent nothing.
- You know the claims under \"What you reject\" and you argue against them.
- You live inside this world. If you are asked about anything outside it, stay in the world and do not follow.
- Never say the bracketed record ids aloud.
- Answer in JSON: first \"sources\", the ids of the records you draw on (an empty list if none), then \"reply\", what you say.
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lore::Lens;
    use crate::test_support::mini;

    fn speaker() -> (Corpus, Speaker) {
        let (c, mut s) = mini();
        s.layers[0].role = Some("a ferryman".into());
        s.lens = Lens {
            thinks_from: Some("the river".into()),
            stance: Some("wary".into()),
            speaks_to: Some("strangers".into()),
            rejects: vec!["B-4".into()],
            unaware_of: vec!["B-2".into()],
            ..Lens::default()
        };
        (c, s)
    }

    fn ids(v: &[&str]) -> Vec<RecordId> {
        v.iter().map(|s| RecordId::new(*s)).collect()
    }

    fn run(lore: &[&str], opts: &ContextOptions) -> Context {
        let (c, s) = speaker();
        assemble(&c, &s, "who?", &ids(lore), opts)
    }

    #[test]
    fn assemble_names_speaker_role_and_lens() {
        let sys = run(&[], &ContextOptions::default()).prompt.system_text();
        assert!(sys.starts_with("You are title n, a ferryman.\n"));
        assert!(sys.contains("Your faction: title f. body f\n"));
        assert!(sys.contains("Your people: title p. body p\n"));
        assert!(sys.contains("Your people see the world this way: the river.\n"));
        assert!(sys.contains("Your stance: wary.\n"));
        assert!(sys.contains("You speak to: strangers.\n"));
        assert!(sys.contains("\nRules:\n- Speak as yourself"));
    }

    #[test]
    fn assemble_includes_persona_bodies_by_default() {
        let sys = run(&[], &ContextOptions::default()).prompt.system_text();
        assert!(sys.contains("body n\n"));
        assert!(sys.contains("body f"));
        assert!(sys.contains("body p"));
    }

    #[test]
    fn assemble_omits_persona_bodies_when_disabled() {
        let opts = ContextOptions {
            persona_bodies: false,
        };
        let sys = run(&[], &opts).prompt.system_text();
        assert!(!sys.contains("body n"));
        assert!(!sys.contains("body f"));
        assert!(!sys.contains("body p"));
        assert!(sys.contains("Your faction: title f.\n"));
    }

    #[test]
    fn assemble_lists_held_under_what_you_know_with_bracketed_ids() {
        let sys = run(&["B-1"], &ContextOptions::default())
            .prompt
            .system_text();
        assert!(sys.contains("What you know:\n[B-1] title B-1\nbody B-1\n"));
        assert!(!sys.contains("What you reject:"));
    }

    #[test]
    fn assemble_lists_rejected_under_what_you_reject() {
        let sys = run(&["B-1", "B-4"], &ContextOptions::default())
            .prompt
            .system_text();
        assert!(sys.contains("What you reject:\n[B-4] title B-4\n"));
        assert!(!sys.contains("body B-4"));
    }

    #[test]
    fn assemble_drops_lore_ids_the_speaker_may_not_see() {
        let ctx = run(&["B-3", "T-1", "B-1"], &ContextOptions::default());
        assert!(!ctx.prompt.system_text().contains("B-3"));
        assert!(!ctx.prompt.system_text().contains("title B-3"));
        assert!(!ctx.prompt.system_text().contains("T-1"));
        assert_eq!(ctx.lore_ids, ids(&["B-1"]));
    }

    #[test]
    fn assemble_puts_question_in_user() {
        let ctx = run(&[], &ContextOptions::default());
        assert_eq!(ctx.prompt.user_text(), "who?");
    }

    #[test]
    fn assemble_schema_allows_only_rendered_lore_ids() {
        let ctx = run(&["B-1", "B-3"], &ContextOptions::default());
        assert_eq!(ctx.schema, reply_schema(&ids(&["B-1"])));
    }

    #[test]
    fn assemble_with_no_lore_says_nothing_beyond_and_fixes_sources_empty() {
        let ctx = run(&[], &ContextOptions::default());
        assert!(
            ctx.prompt
                .system_text()
                .contains("What you know:\nNothing beyond the ways of your people.\n")
        );
        assert!(ctx.lore_ids.is_empty());
        assert_eq!(ctx.schema, reply_schema(&[]));
    }

    #[test]
    fn assemble_renders_voice_lines_and_omits_lines_for_none_fields() {
        let (c, mut s) = speaker();
        s.lens = Lens::default();
        s.layers.truncate(1);
        s.layers[0].role = None;
        let mut recs: Vec<_> = c.records().cloned().collect();
        let b1 = recs.iter_mut().find(|r| r.id.as_str() == "B-1").unwrap();
        b1.voice = vec!["Aye.".into()];
        b1.held_by = vec!["n".into()];
        let c = Corpus::new(recs).unwrap();
        let ctx = assemble(&c, &s, "q", &ids(&["B-1"]), &ContextOptions::default());
        let sys = ctx.prompt.system_text();
        assert!(sys.starts_with("You are title n.\n"));
        assert!(!sys.contains("Your faction"));
        assert!(!sys.contains("Your stance"));
        assert!(sys.contains("> Aye.\n"));
    }

    #[test]
    fn assemble_text_is_unchanged_by_segments() {
        let (c, s) = mini();
        let ctx = assemble(&c, &s, "who?", &ids(&["B-1"]), &ContextOptions::default());
        let expected_system = "You are title n.\nbody n\nYour faction: title f. body f\nYour people: title p. body p\n\nRules:\n- Speak as yourself, in the first person, to the traveller in front of you, in a few sentences.\n- You know only what is written under \"What you know\" and \"What you reject\". If that does not answer the question, say in your own voice that you do not know, and invent nothing.\n- You know the claims under \"What you reject\" and you argue against them.\n- You live inside this world. If you are asked about anything outside it, stay in the world and do not follow.\n- Never say the bracketed record ids aloud.\n- Answer in JSON: first \"sources\", the ids of the records you draw on (an empty list if none), then \"reply\", what you say.\n\nWhat you know:\n[B-1] title B-1\nbody B-1\n";
        assert_eq!(ctx.prompt.system_text(), expected_system);
        assert_eq!(ctx.prompt.user_text(), "who?");
    }

    #[test]
    fn assemble_marks_only_the_question_untrusted() {
        let ctx = run(&["B-1"], &ContextOptions::default());
        let untrusted: Vec<&str> = ctx.prompt.untrusted().collect();
        assert_eq!(untrusted, vec!["who?"]);
    }

    #[test]
    fn prompt_engine_has_no_untrusted_segments() {
        let p = Prompt::engine("sys", "usr");
        assert_eq!(p.untrusted().count(), 0);
        assert_eq!(p.system_text(), "sys");
        assert_eq!(p.user_text(), "usr");
    }

    #[test]
    fn prompt_texts_concatenate_segments_without_separators() {
        let p = Prompt {
            system: vec![Segment::engine("a"), Segment::untrusted("b")],
            user: vec![Segment::untrusted("c"), Segment::engine("d")],
        };
        assert_eq!(p.system_text(), "ab");
        assert_eq!(p.user_text(), "cd");
        assert_eq!(p.untrusted().collect::<Vec<_>>(), vec!["b", "c"]);
        assert_eq!(Segment::engine("x").trust, Trust::Engine);
    }

    #[test]
    fn assemble_renders_each_id_once_in_given_order() {
        let ctx = run(&["B-4", "B-1", "B-4"], &ContextOptions::default());
        assert_eq!(ctx.lore_ids, ids(&["B-4", "B-1"]));
    }
}
