mod common;

use std::ffi::OsString;
use std::path::Path;

use chrono::Utc;
use common::{
    LOCAL_UID, TestDaemon, launch_env, local_context, mock_rtmd_doctor, runtime_doctor_response,
    spawn_test_session,
};
use sm_core::{
    DeleteRequest, DoctorRequest, Label, LabelMutation, LabelRequest, ListRequest, LogsRequest,
    LostEvidence, Namespace, RpcRequest, RpcResponse, RuntimeKind, Selector, SessionState,
    SpawnRequest, WaitCondition, WaitRequest,
};

#[tokio::test]
async fn drives_session_through_delete_lifecycle() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let spawned = spawn_test_session(&daemon, &context, "general").await;

    let deleted = daemon
        .state
        .handle(
            context,
            RpcRequest::Delete {
                request: DeleteRequest {
                    selector: Selector::Id { id: spawned.id },
                    signal: "SIGTERM".to_string(),
                    grace_secs: 5,
                },
            },
        )
        .await;
    let RpcResponse::Deleted { response } = deleted.response else {
        panic!("expected delete response");
    };

    assert_eq!(response.sessions.len(), 1);
    assert_eq!(response.sessions[0].state, SessionState::Terminated);
    assert_eq!(response.sessions[0].exit_code, Some(143));
    assert!(response.sessions[0].terminated_at.is_some());
}

#[tokio::test]
async fn delete_unknown_id_uses_session_noun() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let id = uuid::Uuid::nil();

    let deleted = daemon
        .state
        .handle(
            local_context(),
            RpcRequest::Delete {
                request: DeleteRequest {
                    selector: Selector::Id { id },
                    signal: "SIGTERM".to_string(),
                    grace_secs: 5,
                },
            },
        )
        .await;
    let RpcResponse::Error { message } = deleted.response else {
        panic!("expected delete error response");
    };

    assert_eq!(message, format!("unknown session: {id}"));
}

#[tokio::test]
async fn label_empty_selector_uses_session_noun() {
    let daemon = TestDaemon::new(LOCAL_UID).await;

    let labeled = daemon
        .state
        .handle(
            local_context(),
            RpcRequest::Label {
                request: LabelRequest {
                    selector: Selector::All,
                    mutation: LabelMutation::Set(Label {
                        key: "scope".to_string(),
                        value: "alpha".to_string(),
                    }),
                },
            },
        )
        .await;
    let RpcResponse::Error { message } = labeled.response else {
        panic!("expected label error response");
    };

    assert_eq!(message, "selector matched no sessions: all");
}

#[tokio::test]
async fn agent_config_env_reaches_spawn_driver() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let config = daemon._dir.path().join("agent.toml");
    std::fs::write(
        &config,
        "claude_config_dir = \"/tmp/demo-claude\"\n[env]\nHELIOY_AGENT_NAME = \"demo\"\n",
    )
    .expect("agent config writes");

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: daemon._dir.path().display().to_string(),
                    dir: None,
                    namespace: None,
                    target: "headless".to_string(),
                    agent_config: Some(config.display().to_string()),
                    env: Vec::new(),
                    shell_resume: None,
                    labels: Vec::new(),
                    force: false,
                },
            },
        )
        .await;

    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    assert_eq!(
        response.session.agent_config,
        Some(config.display().to_string())
    );
    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert!(
        launch
            .env
            .contains(&launch_env("CLAUDE_CONFIG_DIR", "/tmp/demo-claude"))
    );
    assert!(
        launch
            .env
            .contains(&launch_env("HELIOY_AGENT_NAME", "demo"))
    );
    assert!(launch.env.contains(&launch_env(
        "HELIOY_SESSION_ID",
        &response.session.id.to_string()
    )));
}

#[tokio::test]
async fn named_agent_config_persists_resolved_path() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let home = tempfile::tempdir().expect("home tempdir creates");
    let config_dir = home.path().join(".agm").join("demo-agent");
    std::fs::create_dir_all(&config_dir).expect("agent config dir creates");
    let config = config_dir.join("agent.toml");
    std::fs::write(&config, "[env]\nHELIOY_AGENT_NAME = \"demo\"\n").expect("agent config writes");
    let _home = set_home_for_test(home.path());

    let spawned = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: daemon._dir.path().display().to_string(),
                    dir: None,
                    namespace: None,
                    target: "headless".to_string(),
                    agent_config: Some("demo-agent".to_string()),
                    env: vec![launch_env("HOME", "/Users/tester")],
                    shell_resume: None,
                    labels: Vec::new(),
                    force: false,
                },
            },
        )
        .await;

    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    let expected_path = config.display().to_string();
    assert_eq!(response.session.agent_config, Some(expected_path.clone()));
    assert_ne!(response.session.agent_config.as_deref(), Some("demo-agent"));

    let listed = daemon
        .state
        .handle(
            context,
            RpcRequest::List {
                request: ListRequest {
                    selector: Some(Selector::Id {
                        id: response.session.id,
                    }),
                },
            },
        )
        .await;
    let RpcResponse::Listed { response } = listed.response else {
        panic!("expected list response");
    };

    assert_eq!(response.sessions.len(), 1);
    assert_eq!(response.sessions[0].agent_config, Some(expected_path));
}

