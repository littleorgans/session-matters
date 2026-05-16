use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SmError, SmResult};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Claude,
    Codex,
}

impl RuntimeKind {
    pub fn command(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

impl fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.command())
    }
}

impl FromStr for RuntimeKind {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        match value {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            other => Err(SmError::UnsupportedRuntime(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionState {
    Spawning,
    Running,
    Terminated,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawning => f.write_str("SPAWNING"),
            Self::Running => f.write_str("RUNNING"),
            Self::Terminated => f.write_str("TERMINATED"),
        }
    }
}

impl FromStr for SessionState {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        match value {
            "SPAWNING" => Ok(Self::Spawning),
            "RUNNING" => Ok(Self::Running),
            "TERMINATED" => Ok(Self::Terminated),
            other => Err(SmError::Message(format!(
                "unsupported session state: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: Uuid,
    pub runtime: RuntimeKind,
    pub role: String,
    pub workspace: String,
    pub state: SessionState,
    pub runtime_pid: u32,
    pub created_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub updated_at: DateTime<Utc>,
}
