use std::path::PathBuf;

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
    #[command(about = generated_help::DOCTOR_ABOUT, long_about = generated_help::DOCTOR_ABOUT)]
    Doctor(DoctorArgs),
    Mail(MailArgs),
    #[command(about = "Add or remove labels on selected sessions")]
    Label(LabelArgs),
    #[command(about = generated_help::LINK_ABOUT, long_about = generated_help::LINK_ABOUT)]
    Link(LinkArgs),
    #[command(about = generated_help::LOGS_ABOUT, long_about = generated_help::LOGS_ABOUT)]
    Logs(LogsArgs),
    #[command(about = generated_help::WAIT_ABOUT, long_about = generated_help::WAIT_ABOUT)]
    Wait(WaitArgs),
    #[command(about = generated_help::NUDGE_ABOUT, long_about = generated_help::NUDGE_ABOUT)]
    Nudge(NudgeArgs),
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
    #[arg(long = "label", help = "Session label as key=value")]
    pub labels: Vec<String>,
    #[arg(long = "agent-config", help = generated_help::AGENT_RUN_AGENT_CONFIG_HELP)]
    pub agent_config: Option<String>,
    #[arg(long)]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    pub resource: GetResource,
    pub id: Option<String>,
    #[arg(long, help = generated_help::AGENT_LIST_SELECTOR_HELP)]
    pub selector: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum GetResource {
    Agent,
    Agents,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    pub resource: DeleteResource,
    #[arg(help = generated_help::AGENT_DELETE_SELECTOR_HELP)]
    pub selector: String,
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
pub struct DoctorArgs {}

#[derive(Debug, Args)]
pub struct LinkArgs {
    #[arg(long, help = generated_help::LINK_SESSION_ID_HELP)]
    pub session_id: Option<String>,
    #[arg(long, help = generated_help::LINK_SELECTOR_HELP)]
    pub selector: Option<String>,
    #[arg(long = "runtime-session", help = generated_help::LINK_RUNTIME_SESSION_HELP)]
    pub runtime_session: String,
    #[arg(long, help = generated_help::LINK_TRANSCRIPT_HELP)]
    pub transcript: PathBuf,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(help = generated_help::LOGS_SELECTOR_HELP)]
    pub selector: String,
    #[arg(short = 'f', long, help = generated_help::LOGS_FOLLOW_HELP)]
    pub follow: bool,
    #[arg(long = "max-bytes", help = generated_help::LOGS_MAX_BYTES_HELP)]
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Args)]
pub struct WaitArgs {
    #[arg(help = generated_help::WAIT_SELECTOR_HELP)]
    pub selector: String,
    #[arg(long = "for", help = generated_help::WAIT_FOR_HELP)]
    pub condition: String,
    #[arg(long, default_value_t = 30, help = generated_help::WAIT_TIMEOUT_SECS_HELP)]
    pub timeout_secs: u64,
}

#[derive(Debug, Args)]
pub struct MailArgs {
    #[command(subcommand)]
    pub action: MailAction,
}

#[derive(Debug, Subcommand)]
pub enum MailAction {
    #[command(about = generated_help::MAIL_SEND_ABOUT, long_about = generated_help::MAIL_SEND_ABOUT)]
    Send(MailSendArgs),
    #[command(about = generated_help::MAIL_READ_ABOUT, long_about = generated_help::MAIL_READ_ABOUT)]
    Read(MailReadArgs),
    #[command(about = generated_help::MAIL_CHECK_ABOUT, long_about = generated_help::MAIL_CHECK_ABOUT)]
    Check(MailCheckArgs),
    #[command(name = "stop-check", about = generated_help::MAIL_STOP_CHECK_ABOUT, long_about = generated_help::MAIL_STOP_CHECK_ABOUT)]
    StopCheck(MailStopCheckArgs),
}

#[derive(Debug, Args)]
pub struct MailSendArgs {
    #[arg(long, help = generated_help::MAIL_SEND_TO_HELP)]
    pub to: String,
    #[arg(long, help = generated_help::MAIL_SEND_FROM_HELP)]
    pub from: Option<String>,
    #[arg(long, help = generated_help::MAIL_SEND_CONTENT_HELP)]
    pub content: String,
}

#[derive(Debug, Args)]
pub struct MailReadArgs {
    #[arg(long, alias = "from", help = generated_help::MAIL_READ_SELECTOR_HELP)]
    pub selector: String,
    #[arg(long, help = generated_help::MAIL_READ_PEEK_HELP)]
    pub peek: bool,
}

#[derive(Debug, Args)]
pub struct MailCheckArgs {
    #[arg(long, alias = "from", help = generated_help::MAIL_CHECK_SELECTOR_HELP)]
    pub selector: String,
}

#[derive(Debug, Args)]
pub struct MailStopCheckArgs {
    #[arg(long, alias = "from", help = generated_help::MAIL_STOP_CHECK_SELECTOR_HELP)]
    pub selector: String,
}

#[derive(Debug, Args)]
pub struct LabelArgs {
    pub selector: String,
    pub mutation: String,
}

#[derive(Debug, Args)]
pub struct NudgeArgs {
    #[arg(long, help = generated_help::NUDGE_TO_HELP)]
    pub to: String,
    #[arg(long, help = generated_help::NUDGE_CONTENT_HELP)]
    pub content: String,
}

#[derive(Debug, Args)]
pub struct McpArgs {}
