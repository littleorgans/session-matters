mod common;

use common::{LOCAL_UID, TestDaemon, local_context};
use lilo_rm_core::{CaptureResponse, PaneSnapshot};
use sm_core::{CaptureRequest, RpcRequest, RpcResponse, RuntimeKind, Selector, SpawnRequest};

#[tokio::test]
async fn spawn_validates_target_and_persists_tmux_pane() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    daemon.driver.set_spawn_tmux_pane("test:0.0");

    let response = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "engineer".to_string(),
                    workspace: "test".to_string(),
                    target: "tmux:test:0.0".to_string(),
                    agent_config: None,
                    labels: Vec::new(),
                },
            },
        )
        .await;

    let RpcResponse::Spawned { response } = response.response else {
        panic!("expected spawn response");
    };
    assert_eq!(response.session.tmux_pane.as_deref(), Some("test:0.0"));
    assert_eq!(daemon.driver.launches()[0].target, "tmux:test:0.0");
}

#[tokio::test]
async fn spawn_rejects_invalid_target_before_launch() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();

    let response = daemon
        .state
        .handle(
            context,
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime: RuntimeKind::Claude,
                    role: "engineer".to_string(),
                    workspace: "test".to_string(),
                    target: "tmux:not-a-pane".to_string(),
                    agent_config: None,
                    labels: Vec::new(),
                },
            },
        )
        .await;

    let RpcResponse::Error { message } = response.response else {
        panic!("expected target validation error");
    };
    assert!(message.contains("invalid runtime target"), "{message}");
    assert!(daemon.driver.launches().is_empty());
}

#[tokio::test]
async fn capture_delegates_to_driver_for_selected_session() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    daemon
        .driver
        .set_capture(CaptureResponse::Captured(PaneSnapshot {
            content: "pane text\n".to_string(),
            captured_at_ms: 10,
            scrollback_lines_requested: 20,
            scrollback_lines_included: 1,
            pane_history_lines: 1,
        }));
    let session = common::spawn_test_session(&daemon.state, &context, "engineer").await;

    let response = daemon
        .state
        .handle(
            context,
            RpcRequest::Capture {
                request: CaptureRequest {
                    selector: Selector::Id { id: session.id },
                    scrollback_lines: Some(20),
                },
            },
        )
        .await;

    let RpcResponse::Capture { response } = response.response else {
        panic!("expected capture response");
    };
    let CaptureResponse::Captured(snapshot) = response.capture else {
        panic!("expected captured snapshot");
    };
    assert_eq!(snapshot.content, "pane text\n");
}