#[tokio::test]
async fn caller_env_and_shell_resume_reach_spawn_driver() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let shell_resume = lilo_rm_core::ShellResume {
        argv: vec!["/bin/zsh".to_string()],
        env: vec![launch_env("TERM", "xterm-256color")],
        cwd: daemon._dir.path().to_path_buf(),
    };

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: daemon._dir.path().display().to_string(),
                    dir: None,
                    namespace: None,
                    target: "tmux:test:0.0".to_string(),
                    agent_config: None,
                    env: vec![
                        launch_env("HOME", "/Users/tester"),
                        launch_env("PATH", "/opt/node/bin:/usr/bin"),
                    ],
                    shell_resume: Some(shell_resume.clone()),
                    labels: Vec::new(),
                    force: true,
                },
            },
        )
        .await;

    let RpcResponse::Spawned { .. } = spawned.response else {
        panic!("expected spawn response");
    };
    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert!(launch.force);
    assert!(launch.env.contains(&launch_env("HOME", "/Users/tester")));
    assert!(
        launch
            .env
            .contains(&launch_env("PATH", "/opt/node/bin:/usr/bin"))
    );
    assert_eq!(launch.shell_resume, Some(shell_resume));
}

struct HomeEnvGuard {
    original: Option<OsString>,
}

impl Drop for HomeEnvGuard {
    fn drop(&mut self) {
        match self.original.take() {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
    }
}

fn set_home_for_test(home: &Path) -> HomeEnvGuard {
    let guard = HomeEnvGuard {
        original: std::env::var_os("HOME"),
    };
    unsafe {
        std::env::set_var("HOME", home.as_os_str());
    }
    guard
}

#[tokio::test]
async fn spawn_launch_cwd_is_request_workspace() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let session = spawn_test_session(&daemon, &local_context(), "pm").await;
    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert_eq!(launch.cwd, daemon._dir.path());
    assert!(!launch.force);
    assert_eq!(session.namespace, Namespace::default());
    assert_eq!(session.dir, daemon._dir.path());
}

#[tokio::test]
async fn spawn_accepts_new_dir_and_namespace_without_legacy_workspace() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let namespace = create_namespace(&daemon, "alpha");
    let dir = daemon._dir.path().display().to_string();

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: String::new(),
                    dir: Some(dir.clone()),
                    namespace: Some(namespace.clone()),
                    target: "headless".to_string(),
                    agent_config: None,
                    env: Vec::new(),
                    shell_resume: None,
                    labels: Vec::new(),
                    force: false,
                },
            },
        )
        .await;

    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    assert_eq!(response.session.workspace, dir);
    assert_eq!(response.session.namespace, namespace);
    assert_eq!(response.session.dir, daemon._dir.path());
}

#[tokio::test]
async fn spawn_prefers_new_dir_when_legacy_workspace_is_also_present() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let namespace = create_namespace(&daemon, "alpha");
    let legacy_workspace = tempfile::tempdir().expect("legacy workspace creates");
    let dir = daemon._dir.path().display().to_string();

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: legacy_workspace.path().display().to_string(),
                    dir: Some(dir.clone()),
                    namespace: Some(namespace.clone()),
                    target: "headless".to_string(),
                    agent_config: None,
                    env: Vec::new(),
                    shell_resume: None,
                    labels: Vec::new(),
                    force: false,
                },
            },
        )
        .await;

    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert_eq!(launch.cwd, daemon._dir.path());
    assert_eq!(response.session.workspace, dir);
    assert_eq!(response.session.namespace, namespace);
}

#[tokio::test]
async fn spawn_rejects_unknown_namespace_before_launch() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let namespace = Namespace::new("missing").expect("namespace validates");

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: String::new(),
                    dir: Some(daemon._dir.path().display().to_string()),
                    namespace: Some(namespace),
                    target: "headless".to_string(),
                    agent_config: None,
                    env: Vec::new(),
                    shell_resume: None,
                    labels: Vec::new(),
                    force: false,
                },
            },
        )
        .await;

    let RpcResponse::Error { message } = spawned.response else {
        panic!("expected error response");
    };
    assert!(message.contains("namespace not found: missing"));
    assert!(daemon.driver.launches().is_empty());
}

#[tokio::test]
async fn spawn_persists_dir_as_received_without_daemon_canonicalisation() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let child = daemon._dir.path().join("child");
    std::fs::create_dir(&child).expect("child dir creates");
    let raw_dir = child.join("..").display().to_string();

    let spawned = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "pm".to_string(),
                    workspace: String::new(),
                    dir: Some(raw_dir.clone()),
                    namespace: None,
                    target: "headless".to_string(),
                    agent_config: None,
                    env: Vec::new(),
                    shell_resume: None,
                    labels: Vec::new(),
                    force: false,
                },
            },
        )
        .await;

    let RpcResponse::Spawned { response } = spawned.response else {
        panic!("expected spawn response");
    };
    let session_namespace = daemon
        .state
        .store
        .lock()
        .expect("store lock poisoned")
        .get_session_namespace(&response.session.id)
        .expect("session namespace loads")
        .expect("session namespace exists");
    assert_eq!(session_namespace.dir.display().to_string(), raw_dir);
}

