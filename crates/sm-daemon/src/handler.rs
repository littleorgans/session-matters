use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use sm_core::{
    DeleteRequest, DeleteResponse, ListRequest, ListResponse, McpBridgeResponse, RpcRequest,
    RpcResponse, Session, SessionState, ShutdownResponse, SpawnResponse,
};
use sm_driver::SpawnDriver;
use sm_store::SqliteStore;
use uuid::Uuid;

pub struct DaemonState {
    pub store: Mutex<SqliteStore>,
    pub driver: Arc<dyn SpawnDriver>,
}

pub struct HandlerResult {
    pub response: RpcResponse,
    pub shutdown: bool,
}

impl DaemonState {
    pub fn new(store: SqliteStore, driver: Arc<dyn SpawnDriver>) -> Self {
        Self {
            store: Mutex::new(store),
            driver,
        }
    }

    pub fn handle(&self, request: RpcRequest) -> HandlerResult {
        match request {
            RpcRequest::Spawn { request } => response(self.spawn(request), false),
            RpcRequest::List { request } => response(self.list(request), false),
            RpcRequest::Delete { request } => response(self.delete(request), false),
            RpcRequest::McpBridge { request } => HandlerResult {
                response: RpcResponse::McpBridge {
                    response: McpBridgeResponse {
                        line: crate::mcp_bridge::handle_line(self, &request.line),
                    },
                },
                shutdown: false,
            },
            RpcRequest::Shutdown => HandlerResult {
                response: RpcResponse::Shutdown {
                    response: ShutdownResponse {
                        message: "stopping".to_string(),
                    },
                },
                shutdown: true,
            },
        }
    }

    fn spawn(&self, request: sm_core::SpawnRequest) -> Result<RpcResponse> {
        let id = Uuid::now_v7();
        let spawned = self
            .driver
            .spawn(&id.to_string(), &request)
            .context("spawn driver failed")?;
        let now = Utc::now();
        let session = Session {
            id,
            runtime: request.runtime,
            role: request.role,
            workspace: request.workspace,
            state: SessionState::Running,
            runtime_pid: spawned.runtime_pid,
            created_at: now,
            started_at: now,
            terminated_at: None,
            exit_code: None,
            updated_at: now,
        };

        self.store
            .lock()
            .expect("store lock poisoned")
            .insert_session(&session)
            .context("failed to persist session")?;

        Ok(RpcResponse::Spawned {
            response: SpawnResponse { session },
        })
    }

    fn list(&self, request: ListRequest) -> Result<RpcResponse> {
        crate::lifecycle::refresh_exits(self)?;
        let sessions = self
            .store
            .lock()
            .expect("store lock poisoned")
            .list_sessions(request.id.as_deref())
            .context("failed to list sessions")?;

        Ok(RpcResponse::Listed {
            response: ListResponse { sessions },
        })
    }

    fn delete(&self, request: DeleteRequest) -> Result<RpcResponse> {
        crate::lifecycle::refresh_exits(self)?;
        let id = Uuid::parse_str(&request.id).context("invalid session id")?;
        let session = self
            .store
            .lock()
            .expect("store lock poisoned")
            .get_session(&id)
            .context("failed to load session")?
            .with_context(|| format!("unknown session: {}", request.id))?;

        if session.state == SessionState::Terminated {
            return Ok(RpcResponse::Deleted {
                response: DeleteResponse { session },
            });
        }

        let exit = self
            .driver
            .terminate(
                &request.id,
                &request.signal,
                Duration::from_secs(request.grace_secs),
            )
            .context("failed to terminate runtime")?
            .with_context(|| format!("session is not owned by this daemon: {}", request.id))?;
        let session = self
            .store
            .lock()
            .expect("store lock poisoned")
            .mark_session_terminated(&id, exit.exit_code, Utc::now())
            .context("failed to persist terminated session")?
            .with_context(|| format!("unknown session: {}", request.id))?;

        Ok(RpcResponse::Deleted {
            response: DeleteResponse { session },
        })
    }
}

fn response(result: Result<RpcResponse>, shutdown: bool) -> HandlerResult {
    HandlerResult {
        response: result.unwrap_or_else(|error| RpcResponse::Error {
            message: format!("{error:#}"),
        }),
        shutdown,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use sm_core::{RuntimeKind, SpawnRequest};
    use sm_driver::{ChildExit, DriverError, SpawnDriver, SpawnedProcess};

    use super::*;

    struct MockDriver {
        exits: Mutex<Vec<ChildExit>>,
    }

    impl MockDriver {
        fn new() -> Self {
            Self {
                exits: Mutex::new(Vec::new()),
            }
        }
    }

    impl SpawnDriver for MockDriver {
        fn spawn(
            &self,
            _session_id: &str,
            _request: &SpawnRequest,
        ) -> Result<SpawnedProcess, DriverError> {
            Ok(SpawnedProcess { runtime_pid: 42 })
        }

        fn reap_exited(&self) -> Result<Vec<ChildExit>, DriverError> {
            Ok(self
                .exits
                .lock()
                .expect("exits lock poisoned")
                .drain(..)
                .collect())
        }

        fn terminate(
            &self,
            session_id: &str,
            _signal: &str,
            _grace: Duration,
        ) -> Result<Option<ChildExit>, DriverError> {
            Ok(Some(ChildExit {
                session_id: session_id.to_string(),
                runtime_pid: 42,
                exit_code: Some(143),
            }))
        }

        fn terminate_all(&self) {}
    }

    #[test]
    fn drives_session_through_delete_lifecycle() {
        let state = DaemonState::new(
            SqliteStore::open_in_memory().expect("store opens"),
            Arc::new(MockDriver::new()),
        );
        let spawned = state.handle(RpcRequest::Spawn {
            request: SpawnRequest {
                runtime: RuntimeKind::Claude,
                role: "general".to_string(),
                workspace: "test".to_string(),
            },
        });
        let RpcResponse::Spawned { response } = spawned.response else {
            panic!("expected spawn response");
        };
        assert_eq!(response.session.state, SessionState::Running);

        let deleted = state.handle(RpcRequest::Delete {
            request: DeleteRequest {
                id: response.session.id.to_string(),
                signal: "SIGTERM".to_string(),
                grace_secs: 5,
            },
        });
        let RpcResponse::Deleted { response } = deleted.response else {
            panic!("expected delete response");
        };

        assert_eq!(response.session.state, SessionState::Terminated);
        assert_eq!(response.session.exit_code, Some(143));
        assert!(response.session.terminated_at.is_some());
    }
}
