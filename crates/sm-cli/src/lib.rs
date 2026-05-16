pub mod cli;

use clap::Parser;

use cli::cli_def::{Cli, Command};

pub async fn run() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Daemon(args) => cli::daemon::run(args).await,
        Command::Run(args) => cli::run::run(args).await,
        Command::Get(args) => cli::get::run(args).await,
        Command::InternalDaemon => sm_daemon::run_daemon(sm_core::SmPaths::from_env()?).await,
    }
}
