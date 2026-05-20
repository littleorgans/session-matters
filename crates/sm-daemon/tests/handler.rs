mod common;

use common::{
    LOCAL_UID, TestDaemon, launch_env, local_context, mail_count, spawn_test_session,
    spawn_test_session_with_labels,
};
use lilo_im_core::{Action, AuditDecision, Principal};
use sm_core::{
    DeleteRequest, DoctorRequest, Label, LinkRequest, LogsRequest, MailReadRequest,
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
    let spawned = spawn_test_session(&daemon.state, &context, "general").await;

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
                    workspace: "test".to_string(),
                    agent_config: Some(config.display().to_string()),
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
async fn spawn_persists_driver_stdout_path_for_logs() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let transcript = daemon._dir.path().join("runtime.stdout.log");
    std::fs::write(&transcript, "daemon spawned\n").expect("transcript writes");
    daemon.driver.set_spawn_stdout_path(transcript.clone());

    let session = spawn_test_session(&daemon.state, &context, "engineer").await;

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
    let session = spawn_test_session(&daemon.state, &context, "engineer").await;
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

    daemon.driver.set_probe_verified(false);
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
    assert!(response.findings[0].message.contains("probe failed"));
}

#[tokio::test]
async fn mail_round_trip_marks_read() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let sender = spawn_test_session(&daemon.state, &context, "pm").await;
    let recipient = spawn_test_session(&daemon.state, &context, "engineer").await;

    let sent = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::MailSend {
                request: MailSendRequest {
                    from: Some(sender.id.to_string()),
                    to: Selector::Id { id: recipient.id },
                    content: "review the spec".to_string(),
                },
            },
        )
        .await;
    assert!(matches!(sent.response, RpcResponse::MailSent { .. }));
    assert_eq!(
        mail_count(&daemon.state, context.clone(), recipient.id).await,
        1
    );

    let read = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::MailRead {
                request: MailReadRequest {
                    selector: Selector::Id { id: recipient.id },
                    peek: false,
                },
            },
        )
        .await;
    let RpcResponse::MailRead { response } = read.response else {
        panic!("expected mail read response");
    };
    assert_eq!(response.mail.len(), 1);
    assert_eq!(response.mail[0].content, "review the spec");
    assert_eq!(mail_count(&daemon.state, context, recipient.id).await, 0);
}

#[tokio::test]
async fn selector_mail_and_nudge_fan_out_to_matching_sessions() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let sender = spawn_test_session(&daemon.state, &context, "pm").await;
    let auth_one = spawn_test_session_with_labels(
        &daemon.state,
        &context,
        "engineer",
        vec![Label {
            key: "area".to_string(),
            value: "auth".to_string(),
        }],
    )
    .await;
    let auth_two = spawn_test_session_with_labels(
        &daemon.state,
        &context,
        "engineer",
        vec![Label {
            key: "area".to_string(),
            value: "auth".to_string(),
        }],
    )
    .await;
    let ui = spawn_test_session_with_labels(
        &daemon.state,
        &context,
        "engineer",
        vec![Label {
            key: "area".to_string(),
            value: "ui".to_string(),
        }],
    )
    .await;

    let sent = daemon
        .state
        .handle(
            context.clone(),
            RpcRequest::MailSend {
                request: MailSendRequest {
                    from: Some(sender.id.to_string()),
                    to: Selector::Label {
                        key: "area".to_string(),
                        op: sm_core::LabelOp::Eq {
                            value: "auth".to_string(),
                        },
                    },
                    content: "merge by Friday".to_string(),
                },
            },
        )
        .await;
    let RpcResponse::MailSent { response } = sent.response else {
        panic!("expected mail sent response");
    };
    assert_eq!(response.mail.len(), 2);
    assert_eq!(
        response
            .mail
            .iter()
            .map(|mail| mail.recipient_id)
            .collect::<Vec<_>>(),
        vec![auth_one.id, auth_two.id]
    );
    assert_eq!(
        mail_count(&daemon.state, context.clone(), auth_one.id).await,
        1
    );
    assert_eq!(
        mail_count(&daemon.state, context.clone(), auth_two.id).await,
        1
    );
    assert_eq!(mail_count(&daemon.state, context.clone(), ui.id).await, 0);

    let nudged = daemon
        .state
        .handle(
            context,
            RpcRequest::Nudge {
                request: NudgeRequest {
                    to: Selector::Role {
                        name: "engineer".to_string(),
                    },
                    content: "review PRs".to_string(),
                },
            },
        )
        .await;
    let RpcResponse::Nudged { response } = nudged.response else {
        panic!("expected nudge response");
    };
    assert_eq!(response.nudges.len(), 3);
}

