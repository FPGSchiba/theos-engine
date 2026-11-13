//! Test-support helpers for code that depends on `theos-ai`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::backend::{Backend, BackendError, Decoder, GenerationRequest, Piece, Thinking};
use crate::clock::Clock;
use crate::context::Prompt;
use crate::embed::{EmbedError, EmbedInput, Embedder};
use crate::memory::MemoryProbe;

/// Locks `m`, recovering the data if a panicking test poisoned the mutex.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Default dimensionality of a [`FakeEmbedder`].
const DEFAULT_DIMS: usize = 64;

/// A deterministic bag-of-words embedder with no model behind it.
///
/// Lowercases the text, splits it on non-alphanumeric characters, hashes each
/// word (FNV-1a) into one of `dims` buckets and L2-normalises the counts.
/// Queries and documents are embedded identically, so texts sharing words
/// score closer together.
#[derive(Clone, Debug)]
pub struct FakeEmbedder {
    dims: usize,
    calls: usize,
}

impl FakeEmbedder {
    /// Creates an embedder producing vectors of `dims` dimensions (at least 1).
    pub fn new(dims: usize) -> Self {
        Self {
            dims: dims.max(1),
            calls: 0,
        }
    }

    /// How many times [`Embedder::embed`] has been called (not inputs embedded).
    pub fn calls(&self) -> usize {
        self.calls
    }

    fn embed_text(&self, text: &str) -> Vec<f32> {
        let mut v = vec![0.0_f32; self.dims];
        let lower = text.to_lowercase();
        for word in lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
        {
            let mut h: u64 = 0xcbf2_9ce4_8422_2325;
            for b in word.bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
            // The remainder is below `dims`, so it fits in usize.
            v[(h % self.dims as u64) as usize] += 1.0;
        }
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            v.iter_mut().for_each(|x| *x /= norm);
        }
        v
    }
}

impl Default for FakeEmbedder {
    fn default() -> Self {
        Self::new(DEFAULT_DIMS)
    }
}

impl Embedder for FakeEmbedder {
    fn embed(&mut self, inputs: &[EmbedInput<'_>]) -> Result<Vec<Vec<f32>>, EmbedError> {
        self.calls += 1;
        Ok(inputs
            .iter()
            .map(|i| match i {
                EmbedInput::Query(t) | EmbedInput::Document(t) => self.embed_text(t),
            })
            .collect())
    }
}

/// A manually advanced clock.
///
/// Clones share one offset, so a [`ScriptedBackend`] and the code under test
/// see the same time.
#[derive(Clone, Debug)]
pub struct FakeClock {
    base: Instant,
    offset: Arc<Mutex<Duration>>,
}

impl FakeClock {
    /// Creates a clock reading "now" that only moves when advanced.
    pub fn new() -> Self {
        Self {
            base: Instant::now(),
            offset: Arc::new(Mutex::new(Duration::ZERO)),
        }
    }

    /// Moves the clock, and every clone of it, forward by `by`.
    pub fn advance(&self, by: Duration) {
        *lock(&self.offset) += by;
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Instant {
        self.base + *lock(&self.offset)
    }
}

/// One scripted generation for a [`ScriptedBackend`].
#[derive(Clone, Debug)]
pub struct Script {
    /// Prompt tokens the decoder reports.
    pub prompt_tokens: u32,
    /// Clock time consumed by [`Backend::begin`].
    pub prefill: Duration,
    /// Clock time consumed by each token.
    pub per_token: Duration,
    /// The tokens to emit, in order.
    pub pieces: Vec<Piece>,
}

#[derive(Default)]
struct ScriptedState {
    scripts: VecDeque<Script>,
    requests: Vec<(Prompt, Value, Thinking)>,
}

/// A backend that replays [`Script`]s and advances a [`FakeClock`] as it goes.
///
/// State is shared with any [`ScriptedBackend::handle`], so
/// [`ScriptedBackend::requests`] stays readable after the backend has moved
/// into an `Npc`.
pub struct ScriptedBackend {
    clock: FakeClock,
    state: Arc<Mutex<ScriptedState>>,
}

impl ScriptedBackend {
    /// Creates a backend with no scripts that advances `clock`.
    pub fn new(clock: FakeClock) -> Self {
        Self {
            clock,
            state: Arc::new(Mutex::new(ScriptedState::default())),
        }
    }

    /// Queues a script; each [`Backend::begin`] consumes the oldest one.
    pub fn push(&self, script: Script) {
        lock(&self.state).scripts.push_back(script);
    }

    /// Every request received so far, as `(prompt, schema, thinking)`.
    pub fn requests(&self) -> Vec<(Prompt, Value, Thinking)> {
        lock(&self.state).requests.clone()
    }

    /// A handle onto the same state, for reading `requests` after a move.
    pub fn handle(&self) -> ScriptedBackend {
        Self {
            clock: self.clock.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

struct ScriptedDecoder {
    clock: FakeClock,
    prompt_tokens: u32,
    per_token: Duration,
    pieces: VecDeque<Piece>,
}

impl Decoder for ScriptedDecoder {
    fn prompt_tokens(&self) -> u32 {
        self.prompt_tokens
    }

    fn next(&mut self) -> Result<Option<Piece>, BackendError> {
        let Some(piece) = self.pieces.pop_front() else {
            return Ok(None);
        };
        self.clock.advance(self.per_token);
        Ok(Some(piece))
    }
}

impl Backend for ScriptedBackend {
    fn begin<'a>(
        &'a mut self,
        request: &GenerationRequest<'_>,
    ) -> Result<Box<dyn Decoder + 'a>, BackendError> {
        let mut state = lock(&self.state);
        state.requests.push((
            request.prompt.clone(),
            request.schema.clone(),
            request.thinking,
        ));
        let script = state
            .scripts
            .pop_front()
            .ok_or_else(|| BackendError::Failed("no script".into()))?;
        self.clock.advance(script.prefill);
        Ok(Box::new(ScriptedDecoder {
            clock: self.clock.clone(),
            prompt_tokens: script.prompt_tokens,
            per_token: script.per_token,
            pieces: script.pieces.into(),
        }))
    }
}

/// A memory probe replaying fixed samples, then repeating the last one.
pub struct FakeProbe {
    samples: Mutex<VecDeque<u64>>,
    last: Mutex<Option<u64>>,
}

impl FakeProbe {
    /// Creates a probe returning `samples` in turn; with none it reports `None`.
    pub fn new(samples: Vec<u64>) -> Self {
        Self {
            samples: Mutex::new(samples.into()),
            last: Mutex::new(None),
        }
    }
}

impl MemoryProbe for FakeProbe {
    fn vram_bytes(&self) -> Option<u64> {
        let mut last = lock(&self.last);
        if let Some(next) = lock(&self.samples).pop_front() {
            *last = Some(next);
        }
        *last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::cosine;

    fn one(e: &mut FakeEmbedder, text: &str) -> Vec<f32> {
        e.embed(&[EmbedInput::Document(text)]).unwrap().remove(0)
    }

    #[test]
    fn fake_embedder_is_deterministic() {
        let mut e = FakeEmbedder::default();
        assert_eq!(one(&mut e, "frozen binders"), one(&mut e, "frozen binders"));
        assert_eq!(e.calls(), 2);
    }

    #[test]
    fn fake_embedder_scores_shared_words_higher() {
        let mut e = FakeEmbedder::default();
        let a = one(&mut e, "frozen binders in the passes");
        let near = one(&mut e, "binders frozen mid-song");
        let far = one(&mut e, "the songbook ends");
        assert!(cosine(&a, &near) > cosine(&a, &far));
    }
}
