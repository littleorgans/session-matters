use clap::{Args, Parser, Subcommand, ValueEnum};
use sm_core::RuntimeKind;

use crate::cli::generated_help;

#[derive(Debug, Parser)]
#[command(name = "sm", about = "session-matters control plane")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Daemon(DaemonArgs),
    #[command(about = generated_help::AGENT_RUN_ABOUT, long_about = generated_help::AGENT_RUN_ABOUT)]
    Run(RunArgs),
    Get(GetArgs),
    Delete(DeleteArgs),
    #[command(about = "Bridge MCP stdio to the session-matters daemon")]
    Mcp(McpArgs),
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
    #[arg(help = generated_help::AGENT_RUN_RUNTIME_HELP)]
    pub runtime: RuntimeKind,
    #[arg(long, help = generated_help::AGENT_RUN_ROLE_HELP)]
    pub role: String,
    #[arg(long, help = generated_help::AGENT_RUN_WORKSPACE_HELP)]
    pub workspace: String,
    #[arg(long)]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub resource: GetResource,
    #[arg(long, help = generated_help::AGENT_LIST_ID_HELP)]
    pub id: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GetResource {
    Agents,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub resource: DeleteResource,
    #[arg(help = generated_help::AGENT_DELETE_ID_HELP)]
    pub id: String,
    #[arg(long, default_value = "SIGTERM", help = generated_help::AGENT_DELETE_SIGNAL_HELP)]
    pub signal: String,
    #[arg(long, default_value_t = 5, help = generated_help::AGENT_DELETE_GRACE_SECS_HELP)]
    pub grace: u64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DeleteResource {
    Agent,
}

#[derive(Debug, Args)]
pub struct McpArgs {}
