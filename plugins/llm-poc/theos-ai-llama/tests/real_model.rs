//! Tests against the real GGUF models on the GPU.
//!
//! They need the files named in `plugins/llm-poc/models.toml` in the models
//! directory (`THEOS_MODELS_DIR`, default `<repo>/models`) and fail with a
//! message saying so when a file is missing. Tests share one llama.cpp runtime
//! and take turns on the GPU so the VRAM readings are not disturbed.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};

use theos_ai::{
    cosine, parse_reply, reply_schema, Backend, EmbedInput, Embedder, GenerationRequest,
    MemoryProbe, Phase, Prompt, RecordId, Thinking,
};
use theos_ai_llama::{
    models_dir, DxgiProbe, LlamaEmbedder, LlamaGenerator, LlamaRuntime, ModelsFile,
};

/// Longest generation a test waits for before giving up.
const MAX_PIECES: usize = 2000;

fn gpu() -> MutexGuard<'static, ()> {
    static GPU: Mutex<()> = Mutex::new(());
    GPU.lock().unwrap_or_else(PoisonError::into_inner)
}

fn runtime() -> Arc<LlamaRuntime> {
    static RT: OnceLock<Arc<LlamaRuntime>> = OnceLock::new();
    Arc::clone(RT.get_or_init(|| LlamaRuntime::init().expect("llama.cpp backend init")))
}

fn models() -> ModelsFile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models.toml");
    ModelsFile::load(&path).expect("plugins/llm-poc/models.toml parses")
}

/// The path of a model file, failing the test with instructions when it is missing.
fn require(file: &str) -> PathBuf {
    let path = models_dir().join(file);
    assert!(
        path.is_file(),
        "model file {} is missing: download it into the models directory, or set \
         THEOS_MODELS_DIR to the directory that holds it (THEOS_MODELS_DIR is {:?})",
        path.display(),
        std::env::var_os("THEOS_MODELS_DIR"),
    );
    path
}

fn generator() -> LlamaGenerator {
    let m = &models().models[0];
    LlamaGenerator::load(runtime(), &require(&m.file), m.profile.clone()).expect("load generator")
}

fn embedder() -> LlamaEmbedder {
    LlamaEmbedder::load(runtime(), &require(&models().embedder)).expect("load embedder")
}

fn ids() -> Vec<RecordId> {
    vec![RecordId::from("B-945"), RecordId::from("B-946")]
}

fn prompt() -> Prompt {
    Prompt {
        system: "You are Hask, a throat-singer of the high passes. Answer the player in \
                 character, in one or two sentences. Reply as JSON: list the records you \
                 draw on in \"sources\" and put your answer in \"reply\".\n\
                 What you know:\n\
                 [B-945] The greater the god, the truer the note it demands. A small spirit \
                 forgives a clumsy song, but a great or proud Listener will not.\n\
                 [B-946] Vareth the Long Cold is a frost-elemental of the high passes: her \
                 note is low and slow, her power vast and her pride immense."
            .into(),
        user: "Is it safe to sing to Vareth?".into(),
    }
}

// Title and body of B-946 and B-947, as the retriever embeds them.
const B946: &str = "Vareth the Long Cold is a frost-elemental of the high passes: her note is \
                    low and slow, her power vast and her pride immense.\nOne of the Three Named \
                    Listeners told to every apprentice. Bind her and you may raise rivers of ice \
                    and bridges from nothing. She suffers no false singer, and the passes are \
                    full of binders frozen mid-song.";
const B947: &str = "Old Grandfather Ember is an ancestor-spirit of the forge-clans, warm and \
                    forgiving.\nOne of the Three Named Listeners. His note is easy to find and he \
                    lends gladly, but he lends little. Apprentices learn on him because his \
                    Quarrel is gentle and he rarely turns.";

