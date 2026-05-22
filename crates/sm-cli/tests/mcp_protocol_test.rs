mod common;

use std::path::Path;
use std::process::Stdio;

use common::DaemonFixture;
use lilo_im_core::{Action, AuditDecision};
use serde_json::{Value, json};

#[test]
fn initialize_and_tools_list_follow_mcp_shape() {
    let daemon = DaemonFixture::start();
    let mut mcp = daemon.spawn_mcp();

    let initialized = mcp.send(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {}
    }));
    assert_eq!(initialized["jsonrpc"], "2.0");
    assert_eq!(initialized["id"], 1);
    assert!(initialized["error"].is_null());
    assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(initialized["result"]["serverInfo"]["name"], "sm");
    assert!(initialized["result"]["instructions"].is_string());
    assert!(initialized["result"]["capabilities"]["tools"].is_object());

    let listed = mcp.send(&json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    }));
    assert_eq!(listed["jsonrpc"], "2.0");
    assert_eq!(listed["id"], 2);
    assert!(listed["error"].is_null());
    let names = tool_names(&listed["result"]["tools"]);
    assert_eq!(
        names,
        vec![
            "session_run",
            "agent_run",
            "session_list",
            "agent_list",
            "session_get",
            "agent_get",
            "namespace_list",
            "namespace_get",
            "session_capture",
            "agent_capture",
            "session_delete",
            "agent_delete",
            "session_label",
            "agent_label",
            "mail_send",
            "mail_read",
            "mail_check",
            "mail_stop_check",
            "nudge",
            "link",
            "logs",
            "wait",
            "doctor"
        ]
    );
    assert_deprecation_hint(&listed["result"]["tools"], "agent_list", "session_list");
    assert_deprecation_hint(&listed["result"]["tools"], "agent_get", "session_get");
}

#[tokio::test]
async fn tools_call_can_run_list_get_and_delete_agent() {
    let runtime_path = common::fake_runtime_path("codex");
    let daemon = DaemonFixture::start_with_runtime_path(runtime_path.path());
    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let empty = call_tool(&mut mcp, 2, "agent_list", json!({}));
    assert!(empty["error"].is_null());
    assert_eq!(
        empty["result"]["structuredContent"]["sessions"]
            .as_array()
            .expect("sessions is array")
            .len(),
        0
    );

    let spawned = call_tool(
        &mut mcp,
        3,
        "agent_run",
        json!({
            "runtime": "codex",
            "role": "engineer",
            "workspace": daemon.dir.path().display().to_string()
        }),
    );
    assert!(spawned["error"].is_null());
    let id = spawned["result"]["structuredContent"]["session"]["id"]
        .as_str()
        .expect("spawn returns session id")
        .to_string();

    let found = call_tool(&mut mcp, 4, "agent_get", json!({ "id": id }));
    assert!(found["error"].is_null());
    assert_eq!(
        found["result"]["structuredContent"]["session"]["workspace"],
        daemon.dir.path().display().to_string()
    );

    let transcript = daemon.dir.path().join("transcript.jsonl");
    std::fs::write(&transcript, "hello transcript\n").expect("transcript writes");
    let linked = call_tool(
        &mut mcp,
        5,
        "link",
        json!({
            "session_id": id.clone(),
            "runtime_session": "runtime-mcp-1",
            "transcript": transcript.display().to_string()
        }),
    );
    assert!(linked["error"].is_null());
    assert_eq!(
        linked["result"]["structuredContent"]["session"]["runtime_session"],
        "runtime-mcp-1"
    );

    let logs = call_tool(
        &mut mcp,
        6,
        "logs",
        json!({ "selector": format!("id:{id}") }),
    );
    assert!(logs["error"].is_null());
    assert_eq!(
        logs["result"]["structuredContent"]["content"],
        "hello transcript\n"
    );

    let waited = call_tool(
        &mut mcp,
        7,
        "wait",
        json!({ "selector": format!("id:{id}"), "for": "running", "timeout_secs": 0 }),
    );
    assert!(waited["error"].is_null());
    assert_eq!(waited["result"]["structuredContent"]["matched"], true);

    let doctor = call_tool(&mut mcp, 8, "doctor", json!({}));
    assert!(doctor["error"].is_null());
    assert_eq!(
        doctor["result"]["structuredContent"]["runtime"],
        "rtmd (lilo-rm-client 0.6.x, protocol 0.6)"
    );
    assert_eq!(
        doctor["result"]["structuredContent"]["runtime_matters"]["status"],
        "ok"
    );

    let deleted = call_tool(
        &mut mcp,
        9,
        "agent_delete",
        json!({ "selector": format!("id:{id}"), "signal": "SIGTERM", "grace_secs": 1 }),
    );
    assert!(deleted["error"].is_null());
    assert_eq!(
        deleted["result"]["structuredContent"]["sessions"][0]["state"],
        "TERMINATED"
    );
    assert!(
        deleted["result"]["structuredContent"]["errors"]
            .as_array()
            .expect("errors is array")
            .is_empty()
    );

    let rows =
        lilo_im_store::query_audit(daemon.audit_path(), lilo_im_store::AuditFilters::default())
            .await
            .expect("audit query succeeds");
    let actions = rows.iter().map(|row| row.action).collect::<Vec<_>>();
    assert_eq!(
        actions,
        vec![
            Action::Spawn,
            Action::Link,
            Action::Logs,
            Action::Doctor,
            Action::Kill
        ]
    );
    assert!(rows.iter().all(|row| row.decision == AuditDecision::Allow));
}

