use lilo_rm_core::{
    NudgeFailureReason, NudgeOutcome, NudgePayload, NudgeRequest, NudgeResponse, RuntimeResponse,
    RuntimeRpc, read_json_line, write_json_line,
};
use sm_driver::{RtmdDriver, SpawnDriver};
use tokio::io::BufReader;
use tokio::net::UnixListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

#[tokio::test]
async fn rtmd_nudge_maps_delivered_outcome() {
    let session_id = Uuid::now_v7();
    let (driver, server) = mock_rtmd_nudge(session_id, NudgeOutcome::Delivered).await;

    let result = driver
        .nudge(&session_id.to_string(), "hello")
        .await
        .expect("nudge delegates to rtmd");

    assert!(result.delivered);
    assert_eq!(result.message, "delivered");
    server.await.expect("server task");
}

#[tokio::test]
async fn rtmd_nudge_maps_tmux_pane_dead_outcome() {
    let session_id = Uuid::now_v7();
    let (driver, server) = mock_rtmd_nudge(
        session_id,
        NudgeOutcome::Failed(NudgeFailureReason::TmuxPaneDead),
    )
    .await;

    let result = driver
        .nudge(&session_id.to_string(), "hello")
        .await
        .expect("failed nudge outcome remains a response");

    assert!(!result.delivered);
    assert_eq!(result.message, "tmux pane is no longer available");
    server.await.expect("server task");
}

async fn mock_rtmd_nudge(session_id: Uuid, outcome: NudgeOutcome) -> (RtmdDriver, JoinHandle<()>) {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let socket_path = tempdir.path().join("rtmd.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind test socket");
    let driver = RtmdDriver::new(socket_path);
    let server = tokio::spawn(async move {
        let _tempdir = tempdir;
        let (stream, _) = listener.accept().await.expect("accept client");
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let rpc: RuntimeRpc = read_json_line(&mut reader).await.expect("read rpc");
        assert_eq!(
            rpc,
            RuntimeRpc::Nudge {
                request: NudgeRequest {
                    session_id,
                    content: "hello".to_string(),
                },
            }
        );
        write_json_line(
            &mut write_half,
            &RuntimeResponse::Nudge(NudgePayload {
                response: NudgeResponse {
                    delivered: matches!(outcome, NudgeOutcome::Delivered),
                    outcome,
                },
            }),
        )
        .await
        .expect("write response");
    });
    (driver, server)
}
