mod common;

use common::{
    LOCAL_UID, TestDaemon, launch_env, local_context, mail_count, mock_rtmd_doctor,
    runtime_doctor_response, spawn_test_session, spawn_test_session_with_labels,
};
use lilo_im_core::{Action, AuditDecision, Principal};
use sm_core::{
    DeleteRequest, DoctorRequest, Label, LinkRequest, LogsRequest, LostEvidence, MailReadRequest,
    MailSendRequest, NudgeRequest, RpcRequest, RpcResponse, RuntimeKind, Selector, SessionState,
    SpawnRequest, WaitCondition, WaitRequest,
};
use sm_daemon::handler::DaemonState;
use sm_daemon::identity_client::RequestContext;
use uuid::Uuid;

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
                    target: "headless".to_string(),
                    agent_config: Some(config.display().to_string()),
                    env: Vec::new(),
                    shell_resume: None,
                    labels: Vec::new(),
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
                    target: "tmux:test:0.0".to_string(),
                    agent_config: None,
                    env: vec![
                        launch_env("HOME", "/Users/tester"),
                        launch_env("PATH", "/opt/node/bin:/usr/bin"),
                    ],
                    shell_resume: Some(shell_resume.clone()),
                    labels: Vec::new(),
                },
            },
        )
        .await;

    let RpcResponse::Spawned { .. } = spawned.response else {
        panic!("expected spawn response");
    };
    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert!(launch.env.contains(&launch_env("HOME", "/Users/tester")));
    assert!(
        launch
            .env
            .contains(&launch_env("PATH", "/opt/node/bin:/usr/bin"))
    );
    assert_eq!(launch.shell_resume, Some(shell_resume));
}

#[tokio::test]
async fn spawn_launch_cwd_is_request_workspace() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    spawn_test_session(&daemon, &local_context(), "pm").await;
    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert_eq!(launch.cwd, daemon._dir.path());
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

#[tokio::test]
async fn link_logs_wait_and_doctor_polish_paths_work() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let session = spawn_test_session(&daemon, &context, "engineer").await;
    let transcript = daemon._dir.path().join("transcript.jsonl");
    std::fs::write(&transcript, "first\nsecond\n").expect("transcript writes");

    let linked = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::Link {
                request: LinkRequest {
                    session_id: Some(session.id),
                    selector: None,
                    runtime_session: "runtime-1".to_string(),
                    transcript_path: transcript.clone(),
                },
            },
        )
        .await;
    let RpcResponse::Linked { response } = linked.response else {
        panic!("expected link response");
    };
    assert_eq!(
        response.session.runtime_session.as_deref(),
        Some("runtime-1")
    );

    let relinked = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::Link {
                request: LinkRequest {
                    session_id: None,
                    selector: None,
                    runtime_session: "runtime-1".to_string(),
                    transcript_path: transcript.clone(),
                },
            },
        )
        .await;
    let RpcResponse::Linked { response } = relinked.response else {
        panic!("expected idempotent link response");
    };
    assert_eq!(response.session.id, session.id);

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
