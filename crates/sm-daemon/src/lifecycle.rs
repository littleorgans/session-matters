use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use uuid::Uuid;

use crate::handler::DaemonState;

pub struct LifecycleTask {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl LifecycleTask {
    pub fn spawn(state: Arc<DaemonState>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            while !stop_thread.load(Ordering::SeqCst) {
                if let Err(error) = refresh_exits(&state) {
                    eprintln!("failed to refresh session lifecycle: {error:#}");
                }
                thread::sleep(Duration::from_millis(200));
            }
        });

        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for LifecycleTask {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn refresh_exits(state: &DaemonState) -> Result<()> {
    for child_exit in state
        .driver
        .reap_exited()
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
