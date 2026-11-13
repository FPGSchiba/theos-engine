//! The request driver: runs one NPC answer end to end and measures it.
//!
//! [`Npc::answer`] selects lore, assembles the prompt, drives a [`Decoder`]
//! token by token under a deadline and a token cap, and returns the parsed
//! reply together with the [`Metrics`] of the run.

use std::time::{Duration, Instant};

use serde_json::Value;

use crate::backend::{Backend, BackendError, GenerationRequest, Phase, Thinking};
use crate::clock::Clock;
use crate::context::{ContextOptions, Prompt, assemble};
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
    /// Time from the start of the request to the first answer-phase token, if
    /// any came. With thinking on this is what a player perceives as the reply
    /// starting.
    pub time_to_first_answer_token: Option<Duration>,
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

/// One prompt to run through the model as it stands, with no retrieval and no
/// reply parsing.
#[derive(Clone, Debug)]
pub struct GenerateJob {
    /// The prompt to complete.
    pub prompt: Prompt,
    /// JSON schema the output must satisfy; `None` leaves it unconstrained.
    pub schema: Option<Value>,
    /// Whether the model may think first.
    pub thinking: Thinking,
    /// The most thinking tokens before the backend must close the thinking
    /// block itself; `None` is no limit.
    pub thinking_budget: Option<u32>,
    /// Time allowed from the start of the call until generation is cut off.
    pub deadline: Duration,
    /// Most answer-phase tokens to generate.
    pub token_cap: u32,
}

