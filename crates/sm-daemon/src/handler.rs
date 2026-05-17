use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use im_core::Action;
use sm_core::{
    DeleteRequest, DeleteResponse, ListRequest, ListResponse, Mail, MailCheckRequest,
    MailCheckResponse, MailReadRequest, MailReadResponse, MailSendRequest, MailSendResponse,
    MailStopCheckRequest, MailStopCheckResponse, McpBridgeResponse, NudgeRequest, NudgeResponse,
    RpcRequest, RpcResponse, Session, SessionState, ShutdownResponse, SpawnResponse,
};
use sm_driver::SpawnDriver;
use sm_store::SqliteStore;
use uuid::Uuid;

use crate::identity_client::{IdentityClient, RequestContext, session_resource, spawn_resource};

pub struct DaemonState {
    pub store: Mutex<SqliteStore>,
    pub driver: Arc<dyn SpawnDriver>,
    identity: Arc<IdentityClient>,
}

pub struct HandlerResult {
    pub response: RpcResponse,
    pub shutdown: bool,
}

impl DaemonState {
    pub fn new(
        store: SqliteStore,
        driver: Arc<dyn SpawnDriver>,
        identity: Arc<IdentityClient>,
    ) -> Self {
        Self {
            store: Mutex::new(store),
            driver,
            identity,
        }
    }

    pub async fn handle(&self, context: RequestContext, request: RpcRequest) -> HandlerResult {
        match request {
            RpcRequest::McpBridge { request } => HandlerResult {
                response: RpcResponse::McpBridge {
                    response: McpBridgeResponse {
                        line: crate::mcp_bridge::handle_line(self, &context, &request.line).await,
                    },
                },
                shutdown: false,
            },
            request => self.handle_direct(context, request).await,
        }
    }

    pub(crate) async fn handle_direct(
        &self,
        context: RequestContext,
        request: RpcRequest,
    ) -> HandlerResult {
        match request {
            RpcRequest::Spawn { request } => response(self.spawn(&context, request).await, false),
            RpcRequest::List { request } => response(self.list(request), false),
            RpcRequest::Delete { request } => response(self.delete(&context, request).await, false),
            RpcRequest::MailSend { request } => {
                response(self.mail_send(&context, request).await, false)
            }
            RpcRequest::MailRead { request } => {
                response(self.mail_read(&context, request).await, false)
            }
            RpcRequest::MailCheck { request } => response(self.mail_check(request), false),
            RpcRequest::MailStopCheck { request } => response(self.mail_stop_check(request), false),
            RpcRequest::Nudge { request } => response(self.nudge(&context, request).await, false),
            RpcRequest::McpBridge { .. } => response(
                Err(anyhow::anyhow!(
                    "nested MCP bridge requests are not supported"
                )),
                false,
            ),
            RpcRequest::Shutdown => response(self.shutdown(&context).await, true),
        }
    }

