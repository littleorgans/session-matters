use serde::{Deserialize, Serialize};

use crate::{RuntimeKind, Session};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnRequest {
    pub runtime: RuntimeKind,
    pub role: String,
    pub workspace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnResponse {
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListRequest {
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListResponse {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteRequest {
    pub id: String,
    pub signal: String,
    pub grace_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteResponse {
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpBridgeRequest {
    pub line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpBridgeResponse {
    pub line: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShutdownResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub pidfile: String,
    pub socket: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcRequest {
    Spawn { request: SpawnRequest },
    List { request: ListRequest },
    Delete { request: DeleteRequest },
    McpBridge { request: McpBridgeRequest },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcResponse {
    Spawned { response: SpawnResponse },
    Listed { response: ListResponse },
    Deleted { response: DeleteResponse },
    McpBridge { response: McpBridgeResponse },
    Shutdown { response: ShutdownResponse },
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_request_round_trips_as_tagged_json() {
        let request = RpcRequest::Spawn {
            request: SpawnRequest {
                runtime: RuntimeKind::Claude,
                role: "general".to_string(),
                workspace: "test".to_string(),
            },
        };

        let json = serde_json::to_string(&request).expect("serializes request");
        let decoded: RpcRequest = serde_json::from_str(&json).expect("decodes request");

        assert_eq!(decoded, request);
    }

    #[test]
    fn delete_request_round_trips_as_tagged_json() {
        let request = RpcRequest::Delete {
            request: DeleteRequest {
                id: "019e32e3-0000-7000-8000-000000000000".to_string(),
                signal: "SIGTERM".to_string(),
                grace_secs: 5,
            },
        };

        let json = serde_json::to_string(&request).expect("serializes request");
        let decoded: RpcRequest = serde_json::from_str(&json).expect("decodes request");

        assert_eq!(decoded, request);
    }
}