#[tokio::test]
async fn namespace_tools_list_and_get_records() {
    let daemon = DaemonFixture::start();
    create_namespace(&daemon, "alpha");
    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let listed = call_tool(&mut mcp, 2, "namespace_list", json!({}));
    assert!(listed["error"].is_null());
    assert_eq!(
        listed["result"]["structuredContent"]["namespaces"][0]["namespace"],
        "alpha"
    );
    assert_eq!(
        listed["result"]["structuredContent"]["namespaces"][1]["namespace"],
        "default"
    );

    let listed_one = call_tool(&mut mcp, 3, "namespace_list", json!({ "slug": "alpha" }));
    assert!(listed_one["error"].is_null());
    assert_eq!(
        listed_one["result"]["structuredContent"]["namespaces"][0]["namespace"],
        "alpha"
    );

    let got = call_tool(&mut mcp, 4, "namespace_get", json!({ "slug": "alpha" }));
    assert!(got["error"].is_null());
    assert_eq!(
        got["result"]["structuredContent"]["namespace"]["namespace"],
        "alpha"
    );
}

#[tokio::test]
async fn tools_call_can_select_and_label_agents() {
    let runtime_path = common::fake_runtime_path("codex");
    let daemon = DaemonFixture::start_with_runtime_path(runtime_path.path());
    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let auth = spawn_agent_with_labels(
        &mut mcp,
        2,
        "engineer",
        daemon.dir.path(),
        json!(["area=auth"]),
    );
    let ui = spawn_agent_with_labels(
        &mut mcp,
        3,
        "engineer",
        daemon.dir.path(),
        json!(["area=ui"]),
    );

    let selected = call_tool(
        &mut mcp,
        4,
        "agent_list",
        json!({ "selector": "label:area=auth" }),
    );
    assert!(selected["error"].is_null());
    assert_eq!(
        selected["result"]["structuredContent"]["sessions"][0]["id"],
        auth
    );

    let labeled = call_tool(
        &mut mcp,
        5,
        "agent_label",
        json!({ "selector": format!("id:{ui}"), "mutation": "area=auth" }),
    );
    assert!(labeled["error"].is_null());

    let selected = call_tool(
        &mut mcp,
        6,
        "agent_list",
        json!({ "selector": "label:area=auth" }),
    );
    assert!(selected["error"].is_null());
    assert_eq!(
        selected["result"]["structuredContent"]["sessions"]
            .as_array()
            .expect("sessions is array")
            .len(),
        2
    );
}

