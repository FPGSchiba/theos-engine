//! The generator: a llama.cpp model behind `theos_ai::Backend`.
//!
//! `begin` renders the prompt through the model's chat template, appends the
//! profile's thinking prefix, prefills it in one batch and returns a decoder.
//! Each `next` first decodes the token sampled by the previous call (so time to
//! first token covers prefill and one sample, not an extra decode), then
//! samples one token.
//!
//! Thinking off: the llguidance JSON-schema sampler is in the chain from the
//! first token. Thinking on: the chain starts without it; once the thinking
//! text ends in `</think>`, the `"\n\n"` separator is decoded without being
//! emitted and the chain is rebuilt with a fresh llguidance matcher.

use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::{LlamaChatMessage, LlamaChatTemplate};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::token_type::LlamaTokenAttr;
use llama_cpp_2::vocab::LlamaVocab;
use llama_cpp_2::DecodeError;
use llguidance::api::TopLevelGrammar;
use llguidance::{Matcher, ParserFactory};
use serde::Deserialize;
use serde_json::Value;
use theos_ai::{
    Backend, BackendError, Decoder, GenerationRequest, Phase, Piece, Segment, Thinking, Trust,
};

use crate::{LlamaError, LlamaRuntime, ModelContext};

/// Context size, and the largest batch a prompt is prefilled in, in tokens,
/// when the profile does not set `n_ctx`.
pub const N_CTX: u32 = 8192;

/// How many recent tokens the presence penalty looks at.
const PENALTY_LAST_N: i32 = 64;

/// Separates the closed thinking block from the answer.
const ANSWER_SEPARATOR: &str = "\n\n";

/// Sampling settings for one thinking mode.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Sampling {
    /// Softmax temperature.
    pub temperature: f32,
    /// Nucleus cut-off.
    pub top_p: f32,
    /// Number of candidates kept before top-p.
    pub top_k: i32,
    /// Presence penalty over the last 64 tokens.
    pub presence_penalty: f32,
}

/// How to prompt and sample one model, read from `models.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ModelProfile {
    /// Appended after the chat template when thinking is off.
    pub thinking_off_prefix: String,
    /// Appended after the chat template when thinking is on.
    pub thinking_on_prefix: String,
    /// Sampling with thinking off.
    pub sampling_off: Sampling,
    /// Sampling with thinking on.
    pub sampling_on: Sampling,
    /// Sampling for the answer phase after `</think>` when thinking is on;
    /// `None` keeps `sampling_on` for the whole generation.
    #[serde(default)]
    pub answer_sampling_on: Option<Sampling>,
    /// Context size in tokens; `None` means [`N_CTX`].
    #[serde(default)]
    pub n_ctx: Option<u32>,
    /// Seed of the final `dist` sampler.
    pub seed: u32,
    /// G1: keep turn markers a player types as text. The prompt is tokenized
    /// canonically (byte-identical to G1 off) unless an untrusted segment
    /// carries special-token text (it tokenizes differently with special
    /// parsing on, the prompt holds more control or user-defined special tokens
    /// than its skeleton, or it contains the text of a user-defined token);
    /// only then the prompt is spliced, with untrusted parts tokenized with
    /// special parsing off, and token boundaries around untrusted parts may
    /// differ from the canonical tokenization. Off keeps the old behaviour.
    ///
    /// G1 covers CONTROL special tokens (the chat template's turn markers) and
    /// USER_DEFINED special tokens of the vocabulary (for Qwen3.5: `<think>`,
    /// `</think>`, `<tool_call>`, `</tool_call>`, `<tool_response>`,
    /// `</tool_response>`). llama.cpp parses the latter even with special parsing
    /// off, so on the splice path each occurrence of such a token's text in an
    /// untrusted part is cut in two halves that are tokenized apart; the text is
    /// kept, only the special token id is not produced. The list is read from the
    /// vocabulary at load.
    #[serde(default)]
    pub safe_untrusted: bool,
}

/// One `[[model]]` entry of `models.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ModelEntry {
    /// Display name for the results tables.
    pub name: String,
    /// GGUF file name, relative to the models directory.
    pub file: String,
    /// Sampling with thinking off for behaviour-tree output.
    #[serde(default)]
    pub tree_sampling_off: Option<Sampling>,
    /// Sampling for the tree answer with thinking on.
    #[serde(default)]
    pub tree_sampling_on: Option<Sampling>,
    /// Prompting and sampling.
    #[serde(flatten)]
    pub profile: ModelProfile,
}

/// The whole of `models.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ModelsFile {
    /// GGUF file name of the embedding model.
    pub embedder: String,
    /// Generators to run, in order.
    #[serde(rename = "model")]
    pub models: Vec<ModelEntry>,
}

impl ModelsFile {
    /// Parses the text of a `models.toml`.
    ///
    /// # Errors
    ///
    /// [`LlamaError::Config`] when the text is not a valid models file.
    pub fn parse(text: &str) -> Result<Self, LlamaError> {
        toml::from_str(text).map_err(|e| LlamaError::Config(e.to_string()))
    }

    /// Reads and parses a `models.toml`.
    ///
    /// # Errors
    ///
    /// [`LlamaError::Config`] when the file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self, LlamaError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| LlamaError::Config(format!("{}: {e}", path.display())))?;
        Self::parse(&text)
    }
}

