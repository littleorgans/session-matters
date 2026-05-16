use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use chrono::Utc;
use sm_core::{
    ListRequest, ListResponse, RpcRequest, RpcResponse, Session, SessionState, ShutdownResponse,
    SpawnResponse,
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
}

fn response(result: Result<RpcResponse>, shutdown: bool) -> HandlerResult {
    HandlerResult {
        response: result.unwrap_or_else(|error| RpcResponse::Error {
            message: format!("{error:#}"),
        }),
        shutdown,
    }
}
