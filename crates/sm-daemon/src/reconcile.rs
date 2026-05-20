use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use sm_core::{Selector, SessionState};
use tokio::task::JoinHandle;

use crate::handler::DaemonState;

const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

pub struct ReconcileTask {
    handle: JoinHandle<()>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileFinding {
    pub session_id: String,
    pub evidence: String,
}

impl ReconcileTask {
    pub fn spawn(state: Arc<DaemonState>) -> Self {
        let handle = tokio::spawn(async move {
            loop {
                if let Err(error) = reconcile_once(&state).await {
                    eprintln!("failed to reconcile sessions: {error:#}");
                }
                tokio::time::sleep(RECONCILE_INTERVAL).await;
            }
        });

        Self { handle }
    }
}

impl Drop for ReconcileTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub async fn reconcile_once(state: &DaemonState) -> Result<Vec<ReconcileFinding>> {
    let sessions = state
        .store
        .lock()
        .expect("store lock poisoned")
        .list_sessions_by_selector(&Selector::All)
        .context("failed to list sessions for reconciliation")?;
    let mut findings = Vec::new();
    for session in sessions.into_iter().filter(|session| {
        matches!(
            session.state,
            SessionState::Running | SessionState::Spawning
        )
    }) {
        let probe = state
            .driver
            .probe_session(&session.id.to_string(), session.runtime_pid)
            .await
            .context("failed to probe runtime session")?;
        if let Some(transcript_path) = probe.transcript_path {
            state
                .store
                .lock()
                .expect("store lock poisoned")
                .record_transcript_path(&session.id, &transcript_path, Utc::now())
                .context("failed to persist transcript path")?;
        }
        if probe.verified {
            continue;
        }
        state
            .store
            .lock()
            .expect("store lock poisoned")
            .mark_session_lost(&session.id, Utc::now())
            .context("failed to mark session lost")?;
        findings.push(ReconcileFinding {
            session_id: session.id.to_string(),
            evidence: probe.evidence,
        });
    }
    Ok(findings)
}
