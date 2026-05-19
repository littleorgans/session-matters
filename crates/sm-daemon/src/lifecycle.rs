use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::handler::DaemonState;

pub struct LifecycleTask {
    handle: JoinHandle<()>,
}

impl LifecycleTask {
    pub fn spawn(state: Arc<DaemonState>) -> Self {
        let handle = tokio::spawn(async move {
            loop {
                if let Err(error) = refresh_exits(&state).await {
                    eprintln!("failed to refresh session lifecycle: {error:#}");
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        });

        Self { handle }
    }
}

impl Drop for LifecycleTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

pub async fn refresh_exits(state: &DaemonState) -> Result<()> {
    for child_exit in state
        .driver
        .reap_exited()
        .await
        .context("failed to reap children")?
    {
        let id = Uuid::parse_str(&child_exit.session_id).context("invalid session id")?;
        state
            .store
            .lock()
            .expect("store lock poisoned")
            .mark_session_terminated(&id, child_exit.exit_code, Utc::now())
            .context("failed to persist terminated session")?;
    }
    Ok(())
}
