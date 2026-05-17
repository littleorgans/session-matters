pub mod handler;
pub mod identity_client;
pub mod lifecycle;
pub mod mcp_bridge;
pub mod server;
pub mod socket;

pub use server::run_daemon;
pub use socket::send_request;
