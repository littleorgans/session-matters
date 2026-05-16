use clap::{Args, Parser, Subcommand, ValueEnum};
use sm_core::RuntimeKind;

#[derive(Debug, Parser)]
#[command(name = "sm", about = "session-matters control plane")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Daemon(DaemonArgs),
    Run(RunArgs),
    Get(GetArgs),
    #[command(name = "__smd", hide = true)]
    InternalDaemon,
}

#[derive(Debug, Args)]
pub struct DaemonArgs {
    #[command(subcommand)]
    pub action: DaemonAction,
}

#[derive(Debug, Subcommand)]
pub enum DaemonAction {
    Start,
    Stop,
    Status,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    pub runtime: RuntimeKind,
    #[arg(long)]
    pub role: String,
    #[arg(long)]
    pub workspace: String,
    #[arg(long)]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub resource: GetResource,
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GetResource {
    Agents,
}
