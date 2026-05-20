use std::str::FromStr;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use sm_core::{
    CaptureRequest, DeleteRequest, DoctorRequest, JsonRpcError, JsonRpcRequest, JsonRpcResponse,
    Label, LabelMutation, LabelRequest, LinkRequest, ListRequest, LogsRequest,
    MCP_PROTOCOL_VERSION, MailCheckRequest, MailReadRequest, MailSendRequest, MailStopCheckRequest,
    NudgeRequest, RpcRequest, RpcResponse, RuntimeKind, Selector, SpawnRequest, WaitCondition,
    WaitRequest, tool_contracts::contract_registry, tool_error, tool_success,
};

use crate::handler::DaemonState;
use crate::identity_client::RequestContext;

pub async fn handle_line(
    state: &DaemonState,
    context: &RequestContext,
    line: &str,
) -> Option<String> {
    let response = match serde_json::from_str::<JsonRpcRequest>(line) {
        Ok(request) => handle_request(state, context, request).await?,
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Value::Null,
            result: None,
            error: Some(json_rpc_error(-32700, format!("Parse error: {error}"))),
        },
    };
    Some(serde_json::to_string(&response).expect("JSON-RPC response serializes"))
}

async fn handle_request(
    state: &DaemonState,
    context: &RequestContext,
    request: JsonRpcRequest,
) -> Option<JsonRpcResponse> {
    let id = request.id.unwrap_or(Value::Null);
    if request.method.starts_with("notifications/") {
        return None;
    }

    let result = match request.method.as_str() {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(contract_registry().tool_list_value()),
        "tools/call" => handle_tool_call(state, context, request.params).await,
        other => Err(json_rpc_error(-32601, format!("Method not found: {other}"))),
    };

    Some(match result {
        Ok(result) => json_rpc_result(id, result),
        Err(error) => json_rpc_failure(id, error),
    })
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": MCP_PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "sm",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": server_instructions()
    })
}

async fn handle_tool_call(
    state: &DaemonState,
    context: &RequestContext,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| json_rpc_error(-32602, "Missing params"))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| json_rpc_error(-32602, "Missing tool name"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    Ok(match call_tool(state, context, name, &arguments).await {
        Ok(value) => value,
        Err(error) => tool_error(error.to_string()),
    })
}

async fn call_tool(
    state: &DaemonState,
    context: &RequestContext,
    name: &str,
    arguments: &Value,
) -> Result<Value> {
    match name {
        "agent_run" => agent_run(state, context, arguments).await,
        "agent_list" => agent_list(state, context, arguments).await,
        "agent_get" => agent_get(state, context, arguments).await,
        "agent_capture" => agent_capture(state, context, arguments).await,
        "agent_delete" => agent_delete(state, context, arguments).await,
        "agent_label" => agent_label(state, context, arguments).await,
        "mail_send" => mail_send(state, context, arguments).await,
        "mail_read" => mail_read(state, context, arguments).await,
        "mail_check" => mail_check(state, context, arguments).await,
        "mail_stop_check" => mail_stop_check(state, context, arguments).await,
        "nudge" => nudge(state, context, arguments).await,
        "link" => link(state, context, arguments).await,
        "logs" => logs(state, context, arguments).await,
        "wait" => wait(state, context, arguments).await,
        "doctor" => doctor(state, context, arguments).await,
        other => Ok(tool_error(format!("Unknown tool: {other}"))),
    }
}