impl ModelProfile {
    /// The sampling settings for `thinking`.
    pub fn sampling(&self, thinking: Thinking) -> Sampling {
        match thinking {
            Thinking::Off => self.sampling_off,
            Thinking::On => self.sampling_on,
        }
    }
}

/// A llama.cpp generator with all layers on the GPU and the profile's context size.
pub struct LlamaGenerator {
    mc: ModelContext,
    batch: LlamaBatch<'static>,
    factory: ParserFactory,
    template: LlamaChatTemplate,
    separator: Vec<LlamaToken>,
    think_end: Vec<LlamaToken>,
    profile: ModelProfile,
    /// The text of every USER_DEFINED special token of the vocabulary, for G1.
    user_defined: Vec<String>,
}

impl LlamaGenerator {
    /// Loads the GGUF at `path` and prepares its context and grammar machinery.
    ///
    /// # Errors
    ///
    /// [`LlamaError`] when the file is missing or cannot be loaded, the model
    /// has no chat template, or the context cannot be created.
    pub fn load(
        rt: Arc<LlamaRuntime>,
        path: &Path,
        profile: ModelProfile,
    ) -> Result<LlamaGenerator, LlamaError> {
        let model = rt.load_model(path)?;
        let template = model
            .chat_template(None)
            .map_err(|e| LlamaError::Model(format!("chat template: {e}")))?;
        let separator = model
            .vocab()
            .tokenize(ANSWER_SEPARATOR.as_bytes(), false, false);
        if separator.is_empty() {
            return Err(LlamaError::Model(
                "the answer separator tokenizes to nothing".into(),
            ));
        }
        // Special parsing on: `</think>` is one (user-defined) special token for Qwen3.5.
        let think_end = model.vocab().tokenize(THINK_END.as_bytes(), false, true);
        if think_end.is_empty() {
            return Err(LlamaError::Model("`</think>` tokenizes to nothing".into()));
        }
        let user_defined = user_defined_tokens(&model.vocab());
        // Built once per model: walking the vocabulary takes a while.
        let env = LlamaSampler::llguidance_tok_env(&model);
        let factory =
            ParserFactory::new_simple(&env).map_err(|e| LlamaError::Grammar(e.to_string()))?;
        let n_ctx = profile.n_ctx.unwrap_or(N_CTX);
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(n_ctx))
            .with_n_batch(n_ctx);
        let mc = ModelContext::new(rt, model, params)?;
        Ok(LlamaGenerator {
            mc,
            batch: LlamaBatch::new(n_ctx as usize, 1),
            factory,
            template,
            separator,
            think_end,
            profile,
            user_defined,
        })
    }

    /// The text of every USER_DEFINED special token of the vocabulary, the ones G1
    /// neutralises on top of the CONTROL tokens.
    pub fn user_defined_tokens(&self) -> &[String] {
        &self.user_defined
    }

    /// How many tokens `text` becomes, without special tokens added, with
    /// special-token text parsed into control tokens or not. Lets tests show
    /// what the prompt tokenization does with forged turn markers.
    pub fn count_tokens(&self, text: &str, parse_special: bool) -> usize {
        self.mc
            .model()
            .vocab()
            .tokenize(text.as_bytes(), false, parse_special)
            .len()
    }

    /// Number of parameters in the model.
    pub fn n_params(&self) -> u64 {
        self.mc.model().n_params()
    }

    /// Asks llguidance to compile `schema` into a matcher, without generating anything.
    ///
    /// # Errors
    ///
    /// [`BackendError::Failed`] when llguidance rejects the schema.
    pub fn compile_schema(&self, schema: &Value) -> Result<(), BackendError> {
        schema_matcher(&self.factory, schema).map(|_| ())
    }

    /// The profile the generator was loaded with.
    pub fn profile(&self) -> &ModelProfile {
        &self.profile
    }

    /// How `text` tokenizes, without special tokens added.
    pub fn tokenize(&self, text: &str, parse_special: bool) -> Vec<LlamaToken> {
        self.mc
            .model()
            .vocab()
            .tokenize(text.as_bytes(), false, parse_special)
    }

    /// The chat-templated, prefixed prompt text for `request`, with untrusted
    /// text in place (whatever the profile's `safe_untrusted` says).
    ///
    /// # Errors
    ///
    /// [`BackendError::Failed`] when the chat template fails.
    pub fn prompt_text(&self, request: &GenerationRequest<'_>) -> Result<String, BackendError> {
        self.templated(request, false).map(|(text, _)| text)
    }

    /// The prompt token ids `begin` prefills for `request`.
    ///
    /// # Errors
    ///
    /// [`BackendError::Failed`] when the chat template fails, or, on the splice
    /// path only (when G1 is on and a segment is forged), when a placeholder does
    /// not survive the template exactly once. A clean prompt takes the canonical
    /// path and its placeholders are not validated.
    pub fn prompt_tokens(
        &self,
        request: &GenerationRequest<'_>,
    ) -> Result<Vec<LlamaToken>, BackendError> {
        let vocab = self.mc.model().vocab();
        if !self.profile.safe_untrusted {
            let prompt = self.prompt_text(request)?;
            return Ok(vocab.tokenize(prompt.as_bytes(), true, true));
        }
        // Canonical first: the same text and tokens as with G1 off.
        let canonical = self.prompt_text(request)?;
        let canonical_tokens = vocab.tokenize(canonical.as_bytes(), true, true);
        let (templated, placeholders) = self.templated(request, true)?;
        let special: Vec<Vec<LlamaToken>> = placeholders
            .iter()
            .map(|(_, text)| vocab.tokenize(text.as_bytes(), false, true))
            .collect();
        let plain: Vec<Vec<LlamaToken>> = placeholders
            .iter()
            .map(|(_, text)| vocab.tokenize(text.as_bytes(), false, false))
            .collect();
        let specials = |tokens: &[LlamaToken]| {
            tokens
                .iter()
                .filter(|t| {
                    vocab.is_control(**t) || vocab.attr(**t).contains(LlamaTokenAttr::UserDefined)
                })
                .count()
        };
        let skeleton_tokens = vocab.tokenize(templated.as_bytes(), true, true);
        let untrusted: Vec<&str> = placeholders.iter().map(|(_, t)| t.as_str()).collect();
        if !is_forged(
            &special,
            &plain,
            specials(&canonical_tokens),
            specials(&skeleton_tokens),
            &untrusted,
            &self.user_defined,
        ) {
            return Ok(canonical_tokens);
        }
        let mut tokens = Vec::new();
        for (i, (text, parse_special)) in splice_plan(&templated, &placeholders)?.iter().enumerate()
        {
            // BOS only on the very first part; later empty parts add nothing.
            if i > 0 && text.is_empty() {
                continue;
            }
            if *parse_special {
                tokens.extend(vocab.tokenize(text.as_bytes(), i == 0, true));
            } else {
                // No piece of an untrusted part equals a user-defined token's text.
                // Per-piece tokenization relies on a byte-level BPE vocabulary (Qwen); a
                // SentencePiece vocabulary would add a prefix space to every piece.
                for piece in split_user_defined(text, &self.user_defined) {
                    tokens.extend(vocab.tokenize(piece.as_bytes(), false, false));
                }
            }
        }
        Ok(tokens)
    }

    /// The text of `tokens`, special tokens spelled out.
    pub fn detokenize(&self, tokens: &[LlamaToken]) -> String {
        let bytes = self.mc.model().vocab().detokenize(tokens, false, true);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// The templated, prefixed prompt. With `placeholders` on, each untrusted
    /// segment is replaced by `"\u{E000}{i}\u{E001}"` and listed with its text.
    fn templated(
        &self,
        request: &GenerationRequest<'_>,
        placeholders: bool,
    ) -> Result<(String, Vec<(String, String)>), BackendError> {
        let mut found: Vec<(String, String)> = Vec::new();
        let mut content = |segments: &[Segment]| -> String {
            segments
                .iter()
                .map(|seg| match (placeholders, seg.trust) {
                    (true, Trust::Untrusted) => {
                        let ph = format!("\u{E000}{}\u{E001}", found.len());
                        found.push((ph.clone(), seg.text.clone()));
                        ph
                    }
                    _ => seg.text.clone(),
                })
                .collect()
        };
        let system = content(&request.prompt.system);
        let user = content(&request.prompt.user);
        let message = |role: &str, content: &str| {
            LlamaChatMessage::new(role.to_owned(), content.to_owned())
                .map_err(|e| BackendError::Failed(format!("chat message: {e}")))
        };
        let chat = [message("system", &system)?, message("user", &user)?];
        let templated = self
            .mc
            .model()
            .apply_chat_template(&self.template, &chat, true)
            .map_err(|e| BackendError::Failed(format!("chat template: {e}")))?;
        Ok((
            render_prompt(&templated, request.thinking, &self.profile),
            found,
        ))
    }
}

