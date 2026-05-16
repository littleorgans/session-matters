use std::time::Duration;

use sm_core::SpawnRequest;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedProcess {
    pub runtime_pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildExit {
    pub session_id: String,
    pub runtime_pid: u32,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Error)]
pub enum DriverError {
    #[error(transparent)]
    Nix(#[from] nix::Error),
    #[error("runtime pid out of range: {0}")]
    PidOutOfRange(i32),
    #[error("runtime command contains a null byte")]
    InvalidRuntimeCommand,
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
    fn spawn(
        &self,
        session_id: &str,
        request: &SpawnRequest,
    ) -> Result<SpawnedProcess, DriverError>;

    fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError>;

    fn terminate(
        &self,
        session_id: &str,
        signal: &str,
        grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError>;

    fn nudge(&self, session_id: &str, content: &str) -> Result<NudgeResult, DriverError>;

    fn terminate_all(&self);
}
