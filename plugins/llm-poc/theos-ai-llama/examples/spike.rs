//! Spike: Qwen3.5-4B on the GPU via llama-cpp-2, plus llguidance, embeddings and a DXGI VRAM probe.
//! A check, not product code.

// The example prints its results; the repo-level clippy.toml bans `println!`.
#![allow(clippy::disallowed_macros)]

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Instant;

use llama_cpp_2::context::params::{LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::sampling::LlamaSampler;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// Local-segment `CurrentUsage` of this process on the NVIDIA adapter, in bytes.
#[cfg(windows)]
fn dxgi_usage() -> Option<u64> {
    use windows::core::Interface;
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO,
    };
    // SAFETY: plain COM calls on interfaces we own; out-params are valid stack values.
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut i = 0;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            i += 1;
            let desc = adapter.GetDesc1().ok()?;
            if desc.VendorId != 0x10DE {
                continue;
            }
            let adapter3: IDXGIAdapter3 = adapter.cast().ok()?;
            let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            adapter3
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
                .ok()?;
            return Some(info.CurrentUsage);
        }
        None
    }
}

#[cfg(not(windows))]
fn dxgi_usage() -> Option<u64> {
    None
}

fn print_dxgi(label: &str) {
    match dxgi_usage() {
        Some(b) => println!("[dxgi] {label}: {:.1} MiB", b as f64 / 1048576.0),
        None => println!("[dxgi] {label}: unavailable"),
    }
}

/// Decode the prompt, then sample up to `max_new` tokens.
/// Returns (text, prompt_tokens, time to first token in s, tokens per second).
fn generate(
    model: &LlamaModel,
    backend: &LlamaBackend,
    prompt: &str,
    sampler: &mut LlamaSampler,
    max_new: usize,
) -> Res<(String, usize, f64, f64)> {
    let mut ctx = model.new_context(
        backend,
        LlamaContextParams::default().with_n_ctx(NonZeroU32::new(8192)),
    )?;
    let vocab = model.vocab();
    let tokens = vocab.tokenize(prompt.as_bytes(), false, true);
    let n_prompt = tokens.len();
    let mut batch = LlamaBatch::new(8192, 1);
    for (i, t) in tokens.iter().enumerate() {
        batch.add(*t, i as i32, &[0], i == n_prompt - 1)?;
    }
    let start = Instant::now();
    ctx.decode(&mut batch)?;
    let mut pos = n_prompt as i32;
    let mut out = Vec::new();
    let mut ttft = 0.0;
    let mut n = 0usize;
    let mut idx = batch.n_tokens() - 1;
    while n < max_new {
        let tok = sampler.sample(&ctx, idx);
        if n == 0 {
            ttft = start.elapsed().as_secs_f64();
        }
        if vocab.is_eog(tok) {
            break;
        }
        out.extend(vocab.token_to_piece(tok, false, None));
        n += 1;
        batch.clear();
        batch.add(tok, pos, &[0], true)?;
        pos += 1;
        idx = 0;
        ctx.decode(&mut batch)?;
    }
    let total = start.elapsed().as_secs_f64();
    let tps = if total > ttft {
        n.saturating_sub(1) as f64 / (total - ttft)
    } else {
        0.0
    };
    Ok((
        String::from_utf8_lossy(&out).into_owned(),
        n_prompt,
        ttft,
        tps,
    ))
}

fn main() -> Res<()> {
    let models: PathBuf = std::env::var_os("THEOS_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../models"));
    let backend = LlamaBackend::init()?;

    // 1. DXGI before any load.
    print_dxgi("before load");

    // 2. Load the generator.
    let t = Instant::now();
    let model = LlamaModel::load_from_file(
        &backend,
        models.join("Qwen3.5-4B-Q4_K_M.gguf"),
        &LlamaModelParams::default().with_n_gpu_layers(u32::MAX),
    )?;
    println!(
        "[load] n_params={} size={} bytes in {:.1}s",
        model.n_params(),
        model.size(),
        t.elapsed().as_secs_f64()
    );
    print_dxgi("after generator load");

    // 3. Unconstrained greedy generation.
    let prompt = "<|im_start|>system\nYou are a gruff blacksmith in a fantasy town.<|im_end|>\n\
                  <|im_start|>user\nWhat do you sell?<|im_end|>\n\
                  <|im_start|>assistant\n<think>\n\n</think>\n\n";
    let mut greedy = LlamaSampler::greedy();
    let (text, n_prompt, ttft, tps) = generate(&model, &backend, prompt, &mut greedy, 128)?;
    println!("[unconstrained] prompt_tokens={n_prompt} ttft={ttft:.3}s tok/s={tps:.1}");
    println!("[unconstrained] output: {text}");

    // 4. llguidance-constrained generation.
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "sources": { "type": "array", "items": { "enum": ["B-946", "B-945"] }, "maxItems": 5 },
            "reply": { "type": "string", "minLength": 1, "maxLength": 600 }
        },
        "required": ["sources", "reply"],
        "additionalProperties": false
    });
    let env = LlamaSampler::llguidance_tok_env(&model);
    let factory = llguidance::ParserFactory::new_simple(&env)?;
    let grammar = llguidance::api::TopLevelGrammar::from_json_schema(schema);
    let parser = factory.create_parser(grammar)?;
    let matcher = llguidance::Matcher::new(Ok(parser));
    let guided: LlamaSampler = matcher.into();
    // The llguidance sampler only masks logits; chain a selector after it.
    let mut guided = LlamaSampler::chain_simple([guided, LlamaSampler::greedy()]);
    let gprompt = "<|im_start|>system\nAnswer as JSON with the records you cite in \"sources\" and your answer in \"reply\". Records: [B-946] the forge; [B-945] the mine.<|im_end|>\n\
                   <|im_start|>user\nWhere does the ore come from?<|im_end|>\n\
                   <|im_start|>assistant\n<think>\n\n</think>\n\n";
    let (raw, _, gttft, gtps) = generate(&model, &backend, gprompt, &mut guided, 256)?;
    println!("[schema] ttft={gttft:.3}s tok/s={gtps:.1}");
    println!("[schema] raw output: {raw}");
    match (raw.find("\"sources\""), raw.find("\"reply\"")) {
        (Some(s), Some(r)) => println!("[schema] sources precedes reply: {}", s < r),
        _ => println!("[schema] sources precedes reply: unknown (key missing)"),
    }
    println!(
        "[schema] parses as JSON: {}",
        serde_json::from_str::<serde_json::Value>(&raw).is_ok()
    );

    // 5. Embedding model.
    let emb = LlamaModel::load_from_file(
        &backend,
        models.join("Qwen3-Embedding-0.6B-Q8_0.gguf"),
        &LlamaModelParams::default().with_n_gpu_layers(u32::MAX),
    )?;
    let mut ectx = emb.new_context(
        &backend,
        LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(512))
            .with_embeddings(true)
            .with_pooling_type(LlamaPoolingType::Last),
    )?;
    let etokens = emb.vocab().tokenize(
        "The blacksmith forges iron at dawn.".as_bytes(),
        true,
        false,
    );
    let mut ebatch = LlamaBatch::new(512, 1);
    ebatch.add_sequence(&etokens, 0, false)?;
    ectx.decode(&mut ebatch)?;
    let v = ectx.embeddings_seq_ith(0)?;
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    println!("[embed] dim={} (expect 1024) l2_norm={norm:.4}", v.len());
    print_dxgi("after embedder load");
    Ok(())
}
