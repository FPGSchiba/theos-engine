//! Harness for the behaviour-tree half of the local-LLM proof of concept.
//!
//! This is a developer tool: it reports on stdout, which the engine's `println!` ban
//! (meant for the game binaries) does not apply to.
#![allow(clippy::disallowed_macros)]

#[allow(dead_code)] // consumed by the run tasks that follow
mod extract;
mod fixture;
mod prompt;
#[allow(dead_code)] // consumed by the run tasks that follow
mod schema;
#[allow(dead_code)] // consumed by the run tasks that follow
mod score;

use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Parser, Subcommand};
use theos_ai::RecordId;

const BT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/bt-poc");
const LORE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../fixtures/lore-poc");

#[derive(Parser)]
#[command(
    name = "bt-poc",
    about = "Local-LLM behaviour-tree proof-of-concept harness"
)]
struct Cli {
    /// The bt-poc fixture directory.
    #[arg(long, default_value = BT_DIR)]
    fixture: PathBuf,
    /// The lore-poc fixture directory.
    #[arg(long, default_value = LORE_DIR)]
    lore: PathBuf,
    /// The scenarios file; defaults to `scenarios.toml` in the fixture directory.
    #[arg(long)]
    scenarios: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the rendered prompt of one scenario.
    Prompt {
        /// Scenario id, e.g. `b06`.
        scenario: String,
        /// Lore record ids to render (NPC actors only); `all` takes every visible belief.
        #[arg(long = "ids", value_delimiter = ',')]
        ids: Vec<String>,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let scenarios = cli
        .scenarios
        .clone()
        .unwrap_or_else(|| cli.fixture.join("scenarios.toml"));
    let fx = fixture::Fixture::load(&cli.fixture, &cli.lore, &scenarios)?;
    match cli.command {
        Command::Prompt { scenario, ids } => {
            let s = fx
                .scenarios
                .iter()
                .find(|s| s.id == scenario)
                .ok_or_else(|| anyhow!("no scenario `{scenario}`"))?;
            let ids: Vec<RecordId> = if ids.len() == 1 && ids[0] == "all" {
                fx.visible.get(&s.actor).map_or_else(Vec::new, |e| {
                    e.held
                        .iter()
                        .chain(&e.rejected)
                        .map(RecordId::new)
                        .collect()
                })
            } else {
                ids.iter().map(RecordId::new).collect()
            };
            let p = prompt::render(&fx, s, &ids, true).prompt;
            println!("=== system ===\n{}\n=== user ===\n{}", p.system, p.user);
        }
    }
    Ok(())
}