#[tokio::test]
async fn mail_send_rejects_unknown_recipient() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let sent = daemon
        .state
        .handle(
            local_context(),
            RpcRequest::MailSend {
                request: MailSendRequest {
                    from: None,
                    to: Selector::Id { id: Uuid::now_v7() },
                    content: "review the spec".to_string(),
                },
            },
        )
        .await;

    let RpcResponse::Error { message } = sent.response else {
        panic!("expected error response");
    };
    assert!(message.contains("unknown recipient session"));
}

#[tokio::test]
async fn nudge_delegates_to_driver_stub() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let recipient = spawn_test_session(&daemon.state, &context, "engineer").await;
    let nudged = daemon
        .state
        .handle(
            context,
            RpcRequest::Nudge {
                request: NudgeRequest {
                    to: Selector::Id { id: recipient.id },
                    content: "ping".to_string(),
                },
            },
        )
        .await;

    let RpcResponse::Nudged { response } = nudged.response else {
        panic!("expected nudge response");
    };
    assert_eq!(response.nudges.len(), 1);
    assert!(!response.nudges[0].delivered);
    assert_eq!(response.nudges[0].message, "nudge skipped");
}

#[tokio::test]
async fn successful_mutations_write_allow_audit_rows() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let sender = spawn_test_session(&daemon.state, &context, "pm").await;
    let recipient = spawn_test_session(&daemon.state, &context, "engineer").await;

    send_read_nudge_delete(&daemon.state, context, sender.id, recipient.id).await;

    let rows =
        lilo_im_store::query_audit(&daemon.audit_path, lilo_im_store::AuditFilters::default())
            .await
            .expect("audit query succeeds");
    let actions = rows.iter().map(|row| row.action).collect::<Vec<_>>();
    assert_eq!(
        actions,
        vec![
            Action::Spawn,
            Action::Spawn,
            Action::MailSend,
            Action::MailRead,
            Action::Nudge,
            Action::Kill,
        ]
    );
    assert!(rows.iter().all(|row| row.decision == AuditDecision::Allow));
}

#[tokio::test]
async fn denied_mutation_is_audited_without_mutating_store() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let denied_context = RequestContext::new(Principal::Local(LOCAL_UID + 1));
    let response = daemon
        .state
        .handle(
            denied_context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "general".to_string(),
                    workspace: "test".to_string(),
                    agent_config: None,
                    labels: Vec::new(),
                },
            },
        )
        .await;

    let RpcResponse::Error { message } = response.response else {
        panic!("expected authz error response");
    };
    assert!(message.contains("unknown principal"));

    let rows =
        lilo_im_store::query_audit(&daemon.audit_path, lilo_im_store::AuditFilters::default())
            .await
            .expect("audit query succeeds");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].action, Action::Spawn);
    assert_eq!(
        rows[0].decision,
        AuditDecision::Deny {
            reason: "non-local uid".to_string(),
        }
    );
    let sessions = daemon
        .state
        .store
        .lock()
        .expect("store lock poisoned")
        .list_sessions(None)
        .expect("session list succeeds");
    assert!(sessions.is_empty());
}

async fn send_read_nudge_delete(
    state: &DaemonState,
    context: RequestContext,
    sender_id: Uuid,
    recipient_id: Uuid,
) {
    let requests = [
        RpcRequest::MailSend {
            request: MailSendRequest {
                from: Some(sender_id.to_string()),
                to: Selector::Id { id: recipient_id },
                content: "review the spec".to_string(),
            },
        },
        RpcRequest::MailRead {
            request: MailReadRequest {
                selector: Selector::Id { id: recipient_id },
                peek: false,
            },
        },
        RpcRequest::Nudge {
            request: NudgeRequest {
                to: Selector::Id { id: recipient_id },
                content: "ping".to_string(),
            },
        },
        RpcRequest::Delete {
            request: DeleteRequest {
                selector: Selector::Id { id: recipient_id },
                signal: "SIGTERM".to_string(),
                grace_secs: 5,
            },
        },
    ];

    for request in requests {
        let response = state.handle(context.clone(), request).await.response;
        assert!(!matches!(response, RpcResponse::Error { .. }));
    }
}
