use std::fs;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use lilo_im_core::Action;
use sm_core::{
    DoctorFinding, DoctorRequest, DoctorResponse, LinkRequest, LinkResponse, LogsRequest,
    LogsResponse, RpcResponse, Selector, Session, SessionState, WaitCondition, WaitRequest,
    WaitResponse,
};

use crate::handler::DaemonState;
use crate::identity_client::session_resource;

impl DaemonState {
    pub(crate) async fn link(
        &self,
        context: &crate::identity_client::RequestContext,
        request: LinkRequest,
    ) -> Result<RpcResponse> {
        if let Some(session) = self
            .store
            .lock()
            .expect("store lock poisoned")
            .get_session_by_runtime_session(&request.runtime_session)
            .context("failed to load runtime session link")?
        {
            return Ok(RpcResponse::Linked {
                response: LinkResponse { session },
            });
        }

        let session = self.link_target(&request)?;
        self.identity
            .authorize(
                &context.principal,
                Action::Link,
                &session_resource(session.id),
            )
            .await?;
        let session = self
            .store
            .lock()
            .expect("store lock poisoned")
            .link_session(
                &session.id,
                &request.runtime_session,
                &request.transcript_path,
                Utc::now(),
            )
            .context("failed to persist runtime session link")?
            .with_context(|| format!("unknown session: {}", session.id))?;

        Ok(RpcResponse::Linked {
            response: LinkResponse { session },
        })
    }

    pub(crate) async fn logs(
        &self,
        context: &crate::identity_client::RequestContext,
        request: LogsRequest,
    ) -> Result<RpcResponse> {
        let session = self.single_session(&request.selector, "logs")?;
        self.identity
            .authorize(
                &context.principal,
                Action::Logs,
                &session_resource(session.id),
            )
            .await?;
        let transcript_path = session
            .transcript_path
            .clone()
            .with_context(|| format!("no transcript available for session {}", session.id))?;
        let content = read_transcript(&transcript_path, request.max_bytes)
            .with_context(|| format!("failed to read transcript {}", transcript_path.display()))?;

        Ok(RpcResponse::Logs {
            response: LogsResponse {
                session,
                transcript_path,
                content,
            },
        })
    }

    pub(crate) async fn doctor(
        &self,
        context: &crate::identity_client::RequestContext,
        _request: DoctorRequest,
    ) -> Result<RpcResponse> {
        self.identity
            .authorize(&context.principal, Action::Doctor, &Default::default())
            .await?;
        let fresh_findings = if self.rtmd_socket_path.is_some() {
            crate::reconcile::reconcile_once(self).await?
        } else {
            Vec::new()
        };
        let sessions = self
            .store
            .lock()
            .expect("store lock poisoned")
            .list_sessions_by_selector(&Selector::All)
            .context("failed to list sessions")?;
        let mut findings = sessions
            .into_iter()
            .filter(|session| matches!(session.state, SessionState::Lost { .. }))
            .map(|session| DoctorFinding {
                severity: "error".to_string(),
                session_id: Some(session.id.to_string()),
                message: lost_session_evidence(&session, &fresh_findings),
            })
            .collect::<Vec<_>>();
        findings.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        let status = if findings.is_empty() {
            "ok"
        } else {
            "degraded"
        };

        Ok(RpcResponse::Doctor {
            response: DoctorResponse {
                status: status.to_string(),
                runtime: "in-process driver active".to_string(),
                findings,
            },
        })
    }

    pub(crate) async fn wait(&self, request: WaitRequest) -> Result<RpcResponse> {
        let deadline = Instant::now() + Duration::from_secs(request.timeout_secs);
        loop {
            let sessions = self
                .store
                .lock()
                .expect("store lock poisoned")
                .list_sessions_by_selector(&request.selector)
                .context("failed to list sessions")?;
            if wait_condition_met(&request.condition, &sessions) {
                return Ok(wait_response(true, sessions));
            }
            if Instant::now() >= deadline {
                return Ok(wait_response(false, sessions));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    fn single_session(&self, selector: &Selector, label: &str) -> Result<Session> {
        let sessions = self.resolve_selector(selector, label)?;
        anyhow::ensure!(
            sessions.len() == 1,
            "{label} selector matched {} sessions; expected exactly one",
            sessions.len()
        );
        Ok(sessions.into_iter().next().expect("one session"))
    }

    fn link_target(&self, request: &LinkRequest) -> Result<Session> {
        if let Some(id) = request.session_id {
            return self
                .store
                .lock()
                .expect("store lock poisoned")
                .get_session(&id)
                .context("failed to load link session")?
                .with_context(|| format!("unknown link session: {id}"));
        }
        if let Some(selector) = &request.selector {
            return self.single_session(selector, "link");
        }
        anyhow::bail!("link requires a session id or selector")
    }
}

fn read_transcript(path: &std::path::Path, max_bytes: Option<u64>) -> Result<String> {
    let mut bytes = fs::read(path)?;
    if let Some(max_bytes) = max_bytes
        && bytes.len() > max_bytes as usize
    {
        bytes = bytes.split_off(bytes.len() - max_bytes as usize);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn wait_condition_met(condition: &WaitCondition, sessions: &[Session]) -> bool {
    match condition {
        WaitCondition::Running => sessions
            .iter()
            .any(|session| session.state == SessionState::Running),
        WaitCondition::Terminated => sessions.iter().any(|session| {
            matches!(
                session.state,
                SessionState::Terminated | SessionState::Lost { .. }
            )
        }),
        WaitCondition::Count { count } => sessions.len() == *count,
    }
}

fn wait_response(matched: bool, sessions: Vec<Session>) -> RpcResponse {
    RpcResponse::Wait {
        response: WaitResponse { matched, sessions },
    }
}

fn lost_session_evidence(
    session: &Session,
    findings: &[crate::reconcile::ReconcileFinding],
) -> String {
    findings
        .iter()
        .find(|finding| finding.session_id == session.id.to_string())
        .map(|finding| finding.evidence.clone())
        .unwrap_or_else(|| match session.state {
            SessionState::Lost { evidence } => format!("session is LOST: {evidence}"),
            _ => format!("session is not LOST: {}", session.state),
        })
}
