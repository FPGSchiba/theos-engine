//! Harness for the behaviour-tree half of the local-LLM proof of concept.
//!
//! This is a developer tool: it reports on stdout, which the engine's `println!` ban
//! (meant for the game binaries) does not apply to.
#![allow(clippy::disallowed_macros)]

mod extract;
mod fixture;
mod prompt;
mod report;
mod run;
mod schema;
mod score;

use std::path::PathBuf;
use std::time::Duration;

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
    #[arg(long, global = true)]
    scenarios: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the scenarios against the model of `models.toml` and write the results tables.
    Run {
        /// Grammar conditions to run.
        #[arg(long, value_enum, default_value_t = run::GrammarModes::Both)]
        grammar: run::GrammarModes,
        /// Thinking conditions to run.
        #[arg(long, value_enum, default_value_t = run::ThinkingModes::Off)]
        thinking: run::ThinkingModes,
        /// One deadline in milliseconds for every request; default is the priority budget.
        #[arg(long)]
        deadline_ms: Option<u64>,
        /// Context size in tokens.
        #[arg(long, default_value_t = 8192)]
        n_ctx: u32,
        /// Thinking tokens allowed to a CombatTactical request.
        #[arg(long, default_value_t = 512)]
        thinking_budget_combat: u32,
        /// Thinking tokens allowed to a WorldEvent request.
        #[arg(long, default_value_t = 2048)]
        thinking_budget_world: u32,
        /// Leave the persona bodies out of NPC prompts.
        #[arg(long)]
        no_persona_bodies: bool,
        /// Run only these scenario ids (a smoke-run filter), e.g. `--only b10,b06`.
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Added to the results directory name.
        #[arg(long)]
        label: Option<String>,
    },
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
        Command::Run {
            grammar,
            thinking,
            deadline_ms,
            n_ctx,
            thinking_budget_combat,
            thinking_budget_world,
            no_persona_bodies,
            only,
            label,
        } => {
            if let Some(id) = only
                .iter()
                .find(|id| !fx.scenarios.iter().any(|s| &s.id == *id))
            {
                return Err(anyhow!("no scenario `{id}`"));
            }
            run::run_cli(
                &fx,
                &run::CliRun {
                    cfg: run::RunConfig {
                        fixture_dir: cli.fixture.clone(),
                        scenarios_file: scenarios,
                        grammar,
                        thinking,
                        deadline: deadline_ms.map(Duration::from_millis),
                        budget_combat: thinking_budget_combat,
                        budget_world: thinking_budget_world,
                        persona_bodies: !no_persona_bodies,
                        only,
                    },
                    n_ctx,
                    label,
                },
            )?;
        }
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