    async fn spawn(
        &self,
        context: &RequestContext,
        request: sm_core::SpawnRequest,
    ) -> Result<RpcResponse> {
        let id = Uuid::now_v7();
        self.identity
            .authorize(
                &context.principal,
                Action::Spawn,
                &spawn_resource(&request, id),
            )
            .await?;
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

    async fn delete(
        &self,
        context: &RequestContext,
        request: DeleteRequest,
    ) -> Result<RpcResponse> {
        let id = Uuid::parse_str(&request.id).context("invalid session id")?;
        self.identity
            .authorize(&context.principal, Action::Kill, &session_resource(id))
            .await?;
        crate::lifecycle::refresh_exits(self)?;
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

    async fn mail_send(
        &self,
        context: &RequestContext,
        request: MailSendRequest,
    ) -> Result<RpcResponse> {
        let recipient_id = Uuid::parse_str(&request.to).context("invalid recipient session id")?;
        self.require_session(&recipient_id, "recipient")?;
        let sender_id = match request.from {
            Some(from) => {
                let id = Uuid::parse_str(&from).context("invalid sender session id")?;
                self.require_session(&id, "sender")?;
                id
            }
            None => Uuid::nil(),
        };
        self.identity
            .authorize(
                &context.principal,
                Action::MailSend,
                &session_resource(recipient_id),
            )
            .await?;
        let mail = Mail {
            id: Uuid::now_v7(),
            sender_id,
            recipient_id,
            content: request.content,
            sent_at: Utc::now(),
            read_at: None,
        };
        self.store
            .lock()
            .expect("store lock poisoned")
            .insert_mail(&mail)
            .context("failed to persist mail")?;

        Ok(RpcResponse::MailSent {
            response: MailSendResponse { mail },
        })
    }

    async fn mail_read(
        &self,
        context: &RequestContext,
        request: MailReadRequest,
    ) -> Result<RpcResponse> {
        let recipient_id = Uuid::parse_str(&request.from).context("invalid session id")?;
        self.require_session(&recipient_id, "recipient")?;
        self.identity
            .authorize(
                &context.principal,
                Action::MailRead,
                &session_resource(recipient_id),
            )
            .await?;
        let mail = self
            .store
            .lock()
            .expect("store lock poisoned")
            .read_unread_mail(&recipient_id, Utc::now(), request.peek)
            .context("failed to read mail")?;

        Ok(RpcResponse::MailRead {
            response: MailReadResponse { mail },
        })
    }

    fn mail_check(&self, request: MailCheckRequest) -> Result<RpcResponse> {
        let unread = self.unread_mail_count(&request.from)?;
        Ok(RpcResponse::MailChecked {
            response: MailCheckResponse { unread },
        })
    }

    fn mail_stop_check(&self, request: MailStopCheckRequest) -> Result<RpcResponse> {
        let unread = self.unread_mail_count(&request.from)?;
        Ok(RpcResponse::MailStopChecked {
            response: MailStopCheckResponse { unread },
        })
    }

    async fn nudge(&self, context: &RequestContext, request: NudgeRequest) -> Result<RpcResponse> {
        let recipient_id = Uuid::parse_str(&request.to).context("invalid recipient session id")?;
        self.require_session(&recipient_id, "recipient")?;
        self.identity
            .authorize(
                &context.principal,
                Action::Nudge,
                &session_resource(recipient_id),
            )
            .await?;
        let result = self
            .driver
            .nudge(&request.to, &request.content)
            .context("nudge driver failed")?;

        Ok(RpcResponse::Nudged {
            response: NudgeResponse {
                to: request.to,
                delivered: result.delivered,
                message: result.message,
            },
        })
    }

    async fn shutdown(&self, context: &RequestContext) -> Result<RpcResponse> {
        self.identity
            .authorize(&context.principal, Action::Daemon, &Default::default())
            .await?;
        Ok(RpcResponse::Shutdown {
            response: ShutdownResponse {
                message: "stopping".to_string(),
            },
        })
    }

    fn unread_mail_count(&self, session_id: &str) -> Result<usize> {
        let recipient_id = Uuid::parse_str(session_id).context("invalid session id")?;
        self.require_session(&recipient_id, "recipient")?;
        self.store
            .lock()
            .expect("store lock poisoned")
            .count_unread_mail(&recipient_id)
            .context("failed to count unread mail")
    }

    fn require_session(&self, id: &Uuid, label: &str) -> Result<()> {
        let exists = self
            .store
            .lock()
            .expect("store lock poisoned")
            .get_session(id)
            .context("failed to load session")?
            .is_some();
        anyhow::ensure!(exists, "unknown {label} session: {id}");
        Ok(())
    }
}

fn response(result: Result<RpcResponse>, shutdown_on_success: bool) -> HandlerResult {
    match result {
        Ok(response) => HandlerResult {
            response,
            shutdown: shutdown_on_success,
        },
        Err(error) => HandlerResult {
            response: RpcResponse::Error {
                message: format!("{error:#}"),
            },
            shutdown: false,
        },
    }
}