#[tokio::test]
async fn session_tools_share_agent_handlers_and_namespace_read_scope() {
    let runtime_path = common::fake_runtime_path("codex");
    let daemon = DaemonFixture::start_with_runtime_path(runtime_path.path());
    create_namespace(&daemon, "alpha");
    create_namespace(&daemon, "beta");

    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let caller = spawn_agent_in_namespace(&mut mcp, 2, "caller", daemon.dir.path(), "alpha");
    let alpha_peer = spawn_agent_in_namespace(&mut mcp, 3, "engineer", daemon.dir.path(), "alpha");
    let beta_peer = spawn_agent_in_namespace(&mut mcp, 4, "engineer", daemon.dir.path(), "beta");

    let agent_all = call_tool(&mut mcp, 5, "agent_list", json!({ "all_namespaces": true }));
    let session_all = call_tool(
        &mut mcp,
        6,
        "session_list",
        json!({ "all_namespaces": true }),
    );
    assert_eq!(
        agent_all["result"]["structuredContent"],
        session_all["result"]["structuredContent"]
    );

    let explicit_alpha = call_tool(&mut mcp, 7, "session_list", json!({ "namespace": "alpha" }));
    assert_session_ids(&explicit_alpha, &[&caller, &alpha_peer]);

    let bypass = call_tool(
        &mut mcp,
        8,
        "session_list",
        json!({ "all_namespaces": true }),
    );
    assert_session_ids(&bypass, &[&caller, &alpha_peer, &beta_peer]);

    let marked_cwd = daemon.dir.path().join("marked-cwd");
    std::fs::create_dir_all(marked_cwd.join(".sm")).expect("marker dir creates");
    std::fs::write(marked_cwd.join(".sm").join("namespace"), "beta").expect("marker writes");
    let mut caller_mcp = daemon.spawn_mcp_for_session(&caller, &marked_cwd);
    caller_mcp.send(&json!({"jsonrpc": "2.0", "id": 9, "method": "initialize", "params": {}}));

    let implicit_alpha = call_tool(&mut caller_mcp, 10, "session_list", json!({}));
    assert_session_ids(&implicit_alpha, &[&caller, &alpha_peer]);

    let nudged_alpha = call_tool(
        &mut mcp,
        11,
        "nudge",
        json!({ "to": "all", "namespace": "alpha", "content": "ping" }),
    );
    assert_nudged_ids(&nudged_alpha, &[&caller, &alpha_peer]);

    let nudged_all = call_tool(
        &mut mcp,
        12,
        "nudge",
        json!({ "to": "all", "all_namespaces": true, "content": "ping" }),
    );
    assert_nudged_ids(&nudged_all, &[&caller, &alpha_peer, &beta_peer]);
}

#[tokio::test]
async fn tools_call_can_send_read_check_mail_and_nudge() {
    let runtime_path = common::fake_runtime_path("codex");
    let daemon = DaemonFixture::start_with_runtime_path(runtime_path.path());
    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let sender = spawn_agent(&mut mcp, 2, "pm", daemon.dir.path());
    let recipient = spawn_agent(&mut mcp, 3, "engineer", daemon.dir.path());

    let sent = call_tool(
        &mut mcp,
        4,
        "mail_send",
        json!({
            "from": sender,
            "to": recipient.clone(),
            "content": "review the spec"
        }),
    );
    assert!(sent["error"].is_null());
    assert_eq!(
        sent["result"]["structuredContent"]["mail"][0]["content"],
        "review the spec"
    );

    let checked = call_tool(
        &mut mcp,
        5,
        "mail_check",
        json!({ "selector": format!("id:{recipient}") }),
    );
    assert!(checked["error"].is_null());
    assert_eq!(checked["result"]["structuredContent"]["unread"], 1);
    assert_eq!(
        checked["result"]["structuredContent"]["counts"][0]["unread"],
        1
    );
    assert_eq!(
        checked["result"]["structuredContent"]["counts"][0]["session_id"],
        recipient
    );

    let read = call_tool(
        &mut mcp,
        6,
        "mail_read",
        json!({ "selector": format!("id:{recipient}") }),
    );
    assert!(read["error"].is_null());
    assert_eq!(
        read["result"]["structuredContent"]["mail"][0]["content"],
        "review the spec"
    );

    let checked = call_tool(
        &mut mcp,
        7,
        "mail_stop_check",
        json!({ "selector": format!("id:{recipient}") }),
    );
    assert!(checked["error"].is_null());
    assert_eq!(checked["result"]["structuredContent"]["unread"], 0);
    assert_eq!(
        checked["result"]["structuredContent"]["counts"][0]["unread"],
        0
    );

    let nudged = call_tool(
        &mut mcp,
        8,
        "nudge",
        json!({ "to": recipient.clone(), "content": "ping" }),
    );
    assert!(nudged["error"].is_null());
    assert_eq!(
        nudged["result"]["structuredContent"]["nudges"][0]["message"],
        "headless runtime does not support nudges"
    );
    assert_eq!(
        nudged["result"]["structuredContent"]["nudges"][0]["delivered"],
        false
    );
    assert!(
        nudged["result"]["structuredContent"]["errors"]
            .as_array()
            .expect("nudge errors is array")
            .is_empty()
    );

    let rows =
        lilo_im_store::query_audit(daemon.audit_path(), lilo_im_store::AuditFilters::default())
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
            Action::Nudge
        ]
    );
    assert!(rows.iter().all(|row| row.decision == AuditDecision::Allow));
}