/// Splits `templated` at each placeholder into `(text, parse_special)` parts:
/// engine text `true`, the untrusted text that replaces a placeholder `false`.
/// Parts alternate engine, untrusted, ..., engine (engine parts may be empty).
///
/// # Errors
///
/// [`BackendError::Failed`] when a placeholder is missing, appears more than
/// once, or the placeholders are out of order.
fn splice_plan(
    templated: &str,
    placeholders: &[(String, String)],
) -> Result<Vec<(String, bool)>, BackendError> {
    let mut parts = Vec::with_capacity(placeholders.len() * 2 + 1);
    let mut rest = templated;
    for (placeholder, text) in placeholders {
        match templated.matches(placeholder.as_str()).count() {
            1 => {}
            0 => {
                return Err(BackendError::Failed(format!(
                    "placeholder {placeholder:?} is missing from the templated prompt"
                )))
            }
            n => {
                return Err(BackendError::Failed(format!(
                    "placeholder {placeholder:?} appears {n} times in the templated prompt"
                )))
            }
        }
        let Some((before, after)) = rest.split_once(placeholder.as_str()) else {
            return Err(BackendError::Failed(format!(
                "placeholder {placeholder:?} is out of order in the templated prompt"
            )));
        };
        parts.push((before.to_owned(), true));
        parts.push((text.clone(), false));
        rest = after;
    }
    parts.push((rest.to_owned(), true));
    move_trailing_blanks(&mut parts);
    Ok(parts)
}

