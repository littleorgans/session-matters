use std::str::FromStr;

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use sm_core::{
    DeleteRequest, JsonRpcError, JsonRpcRequest, JsonRpcResponse, ListRequest,
    MCP_PROTOCOL_VERSION, MailCheckRequest, MailReadRequest, MailSendRequest, MailStopCheckRequest,
    NudgeRequest, RpcRequest, RpcResponse, RuntimeKind, SpawnRequest,
    tool_contracts::contract_registry, tool_error, tool_success,
};

use crate::handler::DaemonState;

pub fn handle_line(state: &DaemonState, line: &str) -> Option<String> {
    let response = match serde_json::from_str::<JsonRpcRequest>(line) {
        Ok(request) => handle_request(state, request)?,
        Err(error) => JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Value::Null,
            result: None,
            error: Some(json_rpc_error(-32700, format!("Parse error: {error}"))),
        },
    };
    Some(serde_json::to_string(&response).expect("JSON-RPC response serializes"))
}

fn handle_request(state: &DaemonState, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = request.id.unwrap_or(Value::Null);
    if request.method.starts_with("notifications/") {
        return None;
    }

    let result = match request.method.as_str() {
        "initialize" => Ok(initialize_result()),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(contract_registry().tool_list_value()),
        "tools/call" => handle_tool_call(state, request.params),
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

fn handle_tool_call(state: &DaemonState, params: Option<Value>) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| json_rpc_error(-32602, "Missing params"))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| json_rpc_error(-32602, "Missing tool name"))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    Ok(match call_tool(state, name, &arguments) {
        Ok(value) => value,
        Err(error) => tool_error(error.to_string()),
    })
}

fn call_tool(state: &DaemonState, name: &str, arguments: &Value) -> Result<Value> {
    match name {
        "agent_run" => agent_run(state, arguments),
        "agent_list" => agent_list(state, arguments),
        "agent_get" => agent_get(state, arguments),
        "agent_delete" => agent_delete(state, arguments),
        "mail_send" => mail_send(state, arguments),
        "mail_read" => mail_read(state, arguments),
        "mail_check" => mail_check(state, arguments),
        "mail_stop_check" => mail_stop_check(state, arguments),
        "nudge" => nudge(state, arguments),
        other => Ok(tool_error(format!("Unknown tool: {other}"))),
    }
}

fn agent_run(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let runtime = RuntimeKind::from_str(required_string(arguments, "runtime")?)?;
    let role = required_string(arguments, "role")?.to_string();
    let workspace = required_string(arguments, "workspace")?.to_string();
    let response = state.handle(RpcRequest::Spawn {
        request: SpawnRequest {
            runtime,
            role,
            workspace,
        },
    });
    match response.response {
        RpcResponse::Spawned { response } => {
            let text = format!("spawned {}", response.session.id);
            Ok(tool_success(text, &json!({ "session": response.session })))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn agent_list(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let id = optional_string(arguments, "id").map(ToString::to_string);
    let response = state.handle(RpcRequest::List {
        request: ListRequest { id },
    });
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

fn agent_get(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let id = required_string(arguments, "id")?.to_string();
    let response = state.handle(RpcRequest::List {
        request: ListRequest {
            id: Some(id.clone()),
        },
    });
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

fn agent_delete(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let id = required_string(arguments, "id")?.to_string();
    let signal = optional_string(arguments, "signal")
        .unwrap_or("SIGTERM")
        .to_string();
    let grace_secs = optional_u64(arguments, "grace_secs").unwrap_or(5);
    let response = state.handle(RpcRequest::Delete {
        request: DeleteRequest {
            id,
            signal,
            grace_secs,
        },
    });
    match response.response {
        RpcResponse::Deleted { response } => {
            let text = format!("deleted {} {}", response.session.id, response.session.state);
            Ok(tool_success(text, &json!({ "session": response.session })))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn mail_send(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let response = state.handle(RpcRequest::MailSend {
        request: MailSendRequest {
            from: optional_string(arguments, "from").map(ToString::to_string),
            to: required_string(arguments, "to")?.to_string(),
            content: required_string(arguments, "content")?.to_string(),
        },
    });
    match response.response {
        RpcResponse::MailSent { response } => Ok(tool_success(
            format!("sent {}", response.mail.id),
            &json!({ "mail": response.mail }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn mail_read(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let response = state.handle(RpcRequest::MailRead {
        request: MailReadRequest {
            from: required_string(arguments, "from")?.to_string(),
            peek: optional_bool(arguments, "peek").unwrap_or(false),
        },
    });
    match response.response {
        RpcResponse::MailRead { response } => {
            let count = response.mail.len();
            Ok(tool_success(
                format!("{count} mail item(s)"),
                &json!({ "mail": response.mail }),
            ))
        }
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn mail_check(state: &DaemonState, arguments: &Value) -> Result<Value> {
    mail_count_tool(
        state,
        RpcRequest::MailCheck {
            request: MailCheckRequest {
                from: required_string(arguments, "from")?.to_string(),
            },
        },
    )
}

fn mail_stop_check(state: &DaemonState, arguments: &Value) -> Result<Value> {
    mail_count_tool(
        state,
        RpcRequest::MailStopCheck {
            request: MailStopCheckRequest {
                from: required_string(arguments, "from")?.to_string(),
            },
        },
    )
}

fn nudge(state: &DaemonState, arguments: &Value) -> Result<Value> {
    let response = state.handle(RpcRequest::Nudge {
        request: NudgeRequest {
            to: required_string(arguments, "to")?.to_string(),
            content: required_string(arguments, "content")?.to_string(),
        },
    });
    match response.response {
        RpcResponse::Nudged { response } => Ok(tool_success(
            response.message.clone(),
            &json!({
                "to": response.to,
                "delivered": response.delivered,
                "message": response.message
            }),
        )),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn mail_count_tool(state: &DaemonState, request: RpcRequest) -> Result<Value> {
    match state.handle(request).response {
        RpcResponse::MailChecked { response } => Ok(unread_tool_response(response.unread)),
        RpcResponse::MailStopChecked { response } => Ok(unread_tool_response(response.unread)),
        RpcResponse::Error { message } => Err(anyhow!(message)),
        other => Err(anyhow!("unexpected daemon response: {other:?}")),
    }
}

fn unread_tool_response(unread: usize) -> Value {
    tool_success(format!("{unread} unread"), &json!({ "unread": unread }))
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
