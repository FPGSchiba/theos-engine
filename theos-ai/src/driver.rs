//! The request driver: runs one NPC answer end to end and measures it.
//!
//! [`Npc::answer`] selects lore, assembles the prompt, drives a [`Decoder`]
//! token by token under a deadline and a token cap, and returns the parsed
//! reply together with the [`Metrics`] of the run.

use std::time::Duration;

use crate::backend::{Backend, BackendError, GenerationRequest, Phase, Thinking};
use crate::clock::Clock;
use crate::context::{ContextOptions, assemble};
use crate::embed::{EmbedError, Embedder};
use crate::lore::{Corpus, RecordId, Speaker};
use crate::memory::MemoryProbe;
use crate::reply::{Reply, ReplyError, parse_reply};
use crate::retrieval::{LoreScope, Retriever};

/// Deadline used when a caller has no better figure.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(3);
/// Most answer-phase tokens a single reply may generate.
///
/// Thinking-phase tokens do not count toward the cap.
pub const ANSWER_TOKEN_CAP: u32 = 256;

/// Why generation stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StopReason {
    /// The model finished on its own.
    Finished,
    /// The deadline passed.
    Deadline,
    /// [`ANSWER_TOKEN_CAP`] answer tokens were generated.
    TokenCap,
    /// The context window filled up.
    ContextFull,
}

/// What one [`Npc::answer`] call measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    /// Prompt tokens the backend processed.
    pub prompt_tokens: u32,
    /// Thinking-phase tokens generated.
    pub thinking_tokens: u32,
    /// Answer-phase tokens generated.
    pub answer_tokens: u32,
    /// Time spent selecting lore.
    pub retrieval: Duration,
    /// Time from the start of the request to the first token, if any token came.
    pub time_to_first_token: Option<Duration>,
    /// Generation speed between the first and last token; `0.0` when it cannot
    /// be measured (fewer than two tokens, or no time between them).
    pub tokens_per_second: f32,
    /// Time from the start of the request to its end.
    pub total: Duration,
    /// Highest video-memory sample taken, if the probe reported any.
    pub vram_peak_bytes: Option<u64>,
    /// Lore ids rendered into the prompt.
    pub context_ids: Vec<RecordId>,
    /// Lore ids the reply cites; empty when there is no parsed reply.
    pub cited_ids: Vec<RecordId>,
    /// Why generation stopped.
    pub stop: StopReason,
}

/// One question put to an NPC.
#[derive(Clone, Debug)]
pub struct NpcRequest {
    /// The speaker answering.
    pub speaker: RecordId,
    /// The player's question, verbatim.
    pub question: String,
    /// How much lore to show the speaker.
    pub scope: LoreScope,
    /// Whether the model may think first.
    pub thinking: Thinking,
    /// Time allowed from the start of the request until generation is cut off.
    pub deadline: Duration,
}

/// The outcome of one [`Npc::answer`] call.
#[derive(Clone, Debug, PartialEq)]
pub struct NpcResponse {
    /// The parsed reply, or why the output could not be used.
    pub reply: Result<Reply, ReplyError>,
    /// The answer-phase text exactly as generated.
    pub raw: String,
    /// The thinking-phase text exactly as generated.
    pub thinking: String,
    /// What the run measured.
    pub metrics: Metrics,
}

