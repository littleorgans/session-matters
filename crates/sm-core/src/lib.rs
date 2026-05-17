pub mod error;
pub mod mcp;
pub mod paths;
pub mod proto;
pub mod tool_contracts;
pub mod types;

pub use error::{SmError, SmResult};
pub use mcp::{
    JsonRpcError, JsonRpcRequest, JsonRpcResponse, MCP_PROTOCOL_VERSION, tool_error, tool_success,
};
pub use paths::SmPaths;
pub use proto::{
    DaemonStatus, DeleteRequest, DeleteResponse, LabelRequest, LabelResponse, ListRequest,
    ListResponse, MailCheckRequest, MailCheckResponse, MailReadRequest, MailReadResponse,
    MailSendRequest, MailSendResponse, MailStopCheckRequest, MailStopCheckResponse,
    MailUnreadCount, McpBridgeRequest, McpBridgeResponse, NudgeDelivery, NudgeRequest,
    NudgeResponse, RpcRequest, RpcResponse, ShutdownResponse, SpawnRequest, SpawnResponse,
    TargetError,
};
pub use types::{
    Channel, Label, LabelMutation, LabelOp, Mail, MailStatus, RuntimeKind, Selector, Session,
    SessionState,
};
