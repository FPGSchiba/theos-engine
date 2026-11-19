//! Harness for the local-LLM NPC proof of concept.
//!
//! This is a developer tool: it reports on stdout, which the engine's `println!` ban
//! (meant for the game binaries) does not apply to.
#![allow(clippy::disallowed_macros)]

mod convert;
mod inject;
mod report;
mod run;
mod scenarios;
mod score;
mod tune;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use serde::Serialize;
use theos_ai::{LoreRecord, Speaker};

#[derive(Parser)]
#[command(name = "npc-poc", about = "Local-LLM NPC proof-of-concept harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert the fixture's content project and write `expected/corpus.json`.
    Export {
        /// Fixture directory; defaults to `fixtures/lore-poc` of this repository.
        #[arg(long)]
        fixture: Option<PathBuf>,
    },
    /// Run the scenarios against every model of `models.toml` and write the results tables.
    Run {
        /// Scenarios file; defaults to `scenarios.toml` of the fixture.
        #[arg(long)]
        scenarios: Option<PathBuf>,
        /// Which thinking modes to run.
        #[arg(long, value_enum, default_value_t = inject::ThinkingModes::Both)]
        thinking: inject::ThinkingModes,
        /// Models file; defaults to `plugins/llm-poc/models.toml`.
        #[arg(long)]
        models: Option<PathBuf>,
        /// Per-request deadline in milliseconds.
        #[arg(long, default_value_t = 3000)]
        deadline_ms: u64,
        /// Leave the persona bodies out of the prompt.
        #[arg(long)]
        no_persona_bodies: bool,
        /// Guards to switch on: `none`, `input` (g1, g4, g9), `all`, or a comma list of
        /// `mod`, `g1`, `g4`, `g9`.
        #[arg(long, default_value = "none")]
        guards: String,
        /// Moderation example lists; defaults to `fixtures/moderation` of this repository.
        #[arg(long)]
        moderation: Option<PathBuf>,
    },
    /// Measure the moderator on the held-out lines and propose thresholds (writes nothing).
    ModerationTune {
        /// Moderation example lists; defaults to `fixtures/moderation` of this repository.
        #[arg(long)]
        moderation: Option<PathBuf>,
        /// Also report this `min_similarity,margin` pair (repeatable), by the same
        /// per-piece rule as the proposal.
        #[arg(long, value_parser = tune::parse_pair)]
        at: Vec<(f32, f32)>,
    },
}

/// The corpus snapshot the `theos-ai` gate test reads.
#[derive(Serialize)]
struct Snapshot<'a> {
    records: &'a [LoreRecord],
    speakers: &'a [Speaker],
}

fn default_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/lore-poc")
}

fn default_moderation() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/moderation")
}

fn export(fixture: &std::path::Path) -> anyhow::Result<PathBuf> {
    let (mut records, mut speakers) = convert::load_fixture(&fixture.join("records"))?;
    records.sort_by(|a, b| a.id.cmp(&b.id));
    speakers.sort_by(|a, b| a.id.cmp(&b.id));
    let json = serde_json::to_string_pretty(&Snapshot {
        records: &records,
        speakers: &speakers,
    })?;
    let out_dir = fixture.join("expected");
    std::fs::create_dir_all(&out_dir)?;
    let out = out_dir.join("corpus.json");
    std::fs::write(&out, json + "\n").with_context(|| format!("writing {}", out.display()))?;
    Ok(out)
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Export { fixture } => {
            let fixture = fixture.unwrap_or_else(default_fixture);
            let out = export(&fixture)?;
            println!("wrote {}", out.display());
        }
        Command::Run {
            scenarios,
            thinking,
            models,
            deadline_ms,
            no_persona_bodies,
            guards,
            moderation,
        } => run::run_cli(
            &default_fixture(),
            scenarios,
            thinking,
            models,
            std::time::Duration::from_millis(deadline_ms),
            !no_persona_bodies,
            guard_kit::Guards::parse(&guards)?,
            &moderation.unwrap_or_else(default_moderation),
        )?,
        Command::ModerationTune { moderation, at } => {
            tune::tune_cli(&moderation.unwrap_or_else(default_moderation), None, &at)?;
        }
    }
    Ok(())
}
