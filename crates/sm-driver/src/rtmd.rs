use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lilo_rm_client::{ClientError, RuntimeClient};
use lilo_rm_core::{
    HeadlessSpawnTarget, KillOutcome, KillRequest, Lifecycle, LifecycleState,
    RuntimeKind as RtmdRuntimeKind, RuntimeSignal, SpawnRequest, SpawnTarget as RtmdSpawnTarget,
    StatusFilter,
};
use sm_core::RuntimeKind;
use tokio::time::{Instant, sleep};
use uuid::Uuid;

use crate::conv::{lifecycle_to_probe, lifecycle_transcript_path};
use crate::driver::{
    ChildExit, DriverError, DriverProbe, NudgeResult, SpawnDriver, SpawnLaunch, SpawnedProcess,
};

#[derive(Clone, Debug)]
pub struct RtmdDriver {
    client: RuntimeClient,
    terminal_sessions: Arc<Mutex<HashSet<Uuid>>>,
}

impl RtmdDriver {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            client: RuntimeClient::new(socket_path),
            terminal_sessions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn client(&self) -> &RuntimeClient {
        &self.client
    }
}

#[async_trait]
impl SpawnDriver for RtmdDriver {
    async fn spawn(
        &self,
        session_id: &str,
        launch: &SpawnLaunch,
    ) -> Result<SpawnedProcess, DriverError> {
        let session_id = parse_session_id(session_id)?;
        self.terminal_sessions
            .lock()
            .expect("terminal sessions lock poisoned")
            .remove(&session_id);
        let payload = self
            .client
            .spawn(SpawnRequest {
                session_id,
                runtime: runtime_kind(launch.runtime),
                env: launch
                    .env
                    .iter()
                    .map(|item| lilo_rm_core::LaunchEnv {
                        key: item.key.clone(),
                        value: item.value.clone(),
                    })
                    .collect(),
                cwd: launch.cwd.clone(),
                target: RtmdSpawnTarget::Headless(HeadlessSpawnTarget {}),
                force: false,
                shell_resume: None,
            })
            .await
            .map_err(spawn_error)?;
        let runtime_pid = runtime_pid(&payload.lifecycle)?;

        Ok(SpawnedProcess {
            runtime_pid,
            log_dir: payload.log_dir,
            stdout_path: payload.stdout_path,
            stderr_path: payload.stderr_path,
        })
    }

    async fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError> {
        let payload = self.client.status(StatusFilter::empty()).await?;
        let mut terminal_sessions = self
            .terminal_sessions
            .lock()
            .expect("terminal sessions lock poisoned");
        let mut exits = Vec::new();
        for lifecycle in payload.lifecycles {
            if let Some(exit) = terminal_child_exit(&lifecycle)?
                && terminal_sessions.insert(lifecycle.session_id)
            {
                exits.push(exit);
            }
        }
        Ok(exits)
    }

    async fn probe_session(
        &self,
        session_id: &str,
        runtime_pid: u32,
    ) -> Result<DriverProbe, DriverError> {
        let session_id = parse_session_id(session_id)?;
        let payload = self.client.status(status_session(session_id)).await?;
        let Some(lifecycle) = payload
            .lifecycles
            .iter()
            .find(|lifecycle| lifecycle.session_id == session_id)
        else {
            return Ok(DriverProbe {
                verified: false,
                evidence: format!("rtmd has no lifecycle for session {session_id}"),
                transcript_path: None,
            });
        };
        lifecycle_to_probe(lifecycle, runtime_pid)
    }

    async fn terminate(
        &self,
        session_id: &str,
        signal: &str,
        grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError> {
        let session_id = parse_session_id(session_id)?;
        let signal = RuntimeSignal::from_str(signal)
            .map_err(|_| DriverError::InvalidSignal(signal.to_string()))?;
        let outcome = self
            .client
            .kill(KillRequest {
                session_id,
                signal,
                grace_secs: grace.as_secs(),
            })
            .await?;

        let exit = match outcome {
            KillOutcome::Signalled | KillOutcome::AlreadyExited => {
                self.wait_for_terminal(session_id, grace).await?
            }
            _ => {
                return Err(DriverError::UnknownRuntimeVariant {
                    variant: format!("{outcome:?}"),
                });
            }
        };
        if exit.is_some() {
            self.terminal_sessions
                .lock()
                .expect("terminal sessions lock poisoned")
                .insert(session_id);
        }
        Ok(exit)
    }

    async fn nudge(&self, _session_id: &str, _content: &str) -> Result<NudgeResult, DriverError> {
        Err(DriverError::Unsupported {
            operation: "nudge",
            pass: "Pass 5",
        })
    }

    fn terminate_all(&self) {}
}

impl RtmdDriver {
    async fn wait_for_terminal(
        &self,
        session_id: Uuid,
        grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError> {
        let timeout = grace.max(Duration::from_secs(1));
        let deadline = Instant::now() + timeout;
        loop {
            let payload = self.client.status(status_session(session_id)).await?;
            let exit = payload
                .lifecycles
                .iter()
                .find(|lifecycle| lifecycle.session_id == session_id)
                .map(terminal_child_exit)
                .transpose()?
                .flatten();
            if exit.is_some() || Instant::now() >= deadline {
                return Ok(exit);
            }
            sleep(Duration::from_millis(100)).await;
        }
    }
}

fn parse_session_id(session_id: &str) -> Result<Uuid, DriverError> {
    Uuid::parse_str(session_id).map_err(|_| DriverError::InvalidSessionId(session_id.to_string()))
}

fn runtime_kind(runtime: RuntimeKind) -> RtmdRuntimeKind {
    match runtime {
        RuntimeKind::Claude => RtmdRuntimeKind::Claude,
        RuntimeKind::Codex => RtmdRuntimeKind::Codex,
    }
}

fn runtime_pid(lifecycle: &Lifecycle) -> Result<u32, DriverError> {
    lifecycle
        .runtime_pid
        .ok_or_else(|| DriverError::MissingRuntimePid(lifecycle.session_id.to_string()))
}

fn terminal_child_exit(lifecycle: &Lifecycle) -> Result<Option<ChildExit>, DriverError> {
    let exit_code = match lifecycle.state {
        LifecycleState::Forking | LifecycleState::Running => return Ok(None),
        LifecycleState::Exited(exit) => exit.code,
        LifecycleState::Lost(_) => None,
        _ => {
            return Err(DriverError::UnknownRuntimeVariant {
                variant: format!("{:?}", lifecycle.state),
            });
        }
    };
    Ok(Some(ChildExit {
        session_id: lifecycle.session_id.to_string(),
        runtime_pid: lifecycle.runtime_pid.unwrap_or_default(),
        exit_code,
        transcript_path: lifecycle_transcript_path(lifecycle),
    }))
}

fn status_session(session_id: Uuid) -> StatusFilter {
    StatusFilter {
        session_id: Some(session_id),
        session_ids: Vec::new(),
        updated_since: None,
        runtime: None,
        state: None,
    }
}

fn spawn_error(error: ClientError) -> DriverError {
    match error {
        ClientError::SpawnConflict(payload) => DriverError::SpawnConflict(format!("{payload:?}")),
        other => DriverError::Client(other),
    }
}