#[test]
fn generated_schema_matches_contract_registry() {
    assert_eq!(
        sm_cli::mcp::schema::tool_list(),
        sm_cli::tool_contracts::contract_registry().tool_list_value()
    );
}

fn call_tool(mcp: &mut common::McpFixture, id: u64, name: &str, arguments: Value) -> Value {
    mcp.send(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": arguments
        }
    }))
}

fn spawn_agent(mcp: &mut common::McpFixture, id: u64, role: &str, workspace: &Path) -> String {
    spawn_agent_with_labels(mcp, id, role, workspace, json!([]))
}

fn spawn_agent_in_namespace(
    mcp: &mut common::McpFixture,
    id: u64,
    role: &str,
    workspace: &Path,
    namespace: &str,
) -> String {
    let spawned = call_tool(
        mcp,
        id,
        "session_run",
        json!({
            "runtime": "codex",
            "role": role,
            "dir": workspace.display().to_string(),
            "namespace": namespace
        }),
    );
    assert!(spawned["error"].is_null());
    spawned["result"]["structuredContent"]["session"]["id"]
        .as_str()
        .expect("spawn returns session id")
        .to_string()
}

fn spawn_agent_with_labels(
    mcp: &mut common::McpFixture,
    id: u64,
    role: &str,
    workspace: &Path,
    labels: Value,
) -> String {
    let spawned = call_tool(
        mcp,
        id,
        "agent_run",
        json!({
            "runtime": "codex",
            "role": role,
            "workspace": workspace.display().to_string(),
            "labels": labels
        }),
    );
    assert!(spawned["error"].is_null());
    spawned["result"]["structuredContent"]["session"]["id"]
        .as_str()
        .expect("spawn returns session id")
        .to_string()
}

fn tool_names(tools: &Value) -> Vec<&str> {
    tools
        .as_array()
        .expect("tools is array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect()
}

fn create_namespace(daemon: &DaemonFixture, name: &str) {
    let status = daemon
        .command()
        .args(["create", "namespace", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("namespace create runs");
    assert!(status.success());
}

fn assert_deprecation_hint(tools: &Value, deprecated: &str, replacement: &str) {
    let description = tools
        .as_array()
        .expect("tools is array")
        .iter()
        .find(|tool| tool["name"] == deprecated)
        .expect("deprecated tool exists")["description"]
        .as_str()
        .expect("description is string");
    assert!(description.contains("Deprecated compatibility alias"));
    assert!(description.contains(replacement));
}

fn assert_session_ids(response: &Value, expected: &[&str]) {
    assert!(response["error"].is_null());
    let mut actual = response["result"]["structuredContent"]["sessions"]
        .as_array()
        .expect("sessions is array")
        .iter()
        .map(|session| {
            session["id"]
                .as_str()
                .expect("session id is string")
                .to_string()
        })
        .collect::<Vec<_>>();
    let mut expected = expected
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

fn assert_nudged_ids(response: &Value, expected: &[&str]) {
    assert!(response["error"].is_null());
    let mut actual = response["result"]["structuredContent"]["nudges"]
        .as_array()
        .expect("nudges is array")
        .iter()
        .map(|nudge| {
            nudge["to"]
                .as_str()
                .expect("nudge target is string")
                .to_string()
        })
        .collect::<Vec<_>>();
    let mut expected = expected
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}
