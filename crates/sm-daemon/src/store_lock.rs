use std::sync::MutexGuard;

use anyhow::{Result, anyhow};
use sm_store::SqliteStore;

use crate::handler::DaemonState;

impl DaemonState {
    pub(crate) fn store(&self) -> Result<MutexGuard<'_, SqliteStore>> {
        self.store
            .lock()
            .map_err(|_| anyhow!("store lock poisoned"))
    }
}
