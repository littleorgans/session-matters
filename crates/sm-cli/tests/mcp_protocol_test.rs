mod common;

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
            "agent_run",
            "agent_list",
            "agent_get",
            "agent_delete",
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
}

#[tokio::test]
async fn tools_call_can_run_list_get_and_delete_agent() {
    let daemon = DaemonFixture::start();
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
            "workspace": "mcp-test"
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
        "mcp-test"
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
        "in-process driver active"
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
async fn tools_call_can_select_and_label_agents() {
    let daemon = DaemonFixture::start();
    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let auth = spawn_agent_with_labels(&mut mcp, 2, "engineer", json!(["area=auth"]));
    let ui = spawn_agent_with_labels(&mut mcp, 3, "engineer", json!(["area=ui"]));

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
async fn tools_call_can_send_read_check_mail_and_nudge() {
    let daemon = DaemonFixture::start();
    let mut mcp = daemon.spawn_mcp();
    mcp.send(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}));

    let sender = spawn_agent(&mut mcp, 2, "pm");
    let recipient = spawn_agent(&mut mcp, 3, "engineer");

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
        nudged["result"]["structuredContent"]["nudges"][0]["delivered"],
        false
    );
    assert_eq!(
        nudged["result"]["structuredContent"]["nudges"][0]["message"],
        "nudge: tmux gateway not available; nudge skipped"
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

fn spawn_agent(mcp: &mut common::McpFixture, id: u64, role: &str) -> String {
    spawn_agent_with_labels(mcp, id, role, json!([]))
}

fn spawn_agent_with_labels(
    mcp: &mut common::McpFixture,
    id: u64,
    role: &str,
    labels: Value,
) -> String {
    let spawned = call_tool(
        mcp,
        id,
        "agent_run",
        json!({
            "runtime": "codex",
            "role": role,
            "workspace": "mcp-mail-test",
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
