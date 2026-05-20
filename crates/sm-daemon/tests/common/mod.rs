#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lilo_im_core::Principal;
use sm_core::{
    Label, MailCheckRequest, RpcRequest, RpcResponse, RuntimeKind, Selector, Session, SpawnRequest,
};
use sm_daemon::handler::DaemonState;
use sm_daemon::identity_client::{IdentityClient, RequestContext};
use sm_driver::{
    ChildExit, DriverError, DriverProbe, LaunchEnv, SpawnDriver, SpawnLaunch, SpawnedProcess,
};
use sm_store::SqliteStore;
use uuid::Uuid;

pub const LOCAL_UID: u32 = 42;

pub struct MockDriver {
    exits: Mutex<Vec<ChildExit>>,
    launches: Mutex<Vec<SpawnLaunch>>,
    probe_verified: Mutex<bool>,
    terminate_exit: Mutex<Option<ChildExit>>,
}

impl MockDriver {
    pub fn new() -> Self {
        Self {
            exits: Mutex::new(Vec::new()),
            launches: Mutex::new(Vec::new()),
            probe_verified: Mutex::new(true),
            terminate_exit: Mutex::new(Some(ChildExit {
                session_id: String::new(),
                runtime_pid: 42,
                exit_code: Some(143),
            })),
        }
    }

    pub fn launches(&self) -> Vec<SpawnLaunch> {
        self.launches
            .lock()
            .expect("launches lock poisoned")
            .clone()
    }

    pub fn set_probe_verified(&self, verified: bool) {
        *self.probe_verified.lock().expect("probe lock poisoned") = verified;
    }

    pub fn set_terminate_exit(&self, exit: Option<ChildExit>) {
        *self
            .terminate_exit
            .lock()
            .expect("terminate exit lock poisoned") = exit;
    }
}

#[async_trait]
impl SpawnDriver for MockDriver {
    async fn spawn(
        &self,
        _session_id: &str,
        launch: &SpawnLaunch,
    ) -> Result<SpawnedProcess, DriverError> {
        self.launches
            .lock()
            .expect("launches lock poisoned")
            .push(launch.clone());
        Ok(SpawnedProcess {
            runtime_pid: 42,
            log_dir: None,
            stdout_path: None,
            stderr_path: None,
        })
    }

    async fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError> {
        Ok(self
            .exits
            .lock()
            .expect("exits lock poisoned")
            .drain(..)
            .collect())
    }

    async fn probe_session(
        &self,
        _session_id: &str,
        _runtime_pid: u32,
    ) -> Result<DriverProbe, DriverError> {
        let verified = *self.probe_verified.lock().expect("probe lock poisoned");
        Ok(DriverProbe {
            verified,
            evidence: if verified {
                "verified".to_string()
            } else {
                "probe failed".to_string()
            },
        })
    }

    async fn terminate(
        &self,
        session_id: &str,
        _signal: &str,
        _grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError> {
        Ok(self
            .terminate_exit
            .lock()
            .expect("terminate exit lock poisoned")
            .clone()
            .map(|exit| ChildExit {
                session_id: session_id.to_string(),
                ..exit
            }))
    }

    async fn nudge(
        &self,
        _session_id: &str,
        _content: &str,
    ) -> Result<sm_driver::NudgeResult, DriverError> {
        Ok(sm_driver::NudgeResult {
            delivered: false,
            message: "nudge skipped".to_string(),
        })
    }

    fn terminate_all(&self) {}
}

pub struct TestDaemon {
    pub state: DaemonState,
    pub driver: Arc<MockDriver>,
    pub audit_path: PathBuf,
    pub _dir: tempfile::TempDir,
}

impl TestDaemon {
    pub async fn new(local_uid: u32) -> Self {
        let dir = tempfile::tempdir().expect("tempdir creates");
        let audit_path = dir.path().join("audit.sqlite");
        let identity = IdentityClient::connect(&audit_path, local_uid)
            .await
            .expect("identity client connects");
        let driver = Arc::new(MockDriver::new());
        let state = DaemonState::new(
            SqliteStore::open_in_memory().expect("store opens"),
            driver.clone(),
            Arc::new(identity),
        );
        Self {
            state,
            driver,
            audit_path,
            _dir: dir,
        }
    }
}

pub async fn spawn_test_session(
    state: &DaemonState,
    context: &RequestContext,
    role: &str,
) -> Session {
    spawn_test_session_with_labels(state, context, role, Vec::new()).await
}

pub async fn spawn_test_session_with_labels(
    state: &DaemonState,
    context: &RequestContext,
    role: &str,
    labels: Vec<Label>,
) -> Session {
    let spawned = state
        .handle(
            context.clone(),
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: role.to_string(),
                    workspace: "test".to_string(),
                    agent_config: None,
                    labels,
                },
            },
        )
        .await;
    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    response.session
}

pub async fn mail_count(state: &DaemonState, context: RequestContext, session_id: Uuid) -> usize {
    let response = state
        .handle(
            context,
            RpcRequest::MailCheck {
                request: MailCheckRequest {
                    selector: Selector::Id { id: session_id },
                },
            },
        )
        .await;
    let RpcResponse::MailChecked { response } = response.response else {
        panic!("expected mail check response");
    };
    response.unread
}

pub fn local_context() -> RequestContext {
    RequestContext::new(Principal::Local(LOCAL_UID))
}

pub fn launch_env(key: &str, value: &str) -> LaunchEnv {
    LaunchEnv {
        key: key.to_string(),
        value: value.to_string(),
    }
}