/// Whether an untrusted segment carries special-token text: some segment tokenizes
/// differently with special parsing on (`special`) than off (`plain`), or the canonical
/// prompt holds more CONTROL or USER_DEFINED special tokens than the skeleton with
/// placeholders in place of the untrusted texts (`canonical_specials` against
/// `skeleton_specials`; this catches a token split across adjacent segments), or an
/// untrusted text (`untrusted`) contains the text of a user-defined token
/// (`user_defined`). User-defined tokens (Qwen3.5's `</think>` and tool-call tags)
/// tokenize the same with parsing on and off, so the first check cannot see them (see
/// `ModelProfile::safe_untrusted`).
fn is_forged(
    special: &[Vec<LlamaToken>],
    plain: &[Vec<LlamaToken>],
    canonical_specials: usize,
    skeleton_specials: usize,
    untrusted: &[&str],
    user_defined: &[String],
) -> bool {
    special.iter().zip(plain).any(|(s, p)| s != p)
        || canonical_specials > skeleton_specials
        || untrusted
            .iter()
            .any(|u| user_defined.iter().any(|t| u.contains(t.as_str())))
}

/// The text of every USER_DEFINED token of `vocab`, read from the token attributes
/// (`LlamaTokenAttr::UserDefined`). Non-UTF-8 texts are skipped, see
/// [`usable_user_defined`] for the rest.
fn user_defined_tokens(vocab: &LlamaVocab<'_>) -> Vec<String> {
    usable_user_defined(
        (0..vocab.n_tokens())
            .map(LlamaToken::new)
            .filter(|t| vocab.attr(*t).contains(LlamaTokenAttr::UserDefined))
            .filter_map(|t| vocab.text(t).and_then(|c| c.to_str().ok())),
    )
}

/// The token texts G1 can act on: those of at least two characters. A one-character
/// token cannot be cut in two, and keeping it would make `is_forged` flag every prompt
/// that holds that character and give up the canonical (G1-off identical) tokenization
/// for nothing.
fn usable_user_defined<'a>(texts: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    texts
        .into_iter()
        .filter(|s| s.chars().count() >= 2)
        .map(str::to_owned)
        .collect()
}

