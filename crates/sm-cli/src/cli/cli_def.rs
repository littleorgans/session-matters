use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use sm_core::{Namespace, RuntimeKind};

use crate::cli::generated_help;
use crate::cli::selector_scope::NamespaceScopeArgs;

#[derive(Debug, Parser)]
#[command(
    name = "sm",
    display_name = "session-matters",
    about = "session-matters control plane",
    version = crate::VERSION,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Daemon(DaemonArgs),
    #[command(about = generated_help::AGENT_RUN_ABOUT, long_about = generated_help::AGENT_RUN_ABOUT)]
    Run(RunArgs),
    #[command(about = "Create namespace records")]
    Create(CreateArgs),
    #[command(about = "Manage user configuration")]
    Config(ConfigArgs),
    #[command(about = "Inspect sessions and namespaces")]
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
    #[command(about = generated_help::AGENT_CAPTURE_ABOUT, long_about = generated_help::AGENT_CAPTURE_ABOUT)]
    Capture(CaptureArgs),
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
    #[arg(long, help = generated_help::AGENT_RUN_DIR_HELP)]
    pub dir: Option<PathBuf>,
    #[arg(long, help = generated_help::AGENT_RUN_NAMESPACE_HELP)]
    pub namespace: Option<Namespace>,
    #[arg(long = "label", help = "Session label as key=value")]
    pub labels: Vec<String>,
    #[arg(long = "agent-config", help = generated_help::AGENT_RUN_AGENT_CONFIG_HELP)]
    pub agent_config: Option<String>,
    #[arg(long, default_value = "headless", help = generated_help::AGENT_RUN_TARGET_HELP)]
    pub target: String,
    #[arg(long)]
    pub detach: bool,
}

#[derive(Debug, Args)]
pub struct GetArgs {
    #[command(subcommand)]
    pub resource: GetResource,
}

#[derive(Debug, Subcommand)]
pub enum GetResource {
    #[command(about = generated_help::SESSION_GET_ABOUT, long_about = generated_help::SESSION_GET_ABOUT)]
    Session(SessionGetArgs),
    #[command(about = "List session records known to the session-matters daemon")]
    Sessions(SessionListArgs),
    #[command(about = "Get one namespace record by slug")]
    Namespace(NamespaceGetArgs),
    #[command(about = "List namespace records")]
    Namespaces(NamespaceListArgs),
}

#[derive(Debug, Args)]
pub struct SessionGetArgs {
    pub id: Option<String>,
    #[command(flatten)]
    pub read: SessionReadArgs,
}

#[derive(Debug, Args)]
pub struct SessionListArgs {
    #[command(flatten)]
    pub read: SessionReadArgs,
}

#[derive(Debug, Args)]
pub struct SessionReadArgs {
    #[arg(long, help = generated_help::AGENT_LIST_SELECTOR_HELP)]
    pub selector: Option<String>,
    #[command(flatten)]
    pub scope: NamespaceScopeArgs,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct NamespaceGetArgs {
    pub slug: Option<String>,
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct NamespaceListArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct CreateArgs {
    #[command(subcommand)]
    pub resource: CreateResource,
}

#[derive(Debug, Subcommand)]
pub enum CreateResource {
    #[command(about = "Create a namespace before spawning sessions into it")]
    Namespace(NamespaceCreateArgs),
}

#[derive(Debug, Args)]
pub struct NamespaceCreateArgs {
    pub slug: String,
}

#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    #[command(about = "Set the current namespace context")]
    SetContext(SetContextArgs),
}

#[derive(Debug, Args)]
pub struct SetContextArgs {
    pub namespace: Namespace,
}

#[derive(Debug, Args)]
pub struct DeleteArgs {
    #[command(subcommand)]
    pub resource: DeleteResource,
}

#[derive(Debug, Subcommand)]
pub enum DeleteResource {
    #[command(alias = "sessions")]
    Session(DeleteSessionArgs),
    Namespace(DeleteNamespaceArgs),
}

#[derive(Debug, Args)]
pub struct DeleteSessionArgs {
    #[arg(help = generated_help::AGENT_DELETE_SELECTOR_HELP)]
    pub selector: String,
    #[command(flatten)]
    pub scope: NamespaceScopeArgs,
    #[arg(long, default_value = "SIGTERM", help = generated_help::AGENT_DELETE_SIGNAL_HELP)]
    pub signal: String,
    #[arg(long, default_value_t = 5, help = generated_help::AGENT_DELETE_GRACE_SECS_HELP)]
    pub grace: u64,
}

#[derive(Debug, Args)]
pub struct DeleteNamespaceArgs {
    pub namespace: Namespace,
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
pub struct CaptureArgs {
    #[arg(long, help = generated_help::AGENT_CAPTURE_SELECTOR_HELP)]
    pub selector: String,
    #[arg(long = "scrollback-lines", help = generated_help::AGENT_CAPTURE_SCROLLBACK_LINES_HELP)]
    pub scrollback_lines: Option<u32>,
    #[arg(long)]
    pub json: bool,
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
    #[command(flatten)]
    pub scope: NamespaceScopeArgs,
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
    #[command(flatten)]
    pub scope: NamespaceScopeArgs,
    pub mutation: String,
}

#[derive(Debug, Args)]
pub struct NudgeArgs {
    #[arg(long, help = generated_help::NUDGE_TO_HELP)]
    pub to: String,
    #[command(flatten)]
    pub scope: NamespaceScopeArgs,
    #[arg(long, help = generated_help::NUDGE_CONTENT_HELP)]
    pub content: String,
}

#[derive(Debug, Args)]
pub struct McpArgs {}
