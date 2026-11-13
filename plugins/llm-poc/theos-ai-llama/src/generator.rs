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
use llama_cpp_2::DecodeError;
use llguidance::api::TopLevelGrammar;
use llguidance::{Matcher, ParserFactory};
use serde::Deserialize;
use serde_json::Value;
use theos_ai::{Backend, BackendError, Decoder, GenerationRequest, Phase, Piece, Thinking};

use crate::{LlamaError, LlamaRuntime, ModelContext};

/// Context size, and the largest batch a prompt is prefilled in, in tokens.
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
    /// Seed of the final `dist` sampler.
    pub seed: u32,
}

/// One `[[model]]` entry of `models.toml`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ModelEntry {
    /// Display name for the results tables.
    pub name: String,
    /// GGUF file name, relative to the models directory.
    pub file: String,
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

/// A llama.cpp generator with all layers on the GPU and an 8192-token context.
pub struct LlamaGenerator {
    mc: ModelContext,
    batch: LlamaBatch<'static>,
    factory: ParserFactory,
    template: LlamaChatTemplate,
    separator: Vec<LlamaToken>,
    profile: ModelProfile,
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
        // Built once per model: walking the vocabulary takes a while.
        let env = LlamaSampler::llguidance_tok_env(&model);
        let factory =
            ParserFactory::new_simple(&env).map_err(|e| LlamaError::Grammar(e.to_string()))?;
        let params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(N_CTX))
            .with_n_batch(N_CTX);
        let mc = ModelContext::new(rt, model, params)?;
        Ok(LlamaGenerator {
            mc,
            batch: LlamaBatch::new(N_CTX as usize, 1),
            factory,
            template,
            separator,
            profile,
        })
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

    /// The profile the generator was loaded with.
    pub fn profile(&self) -> &ModelProfile {
        &self.profile
    }

    /// The chat-templated, prefixed prompt text for `request`.
    fn prompt_text(&self, request: &GenerationRequest<'_>) -> Result<String, BackendError> {
        let message = |role: &str, content: &str| {
            LlamaChatMessage::new(role.to_owned(), content.to_owned())
                .map_err(|e| BackendError::Failed(format!("chat message: {e}")))
        };
        let chat = [
            message("system", &request.prompt.system)?,
            message("user", &request.prompt.user)?,
        ];
        let templated = self
            .mc
            .model()
            .apply_chat_template(&self.template, &chat, true)
            .map_err(|e| BackendError::Failed(format!("chat template: {e}")))?;
        Ok(render_prompt(&templated, request.thinking, &self.profile))
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
        let prompt = self.prompt_text(request)?;
        let tokens = self
            .mc
            .model()
            .vocab()
            .tokenize(prompt.as_bytes(), true, true);
        if tokens.is_empty() {
            return Err(BackendError::Failed("prompt tokenized to nothing".into()));
        }
        let n_ctx = self.mc.n_ctx();
        let n_prompt = u32::try_from(tokens.len()).map_err(|_| BackendError::ContextFull)?;
        if n_prompt >= n_ctx {
            return Err(BackendError::ContextFull);
        }
        let matcher = schema_matcher(&self.factory, request.schema)?;

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
                sampler_chain(n_vocab, sampling, seed, Some(matcher)),
                None,
                Phase::Answer,
            ),
            Thinking::On => (
                sampler_chain(n_vocab, sampling, seed, None),
                Some(matcher),
                Phase::Thinking,
            ),
        };
        // invariant: 0 < n_prompt < n_ctx = 8192, so these fit in i32.
        let pos = n_prompt as i32;
        Ok(Box::new(LlamaDecoder {
            mc: &mut self.mc,
            batch: &mut self.batch,
            separator: &self.separator,
            sampler,
            answer_grammar,
            sampling,
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
    sampler: LlamaSampler,
    /// Thinking on: the schema matcher waiting for the end of the thinking block.
    answer_grammar: Option<Matcher>,
    sampling: Sampling,
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

    /// Closes the thinking phase: queue the separator, switch to the schema chain.
    fn start_answer(&mut self) {
        self.pending.extend_from_slice(self.separator);
        // A fresh matcher: none of the thinking tokens are replayed into it.
        let grammar = self.answer_grammar.take();
        self.sampler = sampler_chain(self.n_vocab, self.sampling, self.seed, grammar);
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
            seed: 42,
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
}
