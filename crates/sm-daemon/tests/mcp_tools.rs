mod common;

use std::path::Path;

use common::{LOCAL_UID, TestDaemon, local_context};
use serde_json::{Value, json};
use sm_core::{IsolationPolicy, MountSpec};

const IMAGE: &str = "runtime-matters-claude:local";

#[tokio::test]
async fn agent_run_isolation_and_image_reach_spawn_driver() {
    assert_run_tool_launch("agent_run").await;
}

#[tokio::test]
async fn session_run_isolation_and_image_reach_spawn_driver() {
    assert_run_tool_launch("session_run").await;
}

#[tokio::test]
async fn agent_run_unknown_isolation_returns_structured_mcp_error() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let arguments = run_arguments(daemon._dir.path(), "kubernetes", None);
    let line = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "agent_run",
            "arguments": arguments
        }
    })
    .to_string();

    let response = sm_daemon::mcp_bridge::handle_line(&daemon.state, &context, &line)
        .await
        .expect("tools/call returns a response");
    let response: Value = serde_json::from_str(&response).expect("response is JSON");
    let message = response["result"]["_meta"]["sm_tool_error"]["message"]
        .as_str()
        .expect("structured MCP error includes a message");

    assert!(response["error"].is_null());
    assert_eq!(
        response["result"]["_meta"]["sm_tool_error"]["is_error"],
        true
    );
    assert!(
        message.contains("invalid isolation policy kubernetes"),
        "{message}"
    );
    assert!(daemon.driver.launches().is_empty());
}

#[tokio::test]
async fn session_run_mounts_reject_host_isolation() {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let mut arguments = run_arguments(daemon._dir.path(), "host", None);
    arguments
        .as_object_mut()
        .expect("run arguments are an object")
        .insert(
            "mounts".to_string(),
            json!(["/host/config:/container/config"]),
        );

    let error = sm_daemon::mcp_tools::call_tool(&daemon.state, &context, "session_run", &arguments)
        .await
        .expect_err("host mounts are rejected");

    assert!(
        error
            .to_string()
            .contains("--mount is docker-only and cannot be used with --isolation host"),
        "{error}"
    );
    assert!(daemon.driver.launches().is_empty());
}

async fn assert_run_tool_launch(tool_name: &str) {
    let daemon = TestDaemon::new(LOCAL_UID).await;
    let context = local_context();
    let mut arguments = run_arguments(daemon._dir.path(), "docker", Some(IMAGE));
    let expected_mounts = vec![
        MountSpec {
            source: "/host/config".into(),
            target: "/container/config".into(),
            read_only: true,
        },
        MountSpec {
            source: "/host/cache".into(),
            target: "/container/cache".into(),
            read_only: false,
        },
    ];
    arguments
        .as_object_mut()
        .expect("run arguments are an object")
        .insert(
            "mounts".to_string(),
            json!([
                "/host/config:/container/config",
                "/host/cache:/container/cache:rw"
            ]),
        );

    let response = sm_daemon::mcp_tools::call_tool(&daemon.state, &context, tool_name, &arguments)
        .await
        .expect("run tool succeeds");
    assert!(response["structuredContent"]["session"]["id"].is_string());

    let launch = daemon.driver.launches().pop().expect("driver saw launch");
    assert_eq!(
        launch.isolation,
        IsolationPolicy::Docker(Default::default())
    );
    assert_eq!(launch.image.as_deref(), Some(IMAGE));
    assert_eq!(launch.mounts, expected_mounts);
}

fn run_arguments(dir: &Path, isolation: &str, image: Option<&str>) -> Value {
    let mut arguments = json!({
        "runtime": "claude",
        "role": "engineer",
        "dir": dir.display().to_string(),
        "isolation": isolation
    });

    if let Some(image) = image {
        arguments
            .as_object_mut()
            .expect("run arguments are an object")
            .insert("image".to_string(), json!(image));
    }

    arguments
}
