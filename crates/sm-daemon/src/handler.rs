use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use lilo_im_core::Action;
use sm_core::{
    CaptureRequest, CaptureResponse, DeleteRequest, DeleteResponse, LabelRequest, LabelResponse,
    ListRequest, ListResponse, Mail, MailCheckRequest, MailCheckResponse, MailReadRequest,
    MailReadResponse, MailSendRequest, MailSendResponse, MailStopCheckRequest,
    MailStopCheckResponse, MailUnreadCount, McpBridgeResponse, NudgeDelivery, NudgeRequest,
    NudgeResponse, RpcRequest, RpcResponse, Selector, Session, SessionState, ShutdownResponse,
    SpawnRequest, SpawnResponse, TargetError,
};
use sm_driver::{LaunchEnv, SpawnDriver, SpawnLaunch};
use sm_store::SqliteStore;
use uuid::Uuid;

use crate::agent_config::{ResolvedAgentConfig, resolve_agent_config};
use crate::identity_client::{IdentityClient, RequestContext, session_resource, spawn_resource};

pub struct DaemonState {
    pub store: Mutex<SqliteStore>,
    pub driver: Arc<dyn SpawnDriver>,
    pub(crate) identity: Arc<IdentityClient>,
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
            RpcRequest::List { request } => response(self.list(request).await, false),
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
            RpcRequest::Label { request } => response(self.label(&context, request).await, false),
            RpcRequest::Link { request } => response(self.link(&context, request).await, false),
            RpcRequest::Logs { request } => response(self.logs(&context, request).await, false),
            RpcRequest::Capture { request } => {
                response(self.capture(&context, request).await, false)
            }
            RpcRequest::Doctor { request } => response(self.doctor(&context, request).await, false),
            RpcRequest::Wait { request } => response(self.wait(request).await, false),
            RpcRequest::McpBridge { .. } => response(
                Err(anyhow::anyhow!(
                    "nested MCP bridge requests are not supported"
                )),
                false,
            ),
            RpcRequest::Shutdown => response(self.shutdown(&context).await, true),
        }
    }

    async fn spawn(&self, context: &RequestContext, request: SpawnRequest) -> Result<RpcResponse> {
        let id = Uuid::now_v7();
        let agent_config = resolve_agent_config(request.agent_config.as_deref())?;
        let launch = spawn_launch(id, &request, agent_config.as_ref());
        let mut labels = request.labels.clone();
        labels.sort();
        self.identity
            .authorize(
                &context.principal,
                Action::Spawn,
                &spawn_resource(&request, id),
            )
            .await?;
        self.driver
            .validate_target(&request.target)
            .await
            .context("runtime target validation failed")?;
        let spawned = self
            .driver
            .spawn(&id.to_string(), &launch)
            .await
            .context("spawn driver failed")?;
        let now = Utc::now();
        let session = Session {
            id,
            runtime: request.runtime,
            role: request.role,
            workspace: request.workspace,
            labels,
            state: SessionState::Running,
            runtime_pid: spawned.runtime_pid,
            runtime_session: None,
            transcript_path: spawned.stdout_path,
            tmux_pane: spawned.tmux_pane,
            agent_config: request.agent_config,
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

    async fn list(&self, request: ListRequest) -> Result<RpcResponse> {
        crate::lifecycle::refresh_exits(self).await?;
        let selector = request.selector.unwrap_or_default();
        let sessions = self
            .store
            .lock()
            .expect("store lock poisoned")
            .list_sessions_by_selector(&selector)
            .context("failed to list sessions")?;

        Ok(RpcResponse::Listed {
            response: ListResponse { sessions },
        })
    }

    async fn capture(
        &self,
        context: &RequestContext,
        request: CaptureRequest,
    ) -> Result<RpcResponse> {
        let session = self
            .resolve_selector(&request.selector, "capture")?
            .remove(0);
        self.identity
            .authorize(
                &context.principal,
                Action::Read,
                &session_resource(session.id),
            )
            .await?;
        let capture = self
            .driver
            .capture(&session.id.to_string(), request.scrollback_lines)
            .await
            .context("runtime capture failed")?
            .response;
        Ok(RpcResponse::Capture {
            response: CaptureResponse { session, capture },
        })
    }

    async fn delete(
        &self,
        context: &RequestContext,
        request: DeleteRequest,
    ) -> Result<RpcResponse> {
        let targets = self.resolve_selector(&request.selector, "agent")?;
        let mut sessions = Vec::new();
        let mut errors = Vec::new();
        for target in targets {
            match self.delete_one(context, &request, target.id).await {
                Ok(session) => sessions.push(session),
                Err(error) => errors.push(target_error(target.id, error)),
            }
        }

        Ok(RpcResponse::Deleted {
            response: DeleteResponse { sessions, errors },
        })
    }

    async fn mail_send(
        &self,
        context: &RequestContext,
        request: MailSendRequest,
    ) -> Result<RpcResponse> {
        let recipients = self.resolve_selector(&request.to, "recipient")?;
        let sender_id = match request.from {
            Some(from) => {
                let id = Uuid::parse_str(&from).context("invalid sender session id")?;
                self.require_session(&id, "sender")?;
                id
            }
            None => Uuid::nil(),
        };
        let mut mail = Vec::new();
        let mut errors = Vec::new();
        for recipient in recipients {
            match self
                .mail_send_one(context, sender_id, recipient.id, &request.content)
                .await
            {
                Ok(item) => mail.push(item),
                Err(error) => errors.push(target_error(recipient.id, error)),
            }
        }

        Ok(RpcResponse::MailSent {
            response: MailSendResponse { mail, errors },
        })
    }

    async fn mail_read(
        &self,
        context: &RequestContext,
        request: MailReadRequest,
    ) -> Result<RpcResponse> {
        let recipients = self.resolve_selector(&request.selector, "recipient")?;
        let mut mail = Vec::new();
        let mut errors = Vec::new();
        for recipient in recipients {
            match self
                .mail_read_one(context, recipient.id, request.peek)
                .await
            {
                Ok(mut items) => mail.append(&mut items),
                Err(error) => errors.push(target_error(recipient.id, error)),
            }
        }

        Ok(RpcResponse::MailRead {
            response: MailReadResponse { mail, errors },
        })
    }

    fn mail_check(&self, request: MailCheckRequest) -> Result<RpcResponse> {
        let counts = self.mail_counts(&request.selector)?;
        let unread = total_unread(&counts);
        Ok(RpcResponse::MailChecked {
            response: MailCheckResponse { unread, counts },
        })
    }

    fn mail_stop_check(&self, request: MailStopCheckRequest) -> Result<RpcResponse> {
        let counts = self.mail_counts(&request.selector)?;
        let unread = total_unread(&counts);
        Ok(RpcResponse::MailStopChecked {
            response: MailStopCheckResponse { unread, counts },
        })
    }

    async fn nudge(&self, context: &RequestContext, request: NudgeRequest) -> Result<RpcResponse> {
        let recipients = self.resolve_selector(&request.to, "recipient")?;
        let mut nudges = Vec::new();
        let mut errors = Vec::new();
        for recipient in recipients {
            match self
                .nudge_one(context, recipient.id, &request.content)
                .await
            {
                Ok(nudge) => nudges.push(nudge),
                Err(error) => errors.push(target_error(recipient.id, error)),
            }
        }

        Ok(RpcResponse::Nudged {
            response: NudgeResponse { nudges, errors },
        })
    }

    async fn label(&self, context: &RequestContext, request: LabelRequest) -> Result<RpcResponse> {
        let targets = self.resolve_selector(&request.selector, "agent")?;
        let mut sessions = Vec::new();
        let mut errors = Vec::new();
        for target in targets {
            match self.label_one(context, target.id, &request).await {
                Ok(session) => sessions.push(session),
                Err(error) => errors.push(target_error(target.id, error)),
            }
        }
        Ok(RpcResponse::Labeled {
            response: LabelResponse { sessions, errors },
        })
    }

    async fn delete_one(
        &self,
        context: &RequestContext,
        request: &DeleteRequest,
        id: Uuid,
    ) -> Result<Session> {
        self.identity
            .authorize(&context.principal, Action::Kill, &session_resource(id))
            .await?;
        crate::lifecycle::refresh_exits(self).await?;
        let id_string = id.to_string();
        let session = self
            .store
            .lock()
            .expect("store lock poisoned")
            .get_session(&id)
            .context("failed to load session")?
            .with_context(|| format!("unknown session: {id}"))?;
        if session.state == SessionState::Terminated {
            return Ok(session);
        }
        let exit = self
            .driver
            .terminate(
                &id_string,
                &request.signal,
                Duration::from_secs(request.grace_secs),
            )
            .await
            .context("failed to terminate runtime")?
            .with_context(|| {
                format!(
                    "runtime did not terminate within {} grace seconds: {id}",
                    request.grace_secs
                )
            })?;
        crate::lifecycle::persist_child_exit(self, exit)
            .context("failed to persist terminated session")?
            .with_context(|| format!("unknown session: {id}"))
    }

    async fn mail_send_one(
        &self,
        context: &RequestContext,
        sender_id: Uuid,
        recipient_id: Uuid,
        content: &str,
    ) -> Result<Mail> {
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
            content: content.to_string(),
            sent_at: Utc::now(),
            read_at: None,
        };
        self.store
            .lock()
            .expect("store lock poisoned")
            .insert_mail(&mail)
            .context("failed to persist mail")?;
        Ok(mail)
    }

    async fn mail_read_one(
        &self,
        context: &RequestContext,
        recipient_id: Uuid,
        peek: bool,
    ) -> Result<Vec<Mail>> {
        self.identity
            .authorize(
                &context.principal,
                Action::MailRead,
                &session_resource(recipient_id),
            )
            .await?;
        self.store
            .lock()
            .expect("store lock poisoned")
            .read_unread_mail(&recipient_id, Utc::now(), peek)
            .context("failed to read mail")
    }

    fn mail_counts(&self, selector: &Selector) -> Result<Vec<MailUnreadCount>> {
        let recipients = self.resolve_selector(selector, "recipient")?;
        recipients
            .iter()
            .map(|session| {
                Ok(MailUnreadCount {
                    session_id: session.id.to_string(),
                    unread: self.unread_mail_count(&session.id)?,
                })
            })
            .collect()
    }

    async fn nudge_one(
        &self,
        context: &RequestContext,
        recipient_id: Uuid,
        content: &str,
    ) -> Result<NudgeDelivery> {
        self.identity
            .authorize(
                &context.principal,
                Action::Nudge,
                &session_resource(recipient_id),
            )
            .await?;
        let to = recipient_id.to_string();
        let result = self
            .driver
            .nudge(&to, content)
            .await
            .context("nudge driver failed")?;
        Ok(NudgeDelivery {
            to,
            delivered: result.delivered,
            message: result.message,
        })
    }

    async fn label_one(
        &self,
        context: &RequestContext,
        target_id: Uuid,
        request: &LabelRequest,
    ) -> Result<Session> {
        self.identity
            .authorize(
                &context.principal,
                Action::Link,
                &session_resource(target_id),
            )
            .await?;
        self.store
            .lock()
            .expect("store lock poisoned")
            .apply_label_mutation(&target_id, &request.mutation)
            .context("failed to persist label")?
            .with_context(|| format!("unknown session: {target_id}"))
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

    fn unread_mail_count(&self, recipient_id: &Uuid) -> Result<usize> {
        self.require_session(recipient_id, "recipient")?;
        self.store
            .lock()
            .expect("store lock poisoned")
            .count_unread_mail(recipient_id)
            .context("failed to count unread mail")
    }

    pub(crate) fn resolve_selector(
        &self,
        selector: &Selector,
        label: &str,
    ) -> Result<Vec<Session>> {
        let sessions = self
            .store
            .lock()
            .expect("store lock poisoned")
            .list_sessions_by_selector(selector)
            .context("failed to resolve selector")?;
        if !sessions.is_empty() {
            return Ok(sessions);
        }
        match selector {
            Selector::Id { id } => anyhow::bail!("unknown {label} session: {id}"),
            _ => anyhow::bail!("{label} selector matched no sessions: {selector:?}"),
        }
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

fn target_error(id: Uuid, error: anyhow::Error) -> TargetError {
    TargetError {
        target: id.to_string(),
        message: format!("{error:#}"),
    }
}

fn total_unread(counts: &[MailUnreadCount]) -> usize {
    counts.iter().map(|count| count.unread).sum()
}

fn spawn_launch(
    id: Uuid,
    request: &SpawnRequest,
    agent_config: Option<&ResolvedAgentConfig>,
) -> SpawnLaunch {
    let mut env = agent_config
        .map(|config| config.env.clone())
        .unwrap_or_default();
    env.retain(|item| !item.key.starts_with("HELIOY_SESSION_"));
    env.push(LaunchEnv {
        key: "HELIOY_SESSION_ID".to_string(),
        value: id.to_string(),
    });
    env.push(LaunchEnv {
        key: "HELIOY_SESSION_ROLE".to_string(),
        value: request.role.clone(),
    });
    env.push(LaunchEnv {
        key: "HELIOY_SESSION_WORKSPACE".to_string(),
        value: request.workspace.clone(),
    });
    SpawnLaunch {
        runtime: request.runtime,
        cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        target: request.target.clone(),
        env,
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
