use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use im_core::{Action, AuditDecision, Principal};
use sm_core::{
    DeleteRequest, Label, MailCheckRequest, MailReadRequest, MailSendRequest, NudgeRequest,
    RpcRequest, RpcResponse, RuntimeKind, Selector, Session, SessionState, SpawnRequest,
};
use sm_daemon::handler::DaemonState;
use sm_daemon::identity_client::{IdentityClient, RequestContext};
use sm_driver::{ChildExit, DriverError, SpawnDriver, SpawnedProcess};
use sm_store::SqliteStore;
use uuid::Uuid;

const LOCAL_UID: u32 = 42;

struct MockDriver {
    exits: Mutex<Vec<ChildExit>>,
}

impl MockDriver {
    fn new() -> Self {
        Self {
            exits: Mutex::new(Vec::new()),
        }
    }
}

impl SpawnDriver for MockDriver {
    fn spawn(
        &self,
        _session_id: &str,
        _request: &SpawnRequest,
    ) -> Result<SpawnedProcess, DriverError> {
        Ok(SpawnedProcess { runtime_pid: 42 })
    }

    fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError> {
        Ok(self
            .exits
            .lock()
            .expect("exits lock poisoned")
            .drain(..)
            .collect())
    }

    fn terminate(
        &self,
        session_id: &str,
        _signal: &str,
        _grace: Duration,
    ) -> Result<Option<ChildExit>, DriverError> {
        Ok(Some(ChildExit {
            session_id: session_id.to_string(),
            runtime_pid: 42,
            exit_code: Some(143),
        }))
    }

    fn nudge(
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

struct TestDaemon {
    state: DaemonState,
    audit_path: PathBuf,
    _dir: tempfile::TempDir,
}

impl TestDaemon {
    async fn new(local_uid: u32) -> Self {
        let dir = tempfile::tempdir().expect("tempdir creates");
        let audit_path = dir.path().join("audit.sqlite");
        let identity = IdentityClient::connect(&audit_path, local_uid)
            .await
            .expect("identity client connects");
        let state = DaemonState::new(
            SqliteStore::open_in_memory().expect("store opens"),
            Arc::new(MockDriver::new()),
            Arc::new(identity),
        );
        Self {
            state,
            audit_path,
            _dir: dir,
        }
    }
}

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

    let rows = im_store::query_audit(&daemon.audit_path, im_store::AuditFilters::default())
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
                    labels: Vec::new(),
                },
            },
        )
        .await;

    let RpcResponse::Error { message } = response.response else {
        panic!("expected authz error response");
    };
    assert!(message.contains("unknown principal"));

    let rows = im_store::query_audit(&daemon.audit_path, im_store::AuditFilters::default())
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

async fn spawn_test_session(state: &DaemonState, context: &RequestContext, role: &str) -> Session {
    spawn_test_session_with_labels(state, context, role, Vec::new()).await
}

async fn spawn_test_session_with_labels(
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

async fn mail_count(state: &DaemonState, context: RequestContext, session_id: Uuid) -> usize {
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

fn local_context() -> RequestContext {
    RequestContext::new(Principal::Local(LOCAL_UID))
}
