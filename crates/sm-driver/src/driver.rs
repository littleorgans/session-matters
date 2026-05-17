use std::time::Duration;

use sm_core::RuntimeKind;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedProcess {
    pub runtime_pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchEnv {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnLaunch {
    pub runtime: RuntimeKind,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NudgeResult {
    pub delivered: bool,
    pub message: String,
}

pub trait SpawnDriver: Send + Sync {
    fn spawn(&self, session_id: &str, launch: &SpawnLaunch) -> Result<SpawnedProcess, DriverError>;

    fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError>;

    fn probe_session(&self, session_id: &str, runtime_pid: u32)
    -> Result<DriverProbe, DriverError>;

    fn terminate(
        &self,
        session_id: &str,
        signal: &str,
        grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError>;

    fn nudge(&self, session_id: &str, content: &str) -> Result<NudgeResult, DriverError>;

    fn terminate_all(&self);
}
