pub mod error;
pub mod mcp;
pub mod paths;
pub mod proto;
pub mod tool_contracts;
pub mod types;

pub use error::{SmError, SmResult, humanize_capture_error};
pub use mcp::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION, tool_error, tool_success,
};
pub use paths::{SmEndpoint, SmPaths, SmPathsError, rtmd_socket_path};
pub use proto::{
    CaptureRequest, CaptureResponse, DaemonStatus, DeleteRequest, DeleteResponse, DoctorFinding,
    DoctorRequest, DoctorResponse, LabelRequest, LabelResponse, LinkRequest, LinkResponse,
    ListRequest, ListResponse, LogsRequest, LogsResponse, MailCheckRequest, MailCheckResponse,
    MailReadRequest, MailReadResponse, MailSendRequest, MailSendResponse, MailStopCheckRequest,
    MailStopCheckResponse, MailUnreadCount, McpBridgeRequest, McpBridgeResponse, NudgeDelivery,
    NudgeRequest, NudgeResponse, RpcRequest, RpcResponse, RuntimeDoctorReport, ShutdownResponse,
    SpawnRequest, SpawnResponse, TargetError, WaitCondition, WaitRequest, WaitResponse,
};
pub use types::{
    Channel, Label, LabelMutation, LabelOp, LostEvidence, Mail, MailStatus, RuntimeKind, Selector,
    Session, SessionState,
};
