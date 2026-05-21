use std::path::PathBuf;
use std::str::FromStr;

use lilo_rm_core::{LaunchEnv, ShellResume};
use serde::{Deserialize, Serialize};

use crate::{LabelMutation, Mail, Namespace, RuntimeKind, Selector, Session, SmError, SmResult};

fn default_spawn_target() -> String {
    "headless".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnRequest {
    pub runtime: RuntimeKind,
    pub role: String,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub namespace: Option<Namespace>,
    #[serde(default = "default_spawn_target")]
    pub target: String,
    #[serde(default)]
    pub agent_config: Option<String>,
    #[serde(default)]
    pub env: Vec<LaunchEnv>,
    #[serde(default)]
    pub shell_resume: Option<ShellResume>,
    #[serde(default)]
    pub labels: Vec<crate::Label>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpawnResponse {
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ListRequest {
    pub selector: Option<Selector>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListResponse {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteRequest {
    pub selector: Selector,
    pub signal: String,
    pub grace_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeleteResponse {
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub errors: Vec<TargetError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailSendRequest {
    pub from: Option<String>,
    pub to: Selector,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailSendResponse {
    pub mail: Vec<Mail>,
    #[serde(default)]
    pub errors: Vec<TargetError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailReadRequest {
    pub selector: Selector,
    pub peek: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailReadResponse {
    pub mail: Vec<Mail>,
    #[serde(default)]
    pub errors: Vec<TargetError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailCheckRequest {
    pub selector: Selector,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailCheckResponse {
    pub unread: usize,
    pub counts: Vec<MailUnreadCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailStopCheckRequest {
    pub selector: Selector,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailStopCheckResponse {
    pub unread: usize,
    pub counts: Vec<MailUnreadCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NudgeRequest {
    pub to: Selector,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NudgeResponse {
    pub nudges: Vec<NudgeDelivery>,
    #[serde(default)]
    pub errors: Vec<TargetError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NudgeDelivery {
    pub to: String,
    pub delivered: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailUnreadCount {
    pub session_id: String,
    pub unread: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabelRequest {
    pub selector: Selector,
    pub mutation: LabelMutation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabelResponse {
    pub sessions: Vec<Session>,
    #[serde(default)]
    pub errors: Vec<TargetError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkRequest {
    pub session_id: Option<uuid::Uuid>,
    pub selector: Option<Selector>,
    pub runtime_session: String,
    pub transcript_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkResponse {
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogsRequest {
    pub selector: Selector,
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogsResponse {
    pub session: Session,
    pub transcript_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureRequest {
    pub selector: Selector,
    #[serde(default)]
    pub scrollback_lines: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CaptureResponse {
    pub session: Session,
    pub capture: lilo_rm_core::CaptureResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DoctorRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorResponse {
    pub status: String,
    pub runtime: String,
    pub runtime_matters: RuntimeDoctorReport,
    pub findings: Vec<DoctorFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeDoctorReport {
    pub status: String,
    pub doctor: Option<Box<lilo_rm_core::DoctorResponse>>,
    pub socket_path: Option<String>,
    pub code: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DoctorFinding {
    pub severity: String,
    pub session_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaitRequest {
    pub selector: Selector,
    pub condition: WaitCondition,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WaitCondition {
    Running,
    Terminated,
    Count { count: usize },
}

impl FromStr for WaitCondition {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        match value {
            "running" => Ok(Self::Running),
            "terminated" => Ok(Self::Terminated),
            raw => {
                let Some(count) = raw.strip_prefix("count=") else {
                    return Err(SmError::Message(format!(
                        "unsupported wait condition: {raw}"
                    )));
                };
                Ok(Self::Count {
                    count: count
                        .parse()
                        .map_err(|_| SmError::Message(format!("invalid wait count: {count}")))?,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaitResponse {
    pub matched: bool,
    pub sessions: Vec<Session>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetError {
    pub target: String,
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
    pub endpoint: String,
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
    Label { request: LabelRequest },
    Link { request: LinkRequest },
    Logs { request: LogsRequest },
    Capture { request: CaptureRequest },
    Doctor { request: DoctorRequest },
    Wait { request: WaitRequest },
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
    Labeled { response: LabelResponse },
    Linked { response: LinkResponse },
    Logs { response: LogsResponse },
    Capture { response: CaptureResponse },
    Doctor { response: DoctorResponse },
    Wait { response: WaitResponse },
    McpBridge { response: McpBridgeResponse },
    Shutdown { response: ShutdownResponse },
    Error { message: String },
}

impl RpcResponse {
    pub fn kind(&self) -> &'static str {
        match self {
            RpcResponse::Spawned { .. } => "Spawned",
            RpcResponse::Listed { .. } => "Listed",
            RpcResponse::Deleted { .. } => "Deleted",
            RpcResponse::MailSent { .. } => "MailSent",
            RpcResponse::MailRead { .. } => "MailRead",
            RpcResponse::MailChecked { .. } => "MailChecked",
            RpcResponse::MailStopChecked { .. } => "MailStopChecked",
            RpcResponse::Nudged { .. } => "Nudged",
            RpcResponse::Labeled { .. } => "Labeled",
            RpcResponse::Linked { .. } => "Linked",
            RpcResponse::Logs { .. } => "Logs",
            RpcResponse::Capture { .. } => "Capture",
            RpcResponse::Doctor { .. } => "Doctor",
            RpcResponse::Wait { .. } => "Wait",
            RpcResponse::McpBridge { .. } => "McpBridge",
            RpcResponse::Shutdown { .. } => "Shutdown",
            RpcResponse::Error { .. } => "Error",
        }
    }
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
                dir: None,
                namespace: None,
                target: "headless".to_string(),
                agent_config: None,
                env: Vec::new(),
                shell_resume: None,
                labels: Vec::new(),
            },
        };

        let json = serde_json::to_string(&request).expect("serializes request");
        let decoded: RpcRequest = serde_json::from_str(&json).expect("decodes request");

        assert_eq!(decoded, request);
    }

    #[test]
    fn spawn_request_decodes_legacy_payload_without_new_fields() {
        let json = r#"{
            "type": "spawn",
            "request": {
                "runtime": "claude",
                "role": "general",
                "workspace": "/tmp/project"
            }
        }"#;

        let decoded: RpcRequest = serde_json::from_str(json).expect("decodes legacy request");
        let RpcRequest::Spawn { request } = decoded else {
            panic!("expected spawn request");
        };
        assert_eq!(request.workspace, "/tmp/project");
        assert_eq!(request.dir, None);
        assert_eq!(request.namespace, None);
        assert_eq!(request.target, "headless");
    }

    #[test]
    fn spawn_request_decodes_new_payload_without_legacy_workspace() {
        let json = r#"{
            "type": "spawn",
            "request": {
                "runtime": "claude",
                "role": "general",
                "dir": "/tmp/project",
                "namespace": "alpha"
            }
        }"#;

        let decoded: RpcRequest = serde_json::from_str(json).expect("decodes new request");
        let RpcRequest::Spawn { request } = decoded else {
            panic!("expected spawn request");
        };
        assert_eq!(request.workspace, "");
        assert_eq!(request.dir.as_deref(), Some("/tmp/project"));
        assert_eq!(request.namespace.unwrap().as_str(), "alpha");
        assert_eq!(request.target, "headless");
    }

    #[test]
    fn delete_request_round_trips_as_tagged_json() {
        let request = RpcRequest::Delete {
            request: DeleteRequest {
                selector: Selector::Id {
                    id: "019e32e3-0000-7000-8000-000000000000".parse().unwrap(),
                },
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
                to: Selector::Id {
                    id: "019e32e3-0000-7000-8000-000000000001".parse().unwrap(),
                },
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
                to: Selector::Id {
                    id: "019e32e3-0000-7000-8000-000000000001".parse().unwrap(),
                },
                content: "ping".to_string(),
            },
        };

        let json = serde_json::to_string(&request).expect("serializes request");
        let decoded: RpcRequest = serde_json::from_str(&json).expect("decodes request");

        assert_eq!(decoded, request);
    }
}
