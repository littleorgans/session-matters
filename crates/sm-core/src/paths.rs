use std::env;
use std::path::PathBuf;

use crate::{SmError, SmResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmPaths {
    pub dir: PathBuf,
    pub socket: PathBuf,
    pub pidfile: PathBuf,
    pub database: PathBuf,
    pub log: PathBuf,
}

impl SmPaths {
    pub fn from_env() -> SmResult<Self> {
        let home = env::var_os("SM_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".sm")))
            .ok_or(SmError::MissingHome)?;
        Ok(Self::new(home))
    }

    pub fn new(dir: PathBuf) -> Self {
        Self {
            socket: dir.join("sock"),
            pidfile: dir.join("sm.pid"),
            database: dir.join("sm.db"),
            log: dir.join("smd.log"),
            dir,
        }
    }
}
