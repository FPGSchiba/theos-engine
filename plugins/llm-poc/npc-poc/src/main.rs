//! Harness for the local-LLM NPC proof of concept.
//!
//! This is a developer tool: it reports on stdout, which the engine's `println!` ban
//! (meant for the game binaries) does not apply to.
#![allow(clippy::disallowed_macros)]

mod convert;
// Consumed by the `run` command in a later task.
#[allow(dead_code)]
mod scenarios;

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
    /// Run the scenarios against the model (not yet implemented).
    Run,
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
        Command::Run => println!("not yet"),
    }
    Ok(())
}