#[tokio::test]
async fn spawn_persists_driver_stdout_path_for_logs() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let transcript = daemon._dir.path().join("runtime.stdout.log");
    std::fs::write(&transcript, "daemon spawned\n").expect("transcript writes");
    daemon.driver.set_spawn_stdout_path(transcript.clone());

    let session = spawn_test_session(&daemon, &context, "engineer").await;

    assert_eq!(
        session.transcript_path.as_deref(),
        Some(transcript.as_path())
    );
    let logs = daemon
        .state
        .handle(
            context,
            RpcRequest::Logs {
                request: LogsRequest {
                    selector: Selector::Id { id: session.id },
                    max_bytes: None,
                },
            },
        )
        .await;
    let RpcResponse::Logs { response } = logs.response else {
        panic!("expected logs response");
    };
    assert_eq!(response.content, "daemon spawned\n");
}

fn create_namespace(daemon: &TestDaemon, value: &str) -> Namespace {
    let namespace = Namespace::for_create(value).expect("namespace validates");
    daemon
        .state
        .store
        .lock()
        .expect("store lock poisoned")
        .create_namespace(&namespace, Utc::now())
        .expect("namespace creates");
    namespace
}

#[tokio::test]
async fn logs_wait_and_doctor_polish_paths_work() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let session = spawn_test_session(&daemon, &context, "engineer").await;
    let transcript = daemon._dir.path().join("transcript.jsonl");
    std::fs::write(&transcript, "first\nsecond\n").expect("transcript writes");
    daemon
        .state
        .store
        .lock()
        .expect("store lock poisoned")
        .record_transcript_path(&session.id, &transcript, Utc::now())
        .expect("transcript path records")
        .expect("session exists");

    let logs = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::Logs {
                request: LogsRequest {
                    selector: Selector::Id { id: session.id },
                    max_bytes: None,
                },
            },
        )
        .await;
    let RpcResponse::Logs { response } = logs.response else {
        panic!("expected logs response");
    };
    assert_eq!(response.content, "first\nsecond\n");

    let waited = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::Wait {
                request: WaitRequest {
                    selector: Selector::Id { id: session.id },
                    condition: WaitCondition::Running,
                    timeout_secs: 0,
                },
            },
        )
        .await;
    let RpcResponse::Wait { response } = waited.response else {
        panic!("expected wait response");
    };
    assert!(response.matched);

    daemon
        .state
        .store
        .lock()
        .expect("store lock poisoned")
        .mark_session_lost(&session.id, LostEvidence::PidNotAlive, chrono::Utc::now())
        .expect("session marks lost");
    let doctor = daemon
        .state
        .handle(
            context,
            RpcRequest::Doctor {
                request: DoctorRequest::default(),
            },
        )
        .await;
    let RpcResponse::Doctor { response } = doctor.response else {
        panic!("expected doctor response");
    };
    assert_eq!(response.status, "degraded");
    assert_eq!(
        response.findings[0].session_id,
        Some(session.id.to_string())
    );
    assert!(response.findings[0].message.contains("PidNotAlive"));
}

#[tokio::test]
async fn doctor_includes_runtime_matters_payload() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let (socket_path, server) = mock_rtmd_doctor(runtime_doctor_response()).await;
    let state = daemon.state.with_rtmd_socket_path(socket_path);

    let doctor = state
        .handle(
            context,
            RpcRequest::Doctor {
                request: DoctorRequest::default(),
            },
        )
        .await;
    let RpcResponse::Doctor { response } = doctor.response else {
        panic!("expected doctor response");
    };

    assert_eq!(response.status, "ok");
    assert!(response.runtime.starts_with("rtmd (lilo-rm-client 0.6.x"));
    assert_eq!(response.runtime_matters.status, "ok");
    assert_eq!(
        response
            .runtime_matters
            .doctor
            .expect("runtime doctor payload")
            .watchers
            .process_exit_watchers,
        1
    );
    server.await.expect("rtmd doctor server");
}

#[tokio::test]
async fn doctor_reports_runtime_matters_unavailable() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let socket_path = daemon._dir.path().join("missing-rtmd.sock");
    let state = daemon.state.with_rtmd_socket_path(socket_path);

    let doctor = state
        .handle(
            context,
            RpcRequest::Doctor {
                request: DoctorRequest::default(),
            },
        )
        .await;
    let RpcResponse::Doctor { response } = doctor.response else {
        panic!("expected doctor response");
    };

    assert_eq!(response.status, "degraded");
    assert_eq!(response.runtime_matters.status, "error");
    assert_eq!(
        response.runtime_matters.code.as_deref(),
        Some("runtime_unavailable")
    );
    assert!(
        response.findings[0]
            .message
            .contains("runtime-matters doctor failed")
    );
}
