pub mod cli;
pub mod mcp;
pub mod tool_contracts;
pub mod tool_docs;
pub mod tool_examples;

use clap::Parser;

use cli::cli_def::{Cli, Command};

pub const VERSION: &str = env!("SM_CLI_VERSION");

pub async fn run() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Daemon(args) => cli::daemon::run(args).await,
        Command::Run(args) => cli::run::run(args).await,
        Command::Get(args) => cli::get::run(args).await,
        Command::Delete(args) => cli::delete::run(args).await,
        Command::Doctor(args) => cli::doctor::run(args).await,
        Command::Mail(args) => cli::mail::run(args).await,
        Command::Label(args) => cli::label::run(args).await,
        Command::Link(args) => cli::link::run(args).await,
        Command::Logs(args) => cli::logs::run(args).await,
        Command::Wait(args) => cli::wait::run(args).await,
        Command::Nudge(args) => cli::nudge::run(args).await,
        Command::Mcp(args) => cli::mcp::run(args).await,
        Command::InternalDaemon => sm_daemon::run_daemon(sm_core::SmPaths::from_env()?).await,
    }
}