/// Splits `text` into pieces, none of which contains the text of a token of
/// `user_defined`: every occurrence is cut in the middle (at a character boundary),
/// so `"</think>"` becomes `"</th"` and `"ink>"`. The pieces concatenate to `text`.
/// A token of a single character cannot be cut and is left alone.
fn split_user_defined(text: &str, user_defined: &[String]) -> Vec<String> {
    let mut cuts: Vec<usize> = Vec::new();
    for (start, _) in text.char_indices() {
        for token in user_defined {
            if token.chars().count() >= 2 && text[start..].starts_with(token.as_str()) {
                let half = token.chars().count() / 2;
                let mid = token
                    .char_indices()
                    .nth(half)
                    .map_or(token.len(), |(i, _)| i);
                cuts.push(start + mid);
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut pieces = Vec::with_capacity(cuts.len() + 1);
    let mut from = 0;
    for cut in cuts {
        pieces.push(text[from..cut].to_owned());
        from = cut;
    }
    pieces.push(text[from..].to_owned());
    pieces.retain(|p| !p.is_empty());
    pieces
}

/// Moves the trailing run of horizontal whitespace (spaces and tabs after the last
/// newline, if any) of each engine part that precedes an untrusted part to the start of
/// that untrusted part. Tokenized apart, the space would be a token of its own and the
/// merge of the space with the first word (" Ada") would be lost, so G1 would change
/// ordinary prompts. Whitespace holds no special-token text, so it is safe in an
/// untrusted part.
fn move_trailing_blanks(parts: &mut [(String, bool)]) {
    for i in (1..parts.len()).step_by(2) {
        let engine = &mut parts[i - 1].0;
        let line = engine.rfind('\n').map_or(0, |n| n + 1);
        let blanks = line + engine[line..].trim_end_matches([' ', '\t']).len();
        if blanks < engine.len() {
            let moved = engine.split_off(blanks);
            parts[i].0.insert_str(0, &moved);
        }
    }
}

/// A fresh llguidance matcher for `schema`.
fn schema_matcher(factory: &ParserFactory, schema: &Value) -> Result<Matcher, BackendError> {
    let grammar = TopLevelGrammar::from_json_schema(schema.clone());
    let parser = factory
        .create_parser(grammar)
        .map_err(|e| BackendError::Failed(format!("reply schema: {e}")))?;
    let matcher = Matcher::new(Ok(parser));
    match matcher.get_error() {
        Some(e) => Err(BackendError::Failed(format!("reply schema: {e}"))),
        None => Ok(matcher),
    }
}

/// The sampler chain: `[llguidance?, penalties, top_k, top_p, temp, dist]`.
///
/// llguidance only masks logits; `dist` at the end selects the token.
fn sampler_chain(n_vocab: i32, s: Sampling, seed: u32, grammar: Option<Matcher>) -> LlamaSampler {
    let mut chain = Vec::with_capacity(6);
    if let Some(matcher) = grammar {
        chain.push(LlamaSampler::from(matcher));
    }
    chain.push(LlamaSampler::penalties(
        n_vocab,
        PENALTY_LAST_N,
        1.0,
        0.0,
        s.presence_penalty,
    ));
    chain.push(LlamaSampler::top_k(s.top_k));
    chain.push(LlamaSampler::top_p(s.top_p, 1));
    chain.push(LlamaSampler::temp(s.temperature));
    chain.push(LlamaSampler::dist(seed));
    LlamaSampler::chain_simple(chain)
}

/// Maps a llama.cpp decode failure: no free KV slot means the context is full.
fn decode_error(e: DecodeError) -> BackendError {
    match e {
        DecodeError::NoKvCacheSlot => BackendError::ContextFull,
        other => BackendError::Failed(format!("decode: {other}")),
    }
}

impl Backend for LlamaGenerator {
    fn begin<'a>(
        &'a mut self,
        request: &GenerationRequest<'_>,
    ) -> Result<Box<dyn Decoder + 'a>, BackendError> {
        let tokens = self.prompt_tokens(request)?;
        if tokens.is_empty() {
            return Err(BackendError::Failed("prompt tokenized to nothing".into()));
        }
        let n_ctx = self.mc.n_ctx();
        let n_prompt = u32::try_from(tokens.len()).map_err(|_| BackendError::ContextFull)?;
        if n_prompt >= n_ctx {
            return Err(BackendError::ContextFull);
        }
        // `None`: no grammar anywhere, the chain stays unconstrained.
        let matcher = request
            .schema
            .map(|schema| schema_matcher(&self.factory, schema))
            .transpose()?;

        self.mc.clear();
        self.batch.clear();
        self.batch
            .add_sequence(&tokens, 0, false)
            .map_err(|_| BackendError::ContextFull)?;
        self.mc.decode(&mut self.batch).map_err(decode_error)?;

        let n_vocab = self.mc.model().n_vocab();
        let sampling = self.profile.sampling(request.thinking);
        let seed = self.profile.seed;
        let (sampler, answer_grammar, phase) = match request.thinking {
            Thinking::Off => (
                sampler_chain(n_vocab, sampling, seed, matcher),
                None,
                Phase::Answer,
            ),
            Thinking::On => (
                sampler_chain(n_vocab, sampling, seed, None),
                matcher,
                Phase::Thinking,
            ),
        };
        let answer_sampling = self.profile.answer_sampling_on.unwrap_or(sampling);
        // invariant: 0 < n_prompt < n_ctx (the profile's context size, 8192 by
        // default), so these fit in i32.
        let pos = n_prompt as i32;
        Ok(Box::new(LlamaDecoder {
            mc: &mut self.mc,
            batch: &mut self.batch,
            separator: &self.separator,
            think_end: &self.think_end,
            sampler,
            answer_grammar,
            answer_sampling,
            thinking_budget: request.thinking_budget,
            thinking_pieces: 0,
            seed,
            n_vocab,
            n_ctx,
            n_prompt,
            pos,
            logits_idx: pos - 1,
            pending: Vec::new(),
            phase,
            thinking_text: String::new(),
            utf8: Utf8Stream::default(),
            finished: false,
        }))
    }
}

/// One running generation on a [`LlamaGenerator`].
struct LlamaDecoder<'g> {
    mc: &'g mut ModelContext,
    batch: &'g mut LlamaBatch<'static>,
    separator: &'g [LlamaToken],
    /// The `</think>` token(s), forced in when the thinking budget runs out.
    think_end: &'g [LlamaToken],
    sampler: LlamaSampler,
    /// Thinking on: the schema matcher waiting for the end of the thinking
    /// block; `None` when there is no schema.
    answer_grammar: Option<Matcher>,
    /// Sampling for the answer chain after the switch.
    answer_sampling: Sampling,
    /// Thinking pieces allowed before the decoder closes the block itself.
    thinking_budget: Option<u32>,
    /// Thinking pieces emitted so far.
    thinking_pieces: u32,
    seed: u32,
    n_vocab: i32,
    n_ctx: u32,
    n_prompt: u32,
    /// Tokens in the context.
    pos: i32,
    /// Batch index of the logits to sample from next.
    logits_idx: i32,
    /// Sampled (or forced) tokens not yet decoded into the context.
    pending: Vec<LlamaToken>,
    phase: Phase,
    thinking_text: String,
    utf8: Utf8Stream,
    finished: bool,
}

impl LlamaDecoder<'_> {
    /// Decodes the pending tokens in one batch; logits come from the last one.
    fn decode_pending(&mut self) -> Result<(), BackendError> {
        let n = i32::try_from(self.pending.len()).map_err(|_| BackendError::ContextFull)?;
        if i64::from(self.pos) + i64::from(n) > i64::from(self.n_ctx) {
            return Err(BackendError::ContextFull);
        }
        self.batch.clear();
        for (i, &token) in (0..).zip(&self.pending) {
            self.batch
                .add(token, self.pos + i, &[0], i == n - 1)
                .map_err(|e| BackendError::Failed(format!("batch: {e}")))?;
        }
        self.mc.decode(self.batch).map_err(decode_error)?;
        self.pos += n;
        self.logits_idx = n - 1;
        self.pending.clear();
        Ok(())
    }

    /// Whether the thinking budget is used up without a natural close.
    /// `Some(0)` skips thinking entirely: the block is closed before the first token.
    fn budget_spent(&self) -> bool {
        self.phase == Phase::Thinking
            && self
                .thinking_budget
                .is_some_and(|budget| self.thinking_pieces >= budget)
    }

    /// Closes the thinking phase: queue the separator, switch to the answer chain.
    fn start_answer(&mut self) {
        self.pending.extend_from_slice(self.separator);
        // A fresh matcher: none of the thinking tokens are replayed into it.
        let grammar = self.answer_grammar.take();
        self.sampler = sampler_chain(self.n_vocab, self.answer_sampling, self.seed, grammar);
        self.phase = Phase::Answer;
    }
}

