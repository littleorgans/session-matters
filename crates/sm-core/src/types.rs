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
    Running,
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => f.write_str("RUNNING"),
        }
    }
}

impl FromStr for SessionState {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        match value {
            "RUNNING" => Ok(Self::Running),
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
    pub updated_at: DateTime<Utc>,
}
