pub mod error;
pub mod paths;
pub mod proto;
pub mod types;

pub use error::{SmError, SmResult};
pub use paths::SmPaths;
pub use proto::{
    DaemonStatus, DeleteRequest, DeleteResponse, ListRequest, ListResponse, RpcRequest,
    RpcResponse, ShutdownResponse, SpawnRequest, SpawnResponse,
};
pub use types::{RuntimeKind, Session, SessionState};