impl Decoder for LlamaDecoder<'_> {
    fn prompt_tokens(&self) -> u32 {
        self.n_prompt
    }

    fn next(&mut self) -> Result<Option<Piece>, BackendError> {
        if self.finished {
            return Ok(None);
        }
        if self.budget_spent() {
            // The model did not close the block in time: close it for it. The
            // forced tokens are decoded, not emitted.
            self.pending.extend_from_slice(self.think_end);
            self.start_answer();
        }
        if !self.pending.is_empty() {
            self.decode_pending()?;
        }
        // `sample` also accepts the token into every sampler of the chain.
        let token = self.mc.sample(&mut self.sampler, self.logits_idx);
        let vocab = self.mc.model().vocab();
        if vocab.is_eog(token) {
            self.finished = true;
            return Ok(None);
        }
        // Special tokens are rendered so a `</think>` control token is seen.
        let text = self.utf8.push(&vocab.token_to_piece(token, true, None));
        self.pending.push(token);
        match self.phase {
            Phase::Answer => Ok(Some(Piece {
                text,
                phase: Phase::Answer,
            })),
            Phase::Thinking => {
                self.thinking_pieces += 1;
                self.thinking_text.push_str(&text);
                if ends_thinking(&self.thinking_text) {
                    self.start_answer();
                }
                Ok(Some(Piece {
                    text,
                    phase: Phase::Thinking,
                }))
            }
        }
    }
}

/// The chat-templated prompt followed by the profile's prefix for `thinking`.
pub fn render_prompt(templated: &str, thinking: Thinking, profile: &ModelProfile) -> String {
    let prefix = match thinking {
        Thinking::Off => &profile.thinking_off_prefix,
        Thinking::On => &profile.thinking_on_prefix,
    };
    format!("{templated}{prefix}")
}

/// Closes the thinking block in the model's output.
const THINK_END: &str = "</think>";

/// Whether the thinking text generated so far closes the thinking block.
///
/// Trailing whitespace is ignored in case the closing tag and a newline share
/// one token.
fn ends_thinking(thinking_text: &str) -> bool {
    thinking_text.trim_end().ends_with(THINK_END)
}

/// Turns a stream of token bytes into text without splitting UTF-8 characters.
///
/// A token may end in the middle of a multi-byte character; those bytes wait
/// for the next token. Invalid bytes become U+FFFD.
#[derive(Debug, Default)]
struct Utf8Stream {
    pending: Vec<u8>,
}

