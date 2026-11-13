//! Tests against the real GGUF model on the GPU. They need the model files named in
//! `plugins/llm-poc/models.toml` in the models directory (`THEOS_MODELS_DIR`).
//!
//! `bt-poc` is a binary crate, so the modules under test are included by path.
#![allow(dead_code)]

#[path = "../src/fixture.rs"]
mod fixture;
#[path = "../src/schema.rs"]
mod schema;

use std::path::Path;

use theos_ai::RecordId;
use theos_ai_llama::{models_dir, LlamaGenerator, LlamaRuntime, ModelsFile};

#[test]
fn schema_compiles_in_llguidance_for_every_scenario() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures");
    let bt = root.join("bt-poc");
    let fx = fixture::Fixture::load(&bt, &root.join("lore-poc"), &bt.join("scenarios.toml"))
        .expect("fixture loads");

    let models = ModelsFile::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../models.toml"))
        .expect("models.toml parses");
    let m = &models.models[0];
    let path = models_dir().join(&m.file);
    assert!(
        path.is_file(),
        "model file {} is missing: download it into the models directory, or set \
         THEOS_MODELS_DIR (it is {:?})",
        path.display(),
        std::env::var_os("THEOS_MODELS_DIR"),
    );
    let rt = LlamaRuntime::init().expect("llama.cpp backend init");
    let generator = LlamaGenerator::load(rt, &path, m.profile.clone()).expect("load generator");

    assert_eq!(fx.scenarios.len(), 20);
    for s in &fx.scenarios {
        let ids: Vec<RecordId> = fx.visible.get(&s.actor).map_or_else(Vec::new, |e| {
            e.held
                .iter()
                .chain(&e.rejected)
                .map(RecordId::new)
                .collect()
        });
        let schema = schema::tree_schema(&fx, s, &ids);
        generator
            .compile_schema(&schema)
            .unwrap_or_else(|e| panic!("{}: llguidance rejects the schema: {e}", s.id));
    }
}
