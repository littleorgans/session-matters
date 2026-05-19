use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use lilo_rm_client::ClientError;
use sm_core::RuntimeKind;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedProcess {
    pub runtime_pid: u32,
    pub log_dir: Option<PathBuf>,
    pub stdout_path: Option<PathBuf>,
    pub stderr_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchEnv {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnLaunch {
    pub runtime: RuntimeKind,
    pub cwd: PathBuf,
    pub env: Vec<LaunchEnv>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildExit {
    pub session_id: String,
    pub runtime_pid: u32,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverProbe {
    pub verified: bool,
    pub evidence: String,
}

#[derive(Debug, Error)]
pub enum DriverError {
    #[error(transparent)]
    Nix(#[from] nix::Error),
    #[error("runtime pid out of range: {0}")]
    PidOutOfRange(i32),
    #[error("stored runtime pid out of range: {0}")]
    StoredPidOutOfRange(u32),
    #[error("runtime command contains a null byte")]
    InvalidRuntimeCommand,
    #[error("launch environment contains a null byte")]
    InvalidEnvironment,
    #[error("unsupported signal: {0}")]
    InvalidSignal(String),
    #[error("runtime process did not terminate after SIGKILL")]
    TerminationTimeout,
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("invalid runtime session id: {0}")]
    InvalidSessionId(String),
    #[error("runtime session has no runtime pid: {0}")]
    MissingRuntimePid(String),
    #[error("unsupported driver operation {operation}; scheduled for {pass}")]
    Unsupported {
        operation: &'static str,
        pass: &'static str,
    },
    #[error("unknown runtime variant: {variant}")]
    UnknownRuntimeVariant { variant: String },
    #[error("runtime spawn conflict: {0}")]
    SpawnConflict(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeResult {
    pub delivered: bool,
    pub message: String,
}

#[async_trait]
pub trait SpawnDriver: Send + Sync {
    async fn spawn(
        &self,
        session_id: &str,
        launch: &SpawnLaunch,
    ) -> Result<SpawnedProcess, DriverError>;

    async fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError>;

    async fn probe_session(
        &self,
        session_id: &str,
        runtime_pid: u32,
    ) -> Result<DriverProbe, DriverError>;

    async fn terminate(
        &self,
        session_id: &str,
        signal: &str,
        grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError>;

    async fn nudge(&self, session_id: &str, content: &str) -> Result<NudgeResult, DriverError>;

    fn terminate_all(&self);
}