impl Utf8Stream {
    /// Adds one token's bytes and returns the text they complete.
    fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    out.push_str(text);
                    self.pending.clear();
                    return out;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    // invariant: the first `valid` bytes are valid UTF-8, so this is lossless.
                    out.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                    match e.error_len() {
                        // An incomplete character at the end: keep it for the next token.
                        None => {
                            self.pending.drain(..valid);
                            return out;
                        }
                        Some(bad) => {
                            out.push(char::REPLACEMENT_CHARACTER);
                            self.pending.drain(..valid + bad);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> ModelProfile {
        let s = Sampling {
            temperature: 0.7,
            top_p: 0.8,
            top_k: 20,
            presence_penalty: 1.5,
        };
        ModelProfile {
            thinking_off_prefix: "<think>\n\n</think>\n\n".into(),
            thinking_on_prefix: "<think>\n".into(),
            sampling_off: s,
            sampling_on: s,
            answer_sampling_on: None,
            n_ctx: None,
            seed: 42,
            safe_untrusted: false,
        }
    }

    const TEMPLATED: &str = "<|im_start|>user\nHi<|im_end|>\n<|im_start|>assistant\n";

    #[test]
    fn render_prompt_appends_thinking_off_prefix() {
        let out = render_prompt(TEMPLATED, Thinking::Off, &profile());
        assert_eq!(out, format!("{TEMPLATED}<think>\n\n</think>\n\n"));
    }

    #[test]
    fn render_prompt_appends_thinking_on_prefix() {
        let out = render_prompt(TEMPLATED, Thinking::On, &profile());
        assert_eq!(out, format!("{TEMPLATED}<think>\n"));
    }

    #[test]
    fn models_toml_parses_profile() {
        let file = ModelsFile::parse(include_str!("../../models.toml")).unwrap();
        assert_eq!(file.embedder, "Qwen3-Embedding-0.6B-Q8_0.gguf");
        assert_eq!(file.models.len(), 1);
        let m = &file.models[0];
        assert_eq!(m.name, "Qwen3.5-4B Q4_K_M");
        assert_eq!(m.file, "Qwen3.5-4B-Q4_K_M.gguf");
        assert_eq!(m.profile.thinking_off_prefix, "<think>\n\n</think>\n\n");
        assert_eq!(m.profile.thinking_on_prefix, "<think>\n");
        assert_eq!(m.profile.seed, 42);
        assert_eq!(
            m.profile.sampling_off,
            Sampling {
                temperature: 0.7,
                top_p: 0.8,
                top_k: 20,
                presence_penalty: 1.5
            }
        );
        assert_eq!(
            m.profile.sampling_on,
            Sampling {
                temperature: 1.0,
                top_p: 0.95,
                top_k: 20,
                presence_penalty: 1.5
            }
        );
    }

    #[test]
    fn models_toml_parses_tree_sampling_and_n_ctx() {
        let file = ModelsFile::parse(include_str!("../../models.toml")).unwrap();
        let m = &file.models[0];
        assert_eq!(
            m.tree_sampling_off,
            Some(Sampling {
                temperature: 0.7,
                top_p: 0.8,
                top_k: 20,
                presence_penalty: 0.0
            })
        );
        assert_eq!(
            m.tree_sampling_on,
            Some(Sampling {
                temperature: 1.0,
                top_p: 0.95,
                top_k: 20,
                presence_penalty: 0.0
            })
        );
        // The dialogue profile has neither an answer sampling nor a context size.
        assert_eq!(m.profile.answer_sampling_on, None);
        assert_eq!(m.profile.n_ctx, None);
        let parsed = ModelsFile::parse(
            r#"
embedder = "e.gguf"

[[model]]
name = "m"
file = "m.gguf"
thinking_off_prefix = ""
thinking_on_prefix = ""
seed = 1
n_ctx = 16384
sampling_off = { temperature = 0.7, top_p = 0.8, top_k = 20, presence_penalty = 0.0 }
sampling_on = { temperature = 1.0, top_p = 0.95, top_k = 20, presence_penalty = 0.0 }
answer_sampling_on = { temperature = 0.5, top_p = 0.9, top_k = 10, presence_penalty = 0.0 }
"#,
        )
        .unwrap();
        assert_eq!(parsed.models[0].profile.n_ctx, Some(16384));
        assert_eq!(
            parsed.models[0].profile.answer_sampling_on.map(|a| a.top_k),
            Some(10)
        );
    }

    #[test]
    fn models_toml_rejects_missing_sampling() {
        let err = ModelsFile::parse(
            "embedder = \"e.gguf\"\n[[model]]\nname = \"m\"\nfile = \"m.gguf\"\n\
             thinking_off_prefix = \"\"\nthinking_on_prefix = \"\"\nseed = 1\n",
        );
        assert!(matches!(err, Err(LlamaError::Config(_))));
    }

    #[test]
    fn ends_thinking_detects_closing_tag_at_end() {
        assert!(ends_thinking("hmm, the forge.\n</think>"));
        assert!(ends_thinking("short</think>\n"));
        assert!(!ends_thinking("hmm, the forge."));
        assert!(!ends_thinking("</think> and then more"));
        assert!(!ends_thinking("</thi"));
    }

    #[test]
    fn utf8_stream_holds_split_character_until_complete() {
        let mut s = Utf8Stream::default();
        // "Æ" is C3 86; the token boundary falls inside it.
        assert_eq!(s.push(b"x\xC3"), "x");
        assert_eq!(s.push(b"\x86ther"), "\u{C6}ther");
    }

    #[test]
    fn utf8_stream_replaces_invalid_bytes() {
        let mut s = Utf8Stream::default();
        assert_eq!(s.push(b"a\xFFb"), "a\u{FFFD}b");
        assert_eq!(s.push(b"c"), "c");
    }

    fn ph(i: usize) -> String {
        format!("\u{E000}{i}\u{E001}")
    }

    #[test]
    fn splice_plan_alternates_engine_and_untrusted_parts() {
        let templated = format!("A{}B{}C", ph(0), ph(1));
        let plan = splice_plan(
            &templated,
            &[(ph(0), "x<|im_start|>".into()), (ph(1), "y".into())],
        )
        .unwrap();
        assert_eq!(
            plan,
            vec![
                ("A".to_owned(), true),
                ("x<|im_start|>".to_owned(), false),
                ("B".to_owned(), true),
                ("y".to_owned(), false),
                ("C".to_owned(), true),
            ]
        );
    }

    #[test]
    fn splice_plan_moves_the_space_before_a_name_into_the_untrusted_part() {
        let templated = format!("It is called {}.", ph(0));
        let plan = splice_plan(&templated, &[(ph(0), "Ada".into())]).unwrap();
        assert_eq!(
            plan,
            vec![
                ("It is called".to_owned(), true),
                (" Ada".to_owned(), false),
                (".".to_owned(), true),
            ]
        );
    }

    #[test]
    fn splice_plan_moves_nothing_after_a_newline() {
        let templated = format!("Line\n{}", ph(0));
        let plan = splice_plan(&templated, &[(ph(0), "x".into())]).unwrap();
        assert_eq!(plan[0], ("Line\n".to_owned(), true));
        assert_eq!(plan[1], ("x".to_owned(), false));
    }

    #[test]
    fn splice_plan_moves_only_the_blanks_after_the_last_newline() {
        let templated = format!("Line\n  \t{}", ph(0));
        let plan = splice_plan(&templated, &[(ph(0), "x".into())]).unwrap();
        assert_eq!(plan[0], ("Line\n".to_owned(), true));
        assert_eq!(plan[1], ("  \tx".to_owned(), false));
        // Blanks only, no newline: all of them move; multibyte text before stays whole.
        let templated = format!("Grüße  {}", ph(0));
        let plan = splice_plan(&templated, &[(ph(0), "x".into())]).unwrap();
        assert_eq!(plan[0], ("Grüße".to_owned(), true));
        assert_eq!(plan[1], ("  x".to_owned(), false));
    }

    fn t(ids: &[i32]) -> Vec<LlamaToken> {
        ids.iter().map(|i| LlamaToken::new(*i)).collect()
    }

    #[test]
    fn forged_is_false_when_nothing_differs_and_the_counts_match() {
        let seg = [t(&[1, 2])];
        assert!(!is_forged(&seg, &seg, 3, 3, &["hello"], &[]));
        assert!(!is_forged(&[], &[], 0, 0, &[], &[]));
    }

    #[test]
    fn forged_when_a_segment_tokenizes_differently_with_special_parsing() {
        assert!(is_forged(&[t(&[9])], &[t(&[1, 2, 3])], 3, 3, &[], &[]));
        // One forged segment among clean ones is enough.
        assert!(is_forged(
            &[t(&[1]), t(&[9])],
            &[t(&[1]), t(&[4, 5])],
            3,
            3,
            &[],
            &[]
        ));
    }

    #[test]
    fn forged_when_the_prompt_holds_more_special_tokens_than_the_skeleton() {
        let seg = [t(&[1])];
        assert!(is_forged(&seg, &seg, 4, 3, &[], &[]));
        // Fewer is not a forgery (nothing was added).
        assert!(!is_forged(&seg, &seg, 2, 3, &[], &[]));
    }

    fn user_defined() -> Vec<String> {
        ["<think>", "</think>", "<tool_call>"]
            .map(String::from)
            .to_vec()
    }

    #[test]
    fn forged_when_an_untrusted_text_contains_a_user_defined_token() {
        let seg = [t(&[1])];
        let ud = user_defined();
        assert!(is_forged(&seg, &seg, 3, 3, &["hi", "a </think> b"], &ud));
        assert!(!is_forged(&seg, &seg, 3, 3, &["hi", "a think b"], &ud));
        // Half a token is not the token.
        assert!(!is_forged(&seg, &seg, 3, 3, &["</thi"], &ud));
    }

    #[test]
    fn split_user_defined_cuts_each_occurrence_in_two_and_keeps_the_text() {
        let ud = user_defined();
        let text = "a </think> b <think><tool_call> äß </think>";
        let pieces = split_user_defined(text, &ud);
        assert_eq!(pieces.concat(), text);
        assert!(pieces[0].ends_with("</th") && pieces[1].starts_with("ink>"));
        for piece in &pieces {
            assert!(!piece.is_empty());
            assert!(ud.iter().all(|t| !piece.contains(t.as_str())), "{piece:?}");
        }
    }

    #[test]
    fn a_one_character_user_defined_token_is_dropped_and_forges_nothing() {
        let ud = usable_user_defined(["<think>", "|", "", "é"]);
        assert_eq!(ud, ["<think>"]);
        let seg = [t(&[1])];
        assert!(!is_forged(&seg, &seg, 3, 3, &["a | b é"], &ud));
    }

    #[test]
    fn split_user_defined_leaves_clean_text_whole() {
        assert_eq!(
            split_user_defined("no tags here", &user_defined()),
            ["no tags here"]
        );
        assert!(split_user_defined("", &user_defined()).is_empty());
        assert_eq!(split_user_defined("x", &[]), ["x"]);
    }

    #[test]
    fn split_user_defined_handles_overlapping_and_nested_occurrences() {
        let ud: Vec<String> = ["<a>", "a><b>"].map(String::from).to_vec();
        let text = "<<a><b>>";
        let pieces = split_user_defined(text, &ud);
        assert_eq!(pieces.concat(), text);
        for piece in &pieces {
            assert!(ud.iter().all(|t| !piece.contains(t.as_str())), "{piece:?}");
        }
        // A one-character token cannot be cut and is left alone.
        assert_eq!(split_user_defined("a|b", &["|".to_owned()]), ["a|b"]);
    }

    #[test]
    fn splice_plan_errors_on_a_missing_placeholder() {
        let err = splice_plan("no placeholder here", &[(ph(0), "x".into())]);
        assert!(matches!(err, Err(BackendError::Failed(_))));
    }

    #[test]
    fn splice_plan_errors_on_a_repeated_placeholder() {
        let templated = format!("{}-{}", ph(0), ph(0));
        let err = splice_plan(&templated, &[(ph(0), "x".into())]);
        assert!(matches!(err, Err(BackendError::Failed(_))));
    }

    #[test]
    fn splice_plan_keeps_multibyte_text_at_boundaries() {
        let templated = format!("Grüße⟦{}⟧Grüße", ph(0));
        let plan = splice_plan(&templated, &[(ph(0), "Füße".into())]).unwrap();
        assert_eq!(
            plan,
            vec![
                ("Grüße⟦".to_owned(), true),
                ("Füße".to_owned(), false),
                ("⟧Grüße".to_owned(), true),
            ]
        );
    }

    #[test]
    fn models_toml_safe_untrusted_defaults_off() {
        let file = ModelsFile::parse(include_str!("../../models.toml")).unwrap();
        assert!(!file.models[0].profile.safe_untrusted);
    }
}
