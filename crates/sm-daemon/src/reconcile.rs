use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use sm_core::{Selector, SessionState};

use crate::handler::DaemonState;

const RECONCILE_INTERVAL: Duration = Duration::from_secs(30);

pub struct ReconcileTask {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileFinding {
    pub session_id: String,
    pub evidence: String,
}

impl ReconcileTask {
    pub fn spawn(state: Arc<DaemonState>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !stop_thread.load(Ordering::SeqCst) {
                if let Err(error) = reconcile_once(&state) {
                    eprintln!("failed to reconcile sessions: {error:#}");
                }
                sleep_interval(&stop_thread);
            }
        });

        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for ReconcileTask {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn reconcile_once(state: &DaemonState) -> Result<Vec<ReconcileFinding>> {
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
            .context("failed to probe runtime session")?;
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

fn sleep_interval(stop: &AtomicBool) {
    let mut slept = Duration::ZERO;
    while slept < RECONCILE_INTERVAL && !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(250));
        slept += Duration::from_millis(250);
    }
}