/// Why [`Npc::new`] or [`Npc::answer`] failed outright.
///
/// An unusable model output is not one of these; it is a [`ReplyError`] inside
/// the [`NpcResponse`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NpcError {
    /// The request names a speaker that was not supplied.
    #[error("unknown speaker `{0}`")]
    UnknownSpeaker(RecordId),
    /// Two supplied speakers share an id.
    #[error("duplicate speaker `{0}`")]
    DuplicateSpeaker(RecordId),
    /// Embedding failed.
    #[error(transparent)]
    Embed(#[from] EmbedError),
    /// The backend failed.
    #[error(transparent)]
    Backend(#[from] BackendError),
}

/// Everything an [`Npc`] is made of.
pub struct NpcParts {
    /// The generation backend.
    pub backend: Box<dyn Backend>,
    /// The embedder used for retrieval.
    pub embedder: Box<dyn Embedder>,
    /// The lore.
    pub corpus: Corpus,
    /// The speakers that may be asked questions.
    pub speakers: Vec<Speaker>,
    /// Prompt assembly options.
    pub options: ContextOptions,
    /// The time source for all measurements and the deadline.
    pub clock: Box<dyn Clock>,
    /// The video-memory probe.
    pub probe: Box<dyn MemoryProbe>,
}

/// A set of speakers over one lore corpus, backed by one model.
pub struct Npc {
    backend: Box<dyn Backend>,
    embedder: Box<dyn Embedder>,
    retriever: Retriever,
    speakers: Vec<Speaker>,
    options: ContextOptions,
    clock: Box<dyn Clock>,
    probe: Box<dyn MemoryProbe>,
}

impl Npc {
    /// Builds an NPC, embedding the corpus's beliefs once.
    ///
    /// # Errors
    ///
    /// [`NpcError::DuplicateSpeaker`] when two speakers share an id, and
    /// [`NpcError::Embed`] when embedding the corpus fails.
    pub fn new(parts: NpcParts) -> Result<Npc, NpcError> {
        let NpcParts {
            backend,
            mut embedder,
            corpus,
            speakers,
            options,
            clock,
            probe,
        } = parts;
        for (i, s) in speakers.iter().enumerate() {
            if speakers[..i].iter().any(|p| p.id == s.id) {
                return Err(NpcError::DuplicateSpeaker(s.id.clone()));
            }
        }
        let retriever = Retriever::build(corpus, embedder.as_mut())?;
        Ok(Npc {
            backend,
            embedder,
            retriever,
            speakers,
            options,
            clock,
            probe,
        })
    }

    /// Answers one question, stopping generation at the deadline or the token cap.
    ///
    /// Hitting a stop reason other than [`StopReason::Finished`] is not an
    /// error: the response carries `Err(ReplyError::Unfinished(_))` and the
    /// metrics of what was generated.
    ///
    /// # Errors
    ///
    /// [`NpcError::UnknownSpeaker`] for an unsupplied speaker,
    /// [`NpcError::Embed`] when retrieval fails and [`NpcError::Backend`] when
    /// the backend fails (a full context window excepted).
    pub fn answer(&mut self, request: &NpcRequest) -> Result<NpcResponse, NpcError> {
        let t0 = self.clock.now();
        let mut vram_peak = self.probe.vram_bytes();

        let speaker = self
            .speakers
            .iter()
            .find(|s| s.id == request.speaker)
            .ok_or_else(|| NpcError::UnknownSpeaker(request.speaker.clone()))?;
        let ids = self.retriever.select(
            speaker,
            &request.question,
            request.scope,
            self.embedder.as_mut(),
        )?;
        let retrieval = self.clock.now().duration_since(t0);
        let ctx = assemble(
            self.retriever.corpus(),
            speaker,
            &request.question,
            &ids,
            &self.options,
        );

        let mut decoder = self.backend.begin(&GenerationRequest {
            prompt: &ctx.prompt,
            schema: &ctx.schema,
            thinking: request.thinking,
        })?;
        vram_peak = vram_peak.max(self.probe.vram_bytes());

        let mut raw = String::new();
        let mut thinking = String::new();
        let mut thinking_tokens = 0_u32;
        let mut answer_tokens = 0_u32;
        let mut first_token = None;
        let mut last_token = None;
        let stop = loop {
            if self.clock.now().duration_since(t0) >= request.deadline {
                break StopReason::Deadline;
            }
            if answer_tokens >= ANSWER_TOKEN_CAP {
                break StopReason::TokenCap;
            }
            match decoder.next() {
                Ok(None) => break StopReason::Finished,
                Err(BackendError::ContextFull) => break StopReason::ContextFull,
                Err(e) => return Err(NpcError::Backend(e)),
                Ok(Some(piece)) => {
                    let at = self.clock.now();
                    first_token.get_or_insert(at);
                    last_token = Some(at);
                    match piece.phase {
                        Phase::Thinking => {
                            thinking_tokens += 1;
                            thinking.push_str(&piece.text);
                        }
                        Phase::Answer => {
                            answer_tokens += 1;
                            raw.push_str(&piece.text);
                        }
                    }
                }
            }
        };
        let prompt_tokens = decoder.prompt_tokens();
        drop(decoder);
        vram_peak = vram_peak.max(self.probe.vram_bytes());
        let total = self.clock.now().duration_since(t0);

        let generated = thinking_tokens + answer_tokens;
        let tokens_per_second = match (first_token, last_token) {
            (Some(first), Some(last)) if generated >= 2 => {
                let span = last.duration_since(first).as_secs_f64();
                if span > 0.0 {
                    // Precision loss is irrelevant for a rate shown to humans.
                    ((f64::from(generated) - 1.0) / span) as f32
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

        let reply = if stop == StopReason::Finished {
            parse_reply(&raw, &ctx.lore_ids)
        } else {
            Err(ReplyError::Unfinished(stop))
        };
        let cited_ids = reply
            .as_ref()
            .map(|r| r.sources.clone())
            .unwrap_or_default();

        Ok(NpcResponse {
            reply,
            raw,
            thinking,
            metrics: Metrics {
                prompt_tokens,
                thinking_tokens,
                answer_tokens,
                retrieval,
                time_to_first_token: first_token.map(|t| t.duration_since(t0)),
                tokens_per_second,
                total,
                vram_peak_bytes: vram_peak,
                context_ids: ctx.lore_ids,
                cited_ids,
                stop,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Piece;
    use crate::test_support::mini;
    use crate::testkit::{FakeClock, FakeEmbedder, FakeProbe, Script, ScriptedBackend};

    const MS: Duration = Duration::from_millis(1);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn piece(text: &str, phase: Phase) -> Piece {
        Piece {
            text: text.to_owned(),
            phase,
        }
    }

    fn answer_pieces(n: usize) -> Vec<Piece> {
        (0..n).map(|_| piece("x", Phase::Answer)).collect()
    }

    /// Spells `text` as one answer piece per character.
    fn spelled(text: &str) -> Vec<Piece> {
        text.chars()
            .map(|c| piece(&c.to_string(), Phase::Answer))
            .collect()
    }

    fn script(prefill: Duration, per_token: Duration, pieces: Vec<Piece>) -> Script {
        Script {
            prompt_tokens: 42,
            prefill,
            per_token,
            pieces,
        }
    }

    struct Rig {
        npc: Npc,
        backend: ScriptedBackend,
    }

    fn rig_with(scripts: Vec<Script>, probe: FakeProbe, speakers: Vec<Speaker>) -> Rig {
        let clock = FakeClock::new();
        let backend = ScriptedBackend::new(clock.clone());
        for s in scripts {
            backend.push(s);
        }
        let (corpus, _) = mini();
        let npc = Npc::new(NpcParts {
            backend: Box::new(backend.handle()),
            embedder: Box::new(FakeEmbedder::default()),
            corpus,
            speakers,
            options: ContextOptions::default(),
            clock: Box::new(clock),
            probe: Box::new(probe),
        })
        .expect("rig builds");
        Rig { npc, backend }
    }

    fn rig(scripts: Vec<Script>) -> Rig {
        let (_, speaker) = mini();
        rig_with(scripts, FakeProbe::new(vec![]), vec![speaker])
    }

    fn request(deadline: Duration) -> NpcRequest {
        NpcRequest {
            speaker: RecordId::new("n"),
            question: "who?".into(),
            scope: LoreScope::AllVisible,
            thinking: Thinking::On,
            deadline,
        }
    }

    const GOOD: &str = r#"{"sources":["B-1"],"reply":"Aye."}"#;

    #[test]
    fn answer_returns_parsed_reply_and_cited_ids() {
        let mut r = rig(vec![script(MS, MS, spelled(GOOD))]);
        let resp = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        assert_eq!(resp.reply.as_ref().unwrap().text, "Aye.");
        assert_eq!(resp.metrics.cited_ids, vec![RecordId::new("B-1")]);
        assert_eq!(resp.metrics.stop, StopReason::Finished);
        assert_eq!(resp.raw, GOOD);
        assert_eq!(resp.metrics.prompt_tokens, 42);
    }

    #[test]
    fn answer_records_context_ids_from_retrieval() {
        let mut r = rig(vec![script(MS, MS, spelled(GOOD))]);
        let resp = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        let (corpus, speaker) = mini();
        let expected = assemble(
            &corpus,
            &speaker,
            "who?",
            &Retriever::build(corpus.clone(), &mut FakeEmbedder::default())
                .unwrap()
                .select(
                    &speaker,
                    "who?",
                    LoreScope::AllVisible,
                    &mut FakeEmbedder::default(),
                )
                .unwrap(),
            &ContextOptions::default(),
        );
        assert!(!expected.lore_ids.is_empty());
        assert_eq!(resp.metrics.context_ids, expected.lore_ids);
    }

    #[test]
    fn answer_passes_prompt_schema_and_thinking_to_backend() {
        let mut r = rig(vec![script(MS, MS, spelled(GOOD))]);
        let resp = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        let requests = r.backend.requests();
        assert_eq!(requests.len(), 1);
        let (prompt, schema, thinking) = &requests[0];
        assert_eq!(*thinking, Thinking::On);
        assert_eq!(prompt.user, "who?");
        let listed = schema["properties"]["sources"]["items"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(listed.len(), resp.metrics.context_ids.len());
        for id in &resp.metrics.context_ids {
            assert!(listed.iter().any(|v| v == id.as_str()));
        }
    }

    #[test]
    fn answer_measures_time_to_first_token_total_and_tokens_per_second() {
        let mut r = rig(vec![script(ms(200), ms(10), answer_pieces(11))]);
        let m = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap().metrics;
        assert_eq!(m.time_to_first_token, Some(ms(210)));
        assert_eq!(m.total, ms(310));
        assert_eq!(m.tokens_per_second, 100.0);
        assert_eq!(m.answer_tokens, 11);
    }

    #[test]
    fn answer_stops_at_deadline_between_tokens() {
        let mut r = rig(vec![script(ms(100), ms(20), answer_pieces(400))]);
        let resp = r.npc.answer(&request(Duration::from_secs(3))).unwrap();
        assert_eq!(resp.metrics.stop, StopReason::Deadline);
        assert_eq!(resp.metrics.answer_tokens, 145);
        assert_eq!(
            resp.reply,
            Err(ReplyError::Unfinished(StopReason::Deadline))
        );
        assert_eq!(resp.metrics.total, Duration::from_secs(3));
        assert!(resp.metrics.cited_ids.is_empty());
    }

    #[test]
    fn deadline_passed_during_prefill_generates_no_tokens() {
        let mut r = rig(vec![script(ms(3500), ms(10), answer_pieces(5))]);
        let m = r
            .npc
            .answer(&request(Duration::from_secs(3)))
            .unwrap()
            .metrics;
        assert_eq!(m.stop, StopReason::Deadline);
        assert_eq!(m.answer_tokens, 0);
        assert_eq!(m.time_to_first_token, None);
        assert_eq!(m.tokens_per_second, 0.0);
    }

    #[test]
    fn single_token_reports_zero_tokens_per_second() {
        let mut r = rig(vec![script(ms(50), ms(10), answer_pieces(1))]);
        let m = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap().metrics;
        assert_eq!(m.answer_tokens, 1);
        assert_eq!(m.time_to_first_token, Some(ms(60)));
        assert_eq!(m.tokens_per_second, 0.0);
    }

    #[test]
    fn answer_stops_at_token_cap() {
        let mut r = rig(vec![script(ms(1), ms(1), answer_pieces(300))]);
        let resp = r.npc.answer(&request(Duration::from_secs(60))).unwrap();
        assert_eq!(resp.metrics.answer_tokens, ANSWER_TOKEN_CAP);
        assert_eq!(resp.metrics.stop, StopReason::TokenCap);
        assert_eq!(
            resp.reply,
            Err(ReplyError::Unfinished(StopReason::TokenCap))
        );
    }

    #[test]
    fn answer_counts_thinking_and_answer_tokens_separately_and_parses_only_answer() {
        let mut pieces: Vec<Piece> = (0..5).map(|_| piece("hm ", Phase::Thinking)).collect();
        pieces.extend(spelled(GOOD));
        let mut r = rig(vec![script(MS, MS, pieces)]);
        let resp = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        assert_eq!(resp.metrics.thinking_tokens, 5);
        assert_eq!(resp.metrics.answer_tokens as usize, GOOD.chars().count());
        assert_eq!(resp.thinking, "hm hm hm hm hm ");
        assert_eq!(resp.raw, GOOD);
        assert_eq!(resp.reply.unwrap().text, "Aye.");
    }

    #[test]
    fn thinking_tokens_do_not_count_toward_the_answer_cap() {
        let mut pieces: Vec<Piece> = (0..300).map(|_| piece("h", Phase::Thinking)).collect();
        pieces.extend(spelled(GOOD));
        let mut r = rig(vec![script(MS, MS, pieces)]);
        let resp = r.npc.answer(&request(Duration::from_secs(60))).unwrap();
        assert_eq!(resp.metrics.stop, StopReason::Finished);
        assert_eq!(resp.metrics.thinking_tokens, 300);
    }

    /// A backend whose decoder yields one token, then reports a full context.
    struct FullBackend;
    struct FullDecoder(bool);

    impl crate::backend::Decoder for FullDecoder {
        fn prompt_tokens(&self) -> u32 {
            7
        }
        fn next(&mut self) -> Result<Option<Piece>, BackendError> {
            if self.0 {
                Err(BackendError::ContextFull)
            } else {
                self.0 = true;
                Ok(Some(piece("x", Phase::Answer)))
            }
        }
    }

    impl Backend for FullBackend {
        fn begin<'a>(
            &'a mut self,
            _: &GenerationRequest<'_>,
        ) -> Result<Box<dyn crate::backend::Decoder + 'a>, BackendError> {
            Ok(Box::new(FullDecoder(false)))
        }
    }

    #[test]
    fn context_full_stops_with_reason_not_error() {
        let (corpus, speaker) = mini();
        let mut npc = Npc::new(NpcParts {
            backend: Box::new(FullBackend),
            embedder: Box::new(FakeEmbedder::default()),
            corpus,
            speakers: vec![speaker],
            options: ContextOptions::default(),
            clock: Box::new(FakeClock::new()),
            probe: Box::new(FakeProbe::new(vec![])),
        })
        .unwrap();
        let resp = npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        assert_eq!(resp.metrics.stop, StopReason::ContextFull);
        assert_eq!(resp.metrics.answer_tokens, 1);
        assert_eq!(
            resp.reply,
            Err(ReplyError::Unfinished(StopReason::ContextFull))
        );
    }

    #[test]
    fn backend_failure_is_an_npc_error() {
        let mut r = rig(vec![]);
        let err = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap_err();
        assert_eq!(
            err,
            NpcError::Backend(BackendError::Failed("no script".into()))
        );
    }

    #[test]
    fn answer_keeps_highest_vram_sample() {
        let (_, speaker) = mini();
        let mut r = rig_with(
            vec![script(MS, MS, spelled(GOOD))],
            FakeProbe::new(vec![1, 5, 3]),
            vec![speaker],
        );
        let m = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap().metrics;
        assert_eq!(m.vram_peak_bytes, Some(5));
    }

    #[test]
    fn answer_without_probe_samples_reports_no_vram() {
        let mut r = rig(vec![script(MS, MS, spelled(GOOD))]);
        let m = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap().metrics;
        assert_eq!(m.vram_peak_bytes, None);
    }

    #[test]
    fn answer_errors_on_unknown_speaker() {
        let mut r = rig(vec![script(MS, MS, spelled(GOOD))]);
        let mut req = request(DEFAULT_DEADLINE);
        req.speaker = RecordId::new("ghost");
        assert_eq!(
            r.npc.answer(&req).unwrap_err(),
            NpcError::UnknownSpeaker(RecordId::new("ghost"))
        );
    }

    #[test]
    fn new_errors_on_duplicate_speaker() {
        let (corpus, speaker) = mini();
        let result = Npc::new(NpcParts {
            backend: Box::new(ScriptedBackend::new(FakeClock::new())),
            embedder: Box::new(FakeEmbedder::default()),
            corpus,
            speakers: vec![speaker.clone(), speaker],
            options: ContextOptions::default(),
            clock: Box::new(FakeClock::new()),
            probe: Box::new(FakeProbe::new(vec![])),
        });
        assert!(matches!(
            result,
            Err(NpcError::DuplicateSpeaker(id)) if id == RecordId::new("n")
        ));
    }

    #[test]
    fn unparsable_reply_is_a_reply_error_not_an_npc_error() {
        let mut r = rig(vec![script(MS, MS, spelled("not json"))]);
        let resp = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        assert!(matches!(resp.reply, Err(ReplyError::Malformed(_))));
        assert_eq!(resp.metrics.stop, StopReason::Finished);
        assert!(resp.metrics.cited_ids.is_empty());
    }

    #[test]
    fn npc_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NpcError>();
    }
}