/// The outcome of one [`Npc::generate`] call.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerateResponse {
    /// The answer-phase text exactly as generated.
    pub raw: String,
    /// The thinking-phase text exactly as generated.
    pub thinking: String,
    /// What the run measured; lore ids are empty and retrieval is zero.
    pub metrics: Metrics,
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
    /// The worker thread ended before it finished starting up, or panicked
    /// while answering this request (after which it stops).
    #[error("the NPC worker thread ended before it could answer")]
    WorkerLost,
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
        let vram_start = self.probe.vram_bytes();

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

        let out = self.run_loop(
            &ctx.prompt,
            Some(&ctx.schema),
            request.thinking,
            None,
            request.deadline,
            ANSWER_TOKEN_CAP,
            t0,
            vram_start,
        )?;

        let reply = if out.stop == StopReason::Finished {
            parse_reply(&out.raw, &ctx.lore_ids)
        } else {
            Err(ReplyError::Unfinished(out.stop))
        };
        let cited_ids = reply
            .as_ref()
            .map(|r| r.sources.clone())
            .unwrap_or_default();

        let mut metrics = out.metrics(retrieval);
        metrics.context_ids = ctx.lore_ids;
        metrics.cited_ids = cited_ids;
        Ok(NpcResponse {
            reply,
            raw: out.raw,
            thinking: out.thinking,
            metrics,
        })
    }

    /// Runs one prompt through the model with no retrieval and no parsing.
    ///
    /// Generation stops at the job's deadline or token cap; the response says
    /// why in `metrics.stop`. `metrics.context_ids` and `cited_ids` are empty
    /// and `metrics.retrieval` is zero.
    ///
    /// # Errors
    ///
    /// [`NpcError::Backend`] when the backend fails (a full context window
    /// excepted).
    pub fn generate(&mut self, job: &GenerateJob) -> Result<GenerateResponse, NpcError> {
        let t0 = self.clock.now();
        let vram_start = self.probe.vram_bytes();
        let out = self.run_loop(
            &job.prompt,
            job.schema.as_ref(),
            job.thinking,
            job.thinking_budget,
            job.deadline,
            job.token_cap,
            t0,
            vram_start,
        )?;
        let metrics = out.metrics(Duration::ZERO);
        Ok(GenerateResponse {
            raw: out.raw,
            thinking: out.thinking,
            metrics,
        })
    }

    /// Ranks the lore a speaker may be shown for `query`, as
    /// [`Retriever::select`] does.
    ///
    /// # Errors
    ///
    /// [`NpcError::UnknownSpeaker`] for an unsupplied speaker and
    /// [`NpcError::Embed`] when embedding the query fails.
    pub fn select(
        &mut self,
        speaker: &RecordId,
        query: &str,
        scope: LoreScope,
    ) -> Result<Vec<RecordId>, NpcError> {
        let speaker = self
            .speakers
            .iter()
            .find(|s| &s.id == speaker)
            .ok_or_else(|| NpcError::UnknownSpeaker(speaker.clone()))?;
        Ok(self
            .retriever
            .select(speaker, query, scope, self.embedder.as_mut())?)
    }

    /// Begins a generation and drives it token by token until it finishes, the
    /// deadline passes or the answer-token cap is reached.
    #[allow(clippy::too_many_arguments)]
    fn run_loop(
        &mut self,
        prompt: &Prompt,
        schema: Option<&Value>,
        thinking: Thinking,
        thinking_budget: Option<u32>,
        deadline: Duration,
        token_cap: u32,
        t0: Instant,
        mut vram_peak: Option<u64>,
    ) -> Result<LoopOutcome, NpcError> {
        let mut decoder = self.backend.begin(&GenerationRequest {
            prompt,
            schema,
            thinking,
            thinking_budget,
        })?;
        vram_peak = vram_peak.max(self.probe.vram_bytes());

        let mut raw = String::new();
        let mut thinking_text = String::new();
        let mut thinking_tokens = 0_u32;
        let mut answer_tokens = 0_u32;
        let mut first_token = None;
        let mut first_answer_token = None;
        let mut last_token = None;
        let stop = loop {
            if self.clock.now().duration_since(t0) >= deadline {
                break StopReason::Deadline;
            }
            if answer_tokens >= token_cap {
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
                            thinking_text.push_str(&piece.text);
                        }
                        Phase::Answer => {
                            answer_tokens += 1;
                            first_answer_token.get_or_insert(at);
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

        Ok(LoopOutcome {
            raw,
            thinking: thinking_text,
            stop,
            prompt_tokens,
            thinking_tokens,
            answer_tokens,
            time_to_first_token: first_token.map(|t| t.duration_since(t0)),
            time_to_first_answer_token: first_answer_token.map(|t| t.duration_since(t0)),
            tokens_per_second,
            total,
            vram_peak_bytes: vram_peak,
        })
    }
}

/// What [`Npc::run_loop`] produced.
struct LoopOutcome {
    raw: String,
    thinking: String,
    stop: StopReason,
    prompt_tokens: u32,
    thinking_tokens: u32,
    answer_tokens: u32,
    time_to_first_token: Option<Duration>,
    time_to_first_answer_token: Option<Duration>,
    tokens_per_second: f32,
    total: Duration,
    vram_peak_bytes: Option<u64>,
}

impl LoopOutcome {
    /// The run's metrics, with no lore ids; the caller fills those in.
    fn metrics(&self, retrieval: Duration) -> Metrics {
        Metrics {
            prompt_tokens: self.prompt_tokens,
            thinking_tokens: self.thinking_tokens,
            answer_tokens: self.answer_tokens,
            retrieval,
            time_to_first_token: self.time_to_first_token,
            time_to_first_answer_token: self.time_to_first_answer_token,
            tokens_per_second: self.tokens_per_second,
            total: self.total,
            vram_peak_bytes: self.vram_peak_bytes,
            context_ids: Vec::new(),
            cited_ids: Vec::new(),
            stop: self.stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Piece;
    use crate::context::Prompt;
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
        let sent = &requests[0];
        assert_eq!(sent.thinking, Thinking::On);
        assert_eq!(sent.prompt.user, "who?");
        let schema = sent.schema.as_ref().unwrap();
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
    fn time_to_first_answer_token_skips_the_thinking_phase() {
        let mut pieces: Vec<Piece> = (0..5).map(|_| piece("hm ", Phase::Thinking)).collect();
        pieces.extend(answer_pieces(3));
        let mut r = rig(vec![script(ms(200), ms(10), pieces)]);
        let m = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap().metrics;
        assert_eq!(m.time_to_first_token, Some(ms(210)));
        assert_eq!(m.time_to_first_answer_token, Some(ms(260)));
    }

    #[test]
    fn time_to_first_answer_token_is_none_when_only_thinking_came() {
        let pieces: Vec<Piece> = (0..5).map(|_| piece("hm ", Phase::Thinking)).collect();
        let mut r = rig(vec![script(ms(200), ms(10), pieces)]);
        let m = r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap().metrics;
        assert!(m.time_to_first_token.is_some());
        assert_eq!(m.time_to_first_answer_token, None);
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

    fn job(pieces: Vec<Piece>) -> (Rig, GenerateJob) {
        let r = rig(vec![script(MS, MS, pieces)]);
        let job = GenerateJob {
            prompt: Prompt {
                system: "sys".into(),
                user: "usr".into(),
            },
            schema: None,
            thinking: Thinking::Off,
            thinking_budget: None,
            deadline: DEFAULT_DEADLINE,
            token_cap: ANSWER_TOKEN_CAP,
        };
        (r, job)
    }

    #[test]
    fn generate_runs_a_prompt_without_retrieval_or_parsing() {
        let (mut r, j) = job(spelled("free text"));
        let resp = r.npc.generate(&j).unwrap();
        assert_eq!(resp.raw, "free text");
        assert!(resp.metrics.context_ids.is_empty());
        assert!(resp.metrics.cited_ids.is_empty());
        assert_eq!(resp.metrics.retrieval, Duration::ZERO);
        assert_eq!(resp.metrics.stop, StopReason::Finished);
        assert_eq!(resp.metrics.prompt_tokens, 42);
    }

    #[test]
    fn generate_passes_none_schema_to_the_backend() {
        let (mut r, j) = job(spelled("x"));
        r.npc.generate(&j).unwrap();
        let requests = r.backend.requests();
        assert_eq!(requests[0].schema, None);
        assert_eq!(requests[0].prompt.system, "sys");
    }

    #[test]
    fn generate_passes_the_thinking_budget_to_the_backend() {
        let (mut r, mut j) = job(spelled("x"));
        j.thinking = Thinking::On;
        j.thinking_budget = Some(77);
        j.schema = Some(serde_json::json!({"type": "string"}));
        r.npc.generate(&j).unwrap();
        let requests = r.backend.requests();
        assert_eq!(requests[0].thinking_budget, Some(77));
        assert_eq!(requests[0].thinking, Thinking::On);
        assert_eq!(
            requests[0].schema,
            Some(serde_json::json!({"type": "string"}))
        );
    }

    #[test]
    fn answer_passes_no_thinking_budget() {
        let mut r = rig(vec![script(MS, MS, spelled(GOOD))]);
        r.npc.answer(&request(DEFAULT_DEADLINE)).unwrap();
        assert_eq!(r.backend.requests()[0].thinking_budget, None);
    }

    #[test]
    fn generate_honours_its_token_cap() {
        let (mut r, mut j) = job(answer_pieces(30));
        j.token_cap = 10;
        let resp = r.npc.generate(&j).unwrap();
        assert_eq!(resp.metrics.stop, StopReason::TokenCap);
        assert_eq!(resp.metrics.answer_tokens, 10);
    }

    #[test]
    fn generate_stops_at_the_deadline() {
        let mut r = rig(vec![script(ms(100), ms(20), answer_pieces(400))]);
        let j = GenerateJob {
            prompt: Prompt {
                system: String::new(),
                user: String::new(),
            },
            schema: None,
            thinking: Thinking::Off,
            thinking_budget: None,
            deadline: Duration::from_secs(3),
            token_cap: 1000,
        };
        let resp = r.npc.generate(&j).unwrap();
        assert_eq!(resp.metrics.stop, StopReason::Deadline);
        assert_eq!(resp.metrics.answer_tokens, 145);
        assert_eq!(resp.metrics.total, Duration::from_secs(3));
    }

    #[test]
    fn select_returns_the_filtered_ranked_ids() {
        let (corpus, speaker) = mini();
        let retriever = Retriever::build(corpus, &mut FakeEmbedder::default()).unwrap();
        let mut r = rig(vec![]);
        for scope in [
            LoreScope::None,
            LoreScope::AllVisible,
            LoreScope::Retrieved { top_k: 2 },
        ] {
            let expected = retriever
                .select(&speaker, "who?", scope, &mut FakeEmbedder::default())
                .unwrap();
            let got = r.npc.select(&speaker.id, "who?", scope).unwrap();
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn select_rejects_an_unknown_speaker() {
        let mut r = rig(vec![]);
        assert_eq!(
            r.npc
                .select(&RecordId::new("ghost"), "who?", LoreScope::AllVisible)
                .unwrap_err(),
            NpcError::UnknownSpeaker(RecordId::new("ghost"))
        );
    }

    #[test]
    fn npc_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<NpcError>();
    }
}
