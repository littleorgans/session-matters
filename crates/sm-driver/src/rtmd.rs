use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use lilo_rm_client::{ClientError, RuntimeClient};
use lilo_rm_core::{
    HeadlessSpawnTarget, Lifecycle, RuntimeKind as RtmdRuntimeKind, SpawnRequest,
    SpawnTarget as RtmdSpawnTarget, StatusFilter,
};
use sm_core::RuntimeKind;
use uuid::Uuid;

use crate::conv::lifecycle_to_probe;
use crate::driver::{
    ChildExit, DriverError, DriverProbe, NudgeResult, SpawnDriver, SpawnLaunch, SpawnedProcess,
};

#[derive(Clone, Debug)]
pub struct RtmdDriver {
    client: RuntimeClient,
}

impl RtmdDriver {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            client: RuntimeClient::new(socket_path),
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
        Ok(Vec::new())
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
            });
        };
        lifecycle_to_probe(lifecycle, runtime_pid)
    }

    async fn terminate(
        &self,
        _session_id: &str,
        _signal: &str,
        _grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError> {
        Err(DriverError::Unsupported {
            operation: "terminate",
            pass: "Pass 2",
        })
    }

    async fn nudge(&self, _session_id: &str, _content: &str) -> Result<NudgeResult, DriverError> {
        Err(DriverError::Unsupported {
            operation: "nudge",
            pass: "Pass 5",
        })
    }

    fn terminate_all(&self) {}
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