#[test]
fn generator_reply_parses_with_theos_ai_parse_reply() {
    let _gpu = gpu();
    let mut generator = generator();
    let (prompt, ids) = (prompt(), ids());
    let schema = reply_schema(&ids);
    let request = GenerationRequest {
        prompt: &prompt,
        schema: &schema,
        thinking: Thinking::Off,
    };
    let mut decoder = generator.begin(&request).expect("begin");
    assert!(decoder.prompt_tokens() > 0);
    let mut raw = String::new();
    let mut finished = false;
    for _ in 0..MAX_PIECES {
        match decoder.next().expect("next") {
            Some(piece) => {
                assert_eq!(piece.phase, Phase::Answer, "thinking off emits only answer");
                raw.push_str(&piece.text);
            }
            None => {
                finished = true;
                break;
            }
        }
    }
    assert!(
        finished,
        "no end of generation after {MAX_PIECES} pieces: {raw}"
    );
    let reply = parse_reply(&raw, &ids);
    assert!(reply.is_ok(), "{reply:?} from raw output {raw:?}");
}

#[test]
fn generator_emits_thinking_pieces_then_answer_when_thinking_on() {
    let _gpu = gpu();
    let mut generator = generator();
    let (prompt, ids) = (prompt(), ids());
    let schema = reply_schema(&ids);
    let request = GenerationRequest {
        prompt: &prompt,
        schema: &schema,
        thinking: Thinking::On,
    };
    let mut decoder = generator.begin(&request).expect("begin");
    let mut thinking = 0;
    let mut answered = false;
    for _ in 0..MAX_PIECES {
        match decoder.next().expect("next") {
            Some(piece) if piece.phase == Phase::Answer => {
                answered = true;
                break;
            }
            Some(_) => thinking += 1,
            None => break,
        }
    }
    assert!(
        answered,
        "no answer piece within {MAX_PIECES} pieces ({thinking} thinking)"
    );
    assert!(thinking > 0, "the answer came without any thinking piece");
}

#[test]
fn embedder_returns_unit_vectors_of_dimension_1024() {
    let _gpu = gpu();
    let mut embedder = embedder();
    let vectors = embedder
        .embed(&[
            EmbedInput::Query("Which spirit should a beginner learn on?"),
            EmbedInput::Document(B947),
        ])
        .expect("embed");
    assert_eq!(vectors.len(), 2);
    for v in &vectors {
        assert_eq!(v.len(), 1024);
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "norm {norm}");
    }
}

#[test]
fn embedder_ranks_related_lore_above_unrelated() {
    let _gpu = gpu();
    let mut embedder = embedder();
    let v = embedder
        .embed(&[
            EmbedInput::Query("Which spirit should a beginner learn on?"),
            EmbedInput::Document(B947),
            EmbedInput::Document(B946),
        ])
        .expect("embed");
    let (related, unrelated) = (cosine(&v[0], &v[1]), cosine(&v[0], &v[2]));
    assert!(
        related > unrelated,
        "B-947 scored {related}, B-946 scored {unrelated}"
    );
}

// Needs no model file: checks the probe finds the NVIDIA adapter and reads it.
#[test]
fn vram_probe_reads_the_nvidia_adapter() {
    let probe = DxgiProbe::new().expect("an NVIDIA adapter");
    assert!(probe.vram_bytes().is_some(), "DXGI query failed");
}

#[test]
fn vram_probe_reads_more_after_loading_the_generator() {
    let _gpu = gpu();
    let path = require(&models().models[0].file);
    let file_bytes = std::fs::metadata(&path).expect("model metadata").len();
    let probe = DxgiProbe::new().expect("an NVIDIA adapter");
    let before = probe.vram_bytes().expect("DXGI reading before load");
    let generator = generator();
    let after = probe.vram_bytes().expect("DXGI reading after load");
    // A rise of at least half the file shows DXGI sees the CUDA allocations.
    assert!(
        after >= before + file_bytes / 2,
        "VRAM went from {before} to {after} bytes loading a {file_bytes}-byte model"
    );
    drop(generator);
}
