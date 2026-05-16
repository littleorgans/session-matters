use serde::{Deserialize, Serialize};

use crate::{Mail, RuntimeKind, Session};

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
pub struct MailSendRequest {
    pub from: Option<String>,
    pub to: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailSendResponse {
    pub mail: Mail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailReadRequest {
    pub from: String,
    pub peek: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailReadResponse {
    pub mail: Vec<Mail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailCheckRequest {
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailCheckResponse {
    pub unread: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailStopCheckRequest {
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailStopCheckResponse {
    pub unread: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NudgeRequest {
    pub to: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NudgeResponse {
    pub to: String,
    pub delivered: bool,
    pub message: String,
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
    MailSend { request: MailSendRequest },
    MailRead { request: MailReadRequest },
    MailCheck { request: MailCheckRequest },
    MailStopCheck { request: MailStopCheckRequest },
    Nudge { request: NudgeRequest },
    McpBridge { request: McpBridgeRequest },
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RpcResponse {
    Spawned { response: SpawnResponse },
    Listed { response: ListResponse },
    Deleted { response: DeleteResponse },
    MailSent { response: MailSendResponse },
    MailRead { response: MailReadResponse },
    MailChecked { response: MailCheckResponse },
    MailStopChecked { response: MailStopCheckResponse },
    Nudged { response: NudgeResponse },
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

    #[test]
    fn mail_request_round_trips_as_tagged_json() {
        let request = RpcRequest::MailSend {
            request: MailSendRequest {
                from: Some("019e32e3-0000-7000-8000-000000000000".to_string()),
                to: "019e32e3-0000-7000-8000-000000000001".to_string(),
                content: "review the spec".to_string(),
            },
        };

        let json = serde_json::to_string(&request).expect("serializes request");
        let decoded: RpcRequest = serde_json::from_str(&json).expect("decodes request");

        assert_eq!(decoded, request);
    }

    #[test]
    fn nudge_request_round_trips_as_tagged_json() {
        let request = RpcRequest::Nudge {
            request: NudgeRequest {
                to: "019e32e3-0000-7000-8000-000000000001".to_string(),
                content: "ping".to_string(),
            },
        };

        let json = serde_json::to_string(&request).expect("serializes request");
        let decoded: RpcRequest = serde_json::from_str(&json).expect("decodes request");

        assert_eq!(decoded, request);
    }
}
