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
    DaemonStatus, DeleteRequest, DeleteResponse, ListRequest, ListResponse, McpBridgeRequest,
    McpBridgeResponse, RpcRequest, RpcResponse, ShutdownResponse, SpawnRequest, SpawnResponse,
};
pub use types::{RuntimeKind, Session, SessionState};