async fn agent_run(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let runtime = RuntimeKind::from_str(required_string(arguments, "runtime")?)?;
    let role = required_string(arguments, "role")?.to_string();
    let workspace = required_string(arguments, "workspace")?.to_string();
    let labels = optional_labels(arguments)?;
    let agent_config = optional_string(arguments, "agent_config").map(ToString::to_string);
    let target = optional_string(arguments, "target")
        .unwrap_or("headless")
        .to_string();
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Spawn {
                request: SpawnRequest {
                    runtime,
                    role,
                    workspace,
                    target,
                    agent_config,
                    labels,
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Spawned { response } => {
            let text = format!("spawned {}", response.session.id);
            Ok(tool_success(text, &json!({ "session": response.session })))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn agent_list(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let selector = optional_selector(arguments, "selector")?
        .or_else(|| optional_string(arguments, "id").and_then(|id| selector_from_id(id).ok()));
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::List {
                request: ListRequest { selector },
            },
        )
        .await;
    match response.response {
        RpcResponse::Listed { response } => {
            let count = response.sessions.len();
            Ok(tool_success(
                format!("{count} session(s)"),
                &json!({ "sessions": response.sessions }),
            ))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn agent_get(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let id = required_string(arguments, "id")?.to_string();
    let selector = Selector::Id {
        id: uuid::Uuid::parse_str(&id)?,
    };
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::List {
                request: ListRequest {
                    selector: Some(selector),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Listed { response } => {
            let session = response
                .sessions
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("unknown session: {id}"))?;
            Ok(tool_success(
                format!("found {}", session.id),
                &json!({ "session": session }),
            ))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn agent_capture(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let selector = required_selector(arguments, "selector")
        .or_else(|_| required_string(arguments, "id").and_then(selector_from_id))?;
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Capture {
                request: CaptureRequest {
                    selector,
                    scrollback_lines: optional_u64(arguments, "scrollback_lines")
                        .map(u32::try_from)
                        .transpose()
                        .map_err(|_| anyhow!("scrollback_lines is out of range"))?,
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Capture { response } => Ok(tool_success(
            format!("captured {}", response.session.id),
            &json!({ "session": response.session, "capture": response.capture }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn agent_delete(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let selector = required_selector(arguments, "selector")
        .or_else(|_| required_string(arguments, "id").and_then(selector_from_id))?;
    let signal = optional_string(arguments, "signal")
        .unwrap_or("SIGTERM")
        .to_string();
    let grace_secs = optional_u64(arguments, "grace_secs").unwrap_or(5);
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Delete {
                request: DeleteRequest {
                    selector,
                    signal,
                    grace_secs,
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Deleted { response } => {
            let text = format!("deleted {} session(s)", response.sessions.len());
            Ok(tool_success(
                text,
                &json!({ "sessions": response.sessions, "errors": response.errors }),
            ))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn agent_label(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let selector = required_selector(arguments, "selector")?;
    let mutation = LabelMutation::from_str(required_string(arguments, "mutation")?)?;
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Label {
                request: LabelRequest { selector, mutation },
            },
        )
        .await;
    match response.response {
        RpcResponse::Labeled { response } => Ok(tool_success(
            format!("labeled {} session(s)", response.sessions.len()),
            &json!({ "sessions": response.sessions, "errors": response.errors }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn mail_send(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::MailSend {
                request: MailSendRequest {
                    from: optional_string(arguments, "from").map(ToString::to_string),
                    to: required_selector(arguments, "to")?,
                    content: required_string(arguments, "content")?.to_string(),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::MailSent { response } => Ok(tool_success(
            format!("sent {} mail item(s)", response.mail.len()),
            &json!({ "mail": response.mail, "errors": response.errors }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn mail_read(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::MailRead {
                request: MailReadRequest {
                    selector: required_selector(arguments, "selector")
                        .or_else(|_| required_selector(arguments, "from"))?,
                    peek: optional_bool(arguments, "peek").unwrap_or(false),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::MailRead { response } => {
            let count = response.mail.len();
            Ok(tool_success(
                format!("{count} mail item(s)"),
                &json!({ "mail": response.mail, "errors": response.errors }),
            ))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn mail_check(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    mail_count_tool(
        state,
        context,
        RpcRequest::MailCheck {
            request: MailCheckRequest {
                selector: required_selector(arguments, "selector")
                    .or_else(|_| required_selector(arguments, "from"))?,
            },
        },
    )
    .await
}

async fn mail_stop_check(
    state: &DaemonState,
    context: &RequestContext,
    arguments: &Value,
) -> Result<Value> {
    mail_count_tool(
        state,
        context,
        RpcRequest::MailStopCheck {
            request: MailStopCheckRequest {
                selector: required_selector(arguments, "selector")
                    .or_else(|_| required_selector(arguments, "from"))?,
            },
        },
    )
    .await
}

async fn nudge(state: &DaemonState, context: &RequestContext, arguments: &Value) -> Result<Value> {
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Nudge {
                request: NudgeRequest {
                    to: required_selector(arguments, "to")?,
                    content: required_string(arguments, "content")?.to_string(),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Nudged { response } => Ok(tool_success(
            format!("nudged {} session(s)", response.nudges.len()),
            &json!({
                "nudges": response.nudges,
                "errors": response.errors
            }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn link(state: &DaemonState, context: &RequestContext, arguments: &Value) -> Result<Value> {
    let session_id = optional_string(arguments, "session_id")
        .map(uuid::Uuid::parse_str)
        .transpose()?;
    let selector = optional_selector(arguments, "selector")?;
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Link {
                request: LinkRequest {
                    session_id,
                    selector,
                    runtime_session: required_string(arguments, "runtime_session")?.to_string(),
                    transcript_path: required_string(arguments, "transcript")?.into(),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Linked { response } => Ok(tool_success(
            format!("linked {}", response.session.id),
            &json!({ "session": response.session }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn logs(state: &DaemonState, context: &RequestContext, arguments: &Value) -> Result<Value> {
    let selector = required_selector(arguments, "selector")
        .or_else(|_| required_string(arguments, "id").and_then(selector_from_id))?;
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Logs {
                request: LogsRequest {
                    selector,
                    max_bytes: optional_u64(arguments, "max_bytes"),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Logs { response } => Ok(tool_success(
            format!("logs {}", response.session.id),
            &json!({
                "session": response.session,
                "transcript_path": response.transcript_path,
                "content": response.content
            }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn wait(state: &DaemonState, context: &RequestContext, arguments: &Value) -> Result<Value> {
    let condition = WaitCondition::from_str(required_string(arguments, "for")?)?;
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Wait {
                request: WaitRequest {
                    selector: required_selector(arguments, "selector")?,
                    condition,
                    timeout_secs: optional_u64(arguments, "timeout_secs").unwrap_or(30),
                },
            },
        )
        .await;
    match response.response {
        RpcResponse::Wait { response } => Ok(tool_success(
            format!("wait matched: {}", response.matched),
            &json!({ "matched": response.matched, "sessions": response.sessions }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn doctor(
    state: &DaemonState,
    context: &RequestContext,
    _arguments: &Value,
) -> Result<Value> {
    let response = state
        .handle_direct(
            context.clone(),
            RpcRequest::Doctor {
                request: DoctorRequest::default(),
            },
        )
        .await;
    match response.response {
        RpcResponse::Doctor { response } => Ok(tool_success(
            format!("doctor {}", response.status),
            &json!({
                "status": response.status,
                "runtime": response.runtime,
                "findings": response.findings
            }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

async fn mail_count_tool(
    state: &DaemonState,
    context: &RequestContext,
    request: RpcRequest,
) -> Result<Value> {
    match state.handle_direct(context.clone(), request).await.response {
        RpcResponse::MailChecked { response } => {
            Ok(unread_tool_response(response.unread, &response.counts))
        }
        RpcResponse::MailStopChecked { response } => {
            Ok(unread_tool_response(response.unread, &response.counts))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn unread_tool_response(unread: usize, counts: &[sm_core::MailUnreadCount]) -> Value {
    tool_success(
        format!("{unread} unread"),
        &json!({ "unread": unread, "counts": counts }),
    )
}

fn required_string<'a>(arguments: &'a Value, field: &str) -> Result<&'a str> {
    optional_string(arguments, field).ok_or_else(|| anyhow!("missing required argument `{field}`"))
}

fn optional_string<'a>(arguments: &'a Value, field: &str) -> Option<&'a str> {
    arguments.get(field).and_then(Value::as_str)
}

fn optional_u64(arguments: &Value, field: &str) -> Option<u64> {
    arguments.get(field).and_then(Value::as_u64)
}

fn optional_bool(arguments: &Value, field: &str) -> Option<bool> {
    arguments.get(field).and_then(Value::as_bool)
}

fn required_selector(arguments: &Value, field: &str) -> Result<Selector> {
    Selector::from_str(required_string(arguments, field)?).map_err(Into::into)
}

fn optional_selector(arguments: &Value, field: &str) -> Result<Option<Selector>> {
    optional_string(arguments, field)
        .map(Selector::from_str)
        .transpose()
        .map_err(Into::into)
}

fn selector_from_id(id: &str) -> Result<Selector> {
    Ok(Selector::Id {
        id: uuid::Uuid::parse_str(id)?,
    })
}

fn optional_labels(arguments: &Value) -> Result<Vec<Label>> {
    let Some(value) = arguments.get("labels") else {
        return Ok(Vec::new());
    };
    let labels = value
        .as_array()
        .ok_or_else(|| anyhow!("`labels` must be an array of key=value strings"))?
        .iter()
        .map(|value| {
            let label = value
                .as_str()
                .ok_or_else(|| anyhow!("`labels` entries must be strings"))?;
            Label::from_str(label).map_err(Into::into)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(labels)
}

fn json_rpc_result(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    }
}

fn json_rpc_failure(id: Value, error: JsonRpcError) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(error),
    }
}

fn json_rpc_error(code: i32, message: impl Into<String>) -> JsonRpcError {
    JsonRpcError {
        code,
        message: message.into(),
        data: None,
    }
}

fn server_instructions() -> String {
    let overview = contract_registry()
        .tools()
        .iter()
        .map(|tool| format!("- {}: {}", tool.name, tool.mcp_description))
        .collect::<Vec<_>>()
        .join("\n");
    format!("session-matters controls local Helioy agent sessions.\n\n{overview}")
}
