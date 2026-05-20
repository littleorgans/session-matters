use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use lilo_rm_client::{EventWatcher, RuntimeClient};
use lilo_rm_core::{EventBatch, StatusFilter};
use tokio::task::JoinHandle;

use crate::handler::DaemonState;

const EVENT_WAIT_MS: u32 = 30_000;
const BACKOFF_INITIAL: Duration = Duration::from_millis(200);
const BACKOFF_MAX: Duration = Duration::from_secs(5);

pub struct RuntimeEventTask {
    handle: JoinHandle<()>,
}

impl RuntimeEventTask {
    pub fn spawn(state: Arc<DaemonState>, socket_path: PathBuf) -> Self {
        let handle = tokio::spawn(async move {
            if let Err(error) = run_event_loop(state, socket_path).await {
                eprintln!("runtime event loop stopped: {error:#}");
            }
        });

        Self { handle }
    }
}

impl Drop for RuntimeEventTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn run_event_loop(state: Arc<DaemonState>, socket_path: PathBuf) -> Result<()> {
    let mut cursor = state
        .store
        .lock()
        .expect("store lock poisoned")
        .event_cursor()
        .context("failed to load runtime event cursor")?;
    let mut backoff = BACKOFF_INITIAL;

    loop {
        let client = RuntimeClient::new(socket_path.clone());
        let status_client = client.clone();
        let mut builder = EventWatcher::builder().wait_ms(EVENT_WAIT_MS);
        if let Some(cursor) = cursor {
            builder = builder.since(cursor);
        }
        let mut watcher = match builder.connect(client).await {
            Ok(watcher) => watcher,
            Err(error) => {
                eprintln!("failed to connect runtime event watcher: {error:#}");
                tokio::time::sleep(backoff).await;
                backoff = next_backoff(backoff);
                continue;
            }
        };
        backoff = BACKOFF_INITIAL;

        loop {
            match watcher.next().await {
                Ok(EventBatch::Events {
                    events,
                    cursor: next,
                }) => {
                    state
                        .store
                        .lock()
                        .expect("store lock poisoned")
                        .apply_runtime_events_and_cursor(&events, next)
                        .context("failed to persist runtime events")?;
                    cursor = Some(next);
                    backoff = BACKOFF_INITIAL;
                }
                Ok(EventBatch::CursorExpired { oldest }) => {
                    let payload = status_client
                        .status(StatusFilter::empty())
                        .await
                        .context("failed to reconcile expired runtime cursor")?;
                    crate::reconcile::reconcile_lifecycles(&state, &payload.lifecycles)?;
                    state
                        .store
                        .lock()
                        .expect("store lock poisoned")
                        .apply_cursor(oldest)
                        .context("failed to persist expired runtime cursor")?;
                    cursor = Some(oldest);
                    backoff = BACKOFF_INITIAL;
                }
                Ok(batch) => {
                    eprintln!("unsupported runtime event batch: {batch:?}");
                    tokio::time::sleep(backoff).await;
                    backoff = next_backoff(backoff);
                    break;
                }
                Err(error) => {
                    eprintln!("runtime event watcher failed: {error:#}");
                    tokio::time::sleep(backoff).await;
                    backoff = next_backoff(backoff);
                    break;
                }
            }
        }
    }
}

fn next_backoff(current: Duration) -> Duration {
    std::cmp::min(current.saturating_mul(2), BACKOFF_MAX)
}
