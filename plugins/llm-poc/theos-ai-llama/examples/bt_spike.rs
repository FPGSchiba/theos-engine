//! Spike: can Qwen3.5-4B write a behaviour tree under a recursive JSON schema
//! (llguidance), and how long does it take? Scenario b10 (thorn archer).
//! A check, not product code.

// The example prints its results; the repo-level clippy.toml bans `println!`.
#![allow(clippy::disallowed_macros)]

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Instant;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaChatMessage, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde_json::{json, Value};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

const MODES: [&str; 10] = [
    "Idle",
    "Patrol",
    "Hold",
    "Advance",
    "Charge",
    "Kite",
    "Flee",
    "TakeCover",
    "Regroup",
    "Follow",
];
const STYLES: [&str; 4] = ["None", "Melee", "Ranged", "Heavy"];
/// b10's live entities.
const HANDLES: [&str; 2] = ["player", "ridge"];

fn fixture(rel: &str) -> Res<String> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/bt-poc")
        .join(rel);
    Ok(std::fs::read_to_string(p)?)
}

fn num(min: f64, max: f64) -> Value {
    json!({"type": "number", "minimum": min, "maximum": max})
}

fn int(min: i64, max: i64) -> Value {
    json!({"type": "integer", "minimum": min, "maximum": max})
}

/// An object with exactly these properties, in this order.
fn obj(props: Vec<(&str, Value)>) -> Value {
    let required: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
    let properties: serde_json::Map<String, Value> =
        props.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
    json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false})
}

fn tag(name: &str) -> Value {
    json!({"const": name})
}

fn set_value(component: &str, field: &str, values: &[&str]) -> Value {
    obj(vec![
        ("type", tag("Action")),
        ("command", tag("SetComponentValue")),
        (
            "params",
            obj(vec![
                ("component", tag(component)),
                ("field", tag(field)),
                ("value", json!({"enum": values})),
            ]),
        ),
    ])
}

fn condition(check: &str, params: Vec<(&str, Value)>) -> Value {
    obj(vec![
        ("type", tag("Condition")),
        ("check", tag(check)),
        ("params", obj(params)),
    ])
}

fn node_ref() -> Value {
    json!({"$ref": "#/$defs/node"})
}

fn children() -> Value {
    json!({"type": "array", "items": node_ref(), "minItems": 1})
}

/// The recursive schema for b10: envelope plus `$defs.node`.
fn schema() -> Value {
    let mut targets: Vec<&str> = HANDLES.to_vec();
    targets.push("none");
    let node = json!({"anyOf": [
        obj(vec![("type", tag("Sequence")), ("children", children())]),
        obj(vec![("type", tag("Selector")), ("children", children())]),
        obj(vec![
            ("type", tag("Parallel")),
            ("policy", json!({"enum": ["all", "one"]})),
            ("children", children()),
        ]),
        obj(vec![("type", tag("Decorator_Repeat")), ("count", int(1, 10)), ("child", node_ref())]),
        obj(vec![("type", tag("Decorator_Invert")), ("child", node_ref())]),
        obj(vec![("type", tag("Decorator_Timeout")), ("seconds", num(0.5, 30.0)), ("child", node_ref())]),
        obj(vec![("type", tag("Wait")), ("seconds", num(0.5, 30.0))]),
        condition("health_below", vec![("threshold", num(0.0, 1.0))]),
        condition(
            "entity_in_range",
            vec![("target_entity", json!({"enum": HANDLES})), ("radius", num(0.0, 100.0))],
        ),
        condition("allies_in_range", vec![("radius", num(0.0, 100.0)), ("at_least", int(1, 10))]),
        condition("time_since_install_over", vec![("seconds", num(0.5, 60.0))]),
        set_value("MovementState", "mode", &MODES),
        set_value("Target", "entity", &targets),
        set_value("Destination", "entity", &targets),
        set_value("Attack", "style", &STYLES),
    ]});
    let mut root = obj(vec![
        ("sources", json!({"type": "array", "maxItems": 0})),
        (
            "tree",
            obj(vec![
                (
                    "name",
                    json!({"type": "string", "minLength": 1, "maxLength": 80}),
                ),
                ("root", node_ref()),
            ]),
        ),
    ]);
    root["$defs"] = json!({"node": node});
    root
}

fn matcher_for(factory: &llguidance::ParserFactory, schema: &Value) -> Res<llguidance::Matcher> {
    let grammar = llguidance::api::TopLevelGrammar::from_json_schema(schema.clone());
    let parser = factory.create_parser(grammar)?;
    let m = llguidance::Matcher::new(Ok(parser));
    if let Some(e) = m.get_error() {
        return Err(e.into());
    }
    Ok(m)
}

