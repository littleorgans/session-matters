pub mod agent_config;
pub mod events;
pub mod handler;
pub mod identity_client;
pub mod lifecycle;
pub mod mcp_bridge;
mod namespace;
pub mod polish;
pub mod reconcile;
pub mod server;
pub mod socket;
mod spawn_request;

pub use server::run_daemon;
pub use socket::send_request;
