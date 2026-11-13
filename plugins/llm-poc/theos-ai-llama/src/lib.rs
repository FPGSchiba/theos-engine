//! llama.cpp backend for the NPC proof of concept: generator, embedder and VRAM probe.
//!
//! One [`LlamaRuntime`] per process owns the llama.cpp backend. The
//! [`LlamaGenerator`] implements `theos_ai::Backend`, the [`LlamaEmbedder`]
//! implements `theos_ai::Embedder` and the [`DxgiProbe`] implements
//! `theos_ai::MemoryProbe`. Generator and embedder hold llama contexts, so
//! build them on the thread that uses them.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;
use llama_cpp_2::{DecodeError, EmbeddingsSeqError};

pub mod embedder;
pub mod generator;
pub mod vram;

pub use embedder::{l2_normalize, LlamaEmbedder, QUERY_INSTRUCTION};
pub use generator::{
    render_prompt, LlamaGenerator, ModelEntry, ModelProfile, ModelsFile, Sampling,
};
pub use vram::DxgiProbe;

/// Why loading or setting up a model failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LlamaError {
    /// The llama.cpp backend could not be initialised.
    #[error("llama.cpp backend init failed: {0}")]
    Init(String),
    /// The model file does not exist.
    #[error("model file not found: {}", .0.display())]
    MissingFile(PathBuf),
    /// llama.cpp could not load the model file.
    #[error("loading {} failed: {reason}", path.display())]
    Load {
        /// The file that failed to load.
        path: PathBuf,
        /// What llama.cpp reported.
        reason: String,
    },
    /// llama.cpp could not create a context for the model.
    #[error("creating a llama context failed: {0}")]
    Context(String),
    /// The model's tokenizer or chat template is not what this backend needs.
    #[error("model setup failed: {0}")]
    Model(String),
    /// The llguidance grammar machinery could not be set up.
    #[error("llguidance setup failed: {0}")]
    Grammar(String),
    /// `models.toml` could not be read or parsed.
    #[error("models config: {0}")]
    Config(String),
}

/// The process-wide llama.cpp backend.
///
/// llama.cpp may be initialised only once per process; [`LlamaRuntime::init`]
/// hands out the live runtime when there is one.
#[derive(Debug)]
pub struct LlamaRuntime {
    backend: LlamaBackend,
}

/// The runtime currently alive, if any.
static RUNTIME: Mutex<Weak<LlamaRuntime>> = Mutex::new(Weak::new());

impl LlamaRuntime {
    /// Initialises the llama.cpp backend, or returns the runtime already alive.
    ///
    /// # Errors
    ///
    /// [`LlamaError::Init`] when llama.cpp refuses to initialise.
    pub fn init() -> Result<Arc<LlamaRuntime>, LlamaError> {
        let mut slot = RUNTIME.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(rt) = slot.upgrade() {
            return Ok(rt);
        }
        let backend = LlamaBackend::init().map_err(|e| LlamaError::Init(e.to_string()))?;
        let rt = Arc::new(LlamaRuntime { backend });
        *slot = Arc::downgrade(&rt);
        Ok(rt)
    }

    /// Loads a GGUF model with every layer on the GPU.
    pub(crate) fn load_model(&self, path: &Path) -> Result<LlamaModel, LlamaError> {
        if !path.is_file() {
            return Err(LlamaError::MissingFile(path.to_path_buf()));
        }
        let params = LlamaModelParams::default().with_n_gpu_layers(u32::MAX);
        LlamaModel::load_from_file(&self.backend, path, &params).map_err(|e| LlamaError::Load {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })
    }

    /// The llama.cpp backend handle.
    pub(crate) fn backend(&self) -> &LlamaBackend {
        &self.backend
    }
}

/// A loaded model together with the one context that runs it.
///
/// `LlamaContext` borrows its model, which would make a generator that owns
/// both self-referential. The model lives in an `Arc` whose allocation never
/// moves; the context is created from a `'static` reference into it and is
/// only reachable through the methods below, so it can never escape this
/// struct or outlive the model.
pub(crate) struct ModelContext {
    // Field order is drop order: the context goes before the model it borrows,
    // and both go before the runtime whose backend they need.
    ctx: LlamaContext<'static>,
    model: Arc<LlamaModel>,
    _rt: Arc<LlamaRuntime>,
}

impl ModelContext {
    /// Creates a context with `params` on `model`.
    pub(crate) fn new(
        rt: Arc<LlamaRuntime>,
        model: LlamaModel,
        params: LlamaContextParams,
    ) -> Result<Self, LlamaError> {
        let model = Arc::new(model);
        // SAFETY: the reference points into the `Arc` allocation, which does not
        // move and stays alive as long as `ModelContext::model` holds the `Arc`.
        // The only value created from it is the context stored in `ctx`, which is
        // declared before `model` (so it is dropped first) and is never handed out
        // by value or by `&mut`, so it cannot outlive the model.
        let model_ref: &'static LlamaModel = unsafe { &*Arc::as_ptr(&model) };
        let ctx = model_ref
            .new_context(rt.backend(), params)
            .map_err(|e| LlamaError::Context(e.to_string()))?;
        Ok(Self {
            ctx,
            model,
            _rt: rt,
        })
    }

    /// The model.
    pub(crate) fn model(&self) -> &LlamaModel {
        &self.model
    }

    /// The context size in tokens.
    pub(crate) fn n_ctx(&self) -> u32 {
        self.ctx.n_ctx()
    }

    /// Forgets every token (and recurrent state) in the context.
    pub(crate) fn clear(&mut self) {
        self.ctx.clear_kv_cache();
    }

    /// Runs the model over `batch`.
    pub(crate) fn decode(&mut self, batch: &mut LlamaBatch) -> Result<(), DecodeError> {
        self.ctx.decode(batch)
    }

    /// Samples (and accepts) a token from the logits of batch index `idx`.
    pub(crate) fn sample(&self, sampler: &mut LlamaSampler, idx: i32) -> LlamaToken {
        sampler.sample(&self.ctx, idx)
    }

    /// The pooled embedding of sequence `seq` after the last decode.
    pub(crate) fn sequence_embedding(&self, seq: i32) -> Result<&[f32], EmbeddingsSeqError> {
        self.ctx.embeddings_seq_ith(seq)
    }
}

/// The directory GGUF files are read from: `THEOS_MODELS_DIR`, or `<repo>/models`.
pub fn models_dir() -> PathBuf {
    std::env::var_os("THEOS_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../models"))
}