fn guided(m: llguidance::Matcher) -> LlamaSampler {
    // The llguidance sampler only masks logits; chain a selector after it.
    LlamaSampler::chain_simple([LlamaSampler::from(m), LlamaSampler::greedy()])
}

struct Run {
    text: String,
    prompt_tokens: usize,
    ttft: f64,
    total: f64,
    thinking_tokens: usize,
    answer_tokens: usize,
    peak_ctx: usize,
    ctx_full: bool,
}

/// Decode the prompt and sample until end-of-generation, the grammar's end, or a full context.
/// Thinking on: unconstrained until `</think>`, then "\n\n", then a fresh grammar.
fn run(
    model: &LlamaModel,
    backend: &LlamaBackend,
    factory: &llguidance::ParserFactory,
    prompt: &str,
    n_ctx: u32,
    thinking: bool,
    schema: &Value,
) -> Res<Run> {
    let mut ctx = model.new_context(
        backend,
        LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(n_ctx))
            .with_n_batch(n_ctx),
    )?;
    let vocab = model.vocab();
    let tokens = vocab.tokenize(prompt.as_bytes(), true, true);
    let n_prompt = tokens.len();
    let mut batch = LlamaBatch::new(n_ctx as usize, 1);
    for (i, t) in tokens.iter().enumerate() {
        batch.add(*t, i as i32, &[0], i == n_prompt - 1)?;
    }
    let separator = vocab.tokenize(b"\n\n", false, false);
    let start = Instant::now();
    ctx.decode(&mut batch)?;
    let mut pos = n_prompt as i32;
    let mut idx = batch.n_tokens() - 1;

    let mut in_thinking = thinking;
    let mut sampler = if thinking {
        LlamaSampler::chain_simple([
            LlamaSampler::top_k(20),
            LlamaSampler::top_p(0.95, 1),
            LlamaSampler::temp(0.6),
            LlamaSampler::dist(1234),
        ])
    } else {
        guided(matcher_for(factory, schema)?)
    };
    let mut think_text = Vec::new();
    let mut out = Vec::new();
    let (mut ttft, mut n_think, mut n_ans) = (0.0, 0usize, 0usize);
    let mut ctx_full = false;
    let mut first = true;
    loop {
        if pos as u32 + 1 >= n_ctx {
            ctx_full = true;
            break;
        }
        // `sample` also accepts the token into the chain.
        let tok = sampler.sample(&ctx, idx);
        if first {
            ttft = start.elapsed().as_secs_f64();
            first = false;
        }
        if vocab.is_eog(tok) {
            break;
        }
        let piece = vocab.token_to_piece(tok, true, None);
        let mut next = vec![tok];
        if in_thinking {
            n_think += 1;
            think_text.extend_from_slice(&piece);
            if String::from_utf8_lossy(&think_text)
                .trim_end()
                .ends_with("</think>")
            {
                in_thinking = false;
                next.extend_from_slice(&separator);
                sampler = guided(matcher_for(factory, schema)?);
            }
        } else {
            n_ans += 1;
            out.extend_from_slice(&piece);
        }
        batch.clear();
        for (i, t) in next.iter().enumerate() {
            batch.add(*t, pos + i as i32, &[0], i == next.len() - 1)?;
        }
        pos += next.len() as i32;
        idx = batch.n_tokens() - 1;
        if ctx.decode(&mut batch).is_err() {
            ctx_full = true;
            break;
        }
    }
    Ok(Run {
        text: String::from_utf8_lossy(&out).into_owned(),
        prompt_tokens: n_prompt,
        ttft,
        total: start.elapsed().as_secs_f64(),
        thinking_tokens: n_think,
        answer_tokens: n_ans,
        peak_ctx: pos as usize,
        ctx_full,
    })
}

/// (node count, max depth) of a node; the root is depth 1.
fn measure(node: &Value, depth: usize) -> (usize, usize) {
    let mut count = 1;
    let mut max = depth;
    let mut kids: Vec<&Value> = Vec::new();
    if let Some(c) = node.get("children").and_then(Value::as_array) {
        kids.extend(c);
    }
    if let Some(c) = node.get("child") {
        kids.push(c);
    }
    for k in kids {
        let (c, d) = measure(k, depth + 1);
        count += c;
        max = max.max(d);
    }
    (count, max)
}

fn report(label: &str, r: &Run) {
    let gen_tokens = r.thinking_tokens + r.answer_tokens;
    let tps = if r.total > r.ttft {
        gen_tokens.saturating_sub(1) as f64 / (r.total - r.ttft)
    } else {
        0.0
    };
    println!(
        "[{label}] prompt_tokens={} ttft={:.3}s total={:.2}s thinking_tokens={} output_tokens={} tok/s={tps:.1} peak_ctx={} ctx_full={}",
        r.prompt_tokens, r.ttft, r.total, r.thinking_tokens, r.answer_tokens, r.peak_ctx, r.ctx_full
    );
    match serde_json::from_str::<Value>(&r.text) {
        Ok(v) => match v.pointer("/tree/root") {
            Some(root) => {
                let (n, d) = measure(root, 1);
                println!("[{label}] parses=true nodes={n} max_depth={d}");
            }
            None => println!("[{label}] parses=true but no tree.root"),
        },
        Err(e) => println!("[{label}] parses=false ({e})"),
    }
}

fn main() -> Res<()> {
    let models: PathBuf = std::env::var_os("THEOS_MODELS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../models"));
    let backend = LlamaBackend::init()?;
    let model = LlamaModel::load_from_file(
        &backend,
        models.join("Qwen3.5-4B-Q4_K_M.gguf"),
        &LlamaModelParams::default().with_n_gpu_layers(u32::MAX),
    )?;
    let env = LlamaSampler::llguidance_tok_env(&model);
    let factory = llguidance::ParserFactory::new_simple(&env)?;

    // 1. Does llguidance compile the recursive schema?
    let schema = schema();
    match matcher_for(&factory, &schema) {
        Ok(_) => println!("[schema] llguidance compiles the recursive schema: yes"),
        Err(e) => {
            println!("[schema] llguidance compiles the recursive schema: NO: {e}");
            return Err(e);
        }
    }
    // 2. Does it accept the reference tree? Feed its tokens through a fresh matcher.
    let reference: Value = serde_json::from_str(&fixture("reference/b10-good.json")?)?;
    let reference_text = serde_json::to_string(&reference["reply"])?;
    let mut m = matcher_for(&factory, &schema)?;
    let mut accepted = true;
    for t in model
        .vocab()
        .tokenize(reference_text.as_bytes(), false, false)
    {
        if m.consume_token(t.0 as u32).is_err() {
            accepted = false;
            break;
        }
    }
    println!(
        "[schema] accepts reference/b10-good.json: {}",
        accepted && m.is_accepting().unwrap_or(false)
    );

    // 3. The prompt.
    let record = fixture("records/enemies/fx-thorn-archer.md")?;
    let body = record.rsplit("---").next().unwrap_or("").trim();
    let vocab_json: Value = serde_json::from_str(&fixture("vocabulary.json")?)?;
    let examples: Value = serde_json::from_str(&fixture("prompt-examples.json")?)?;
    let system = format!(
        "You are the behaviour designer for Thorn archer, an enemy in a fantasy game. Its design:\n{body}\n\n\
         Situation now: health 1.0; no flags set. Entities: player (kind player, 25 m away), ridge (kind place, 0 m away).\n\n\
         Reply with one JSON object {{\"sources\": [], \"tree\": {{\"name\": ..., \"root\": ...}}}}. The tree is ticked every 0.5 s from the root.\n\
         Vocabulary:\n{}\n\nWorked example:\n{}",
        serde_json::to_string_pretty(&vocab_json)?,
        serde_json::to_string_pretty(&examples[0])?,
    );
    let user = "Hold the ridge and shoot the player.";
    let chat = [
        LlamaChatMessage::new("system".to_owned(), system)?,
        LlamaChatMessage::new("user".to_owned(), user.to_owned())?,
    ];
    let template = model.chat_template(None)?;
    let templated = model.apply_chat_template(&template, &chat, true)?;
    let off_prompt = format!("{templated}<think>\n\n</think>\n\n");
    let on_prompt = format!("{templated}<think>\n");

    // 4. Thinking off.
    let r = run(
        &model,
        &backend,
        &factory,
        &off_prompt,
        8192,
        false,
        &schema,
    )?;
    report("think-off n_ctx=8192", &r);
    println!("[think-off] fits 8192: {}", !r.ctx_full);

    // 5. Thinking on, grammar on after </think>.
    let r = run(&model, &backend, &factory, &on_prompt, 8192, true, &schema)?;
    report("think-on n_ctx=8192", &r);
    println!("[think-on] fits 8192: {}", !r.ctx_full);
    if r.ctx_full {
        let r = run(&model, &backend, &factory, &on_prompt, 16384, true, &schema)?;
        report("think-on n_ctx=16384", &r);
        println!("[think-on] fits 16384: {}", !r.ctx_full);
    }
    Ok(())
}
