use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{SmError, SmResult};

pub use lilo_rm_core::LostEvidence;

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
    Lost { evidence: LostEvidence },
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawning => f.write_str("SPAWNING"),
            Self::Running => f.write_str("RUNNING"),
            Self::Terminated => f.write_str("TERMINATED"),
            Self::Lost { evidence } => write!(f, "Lost({evidence})"),
        }
    }
}

impl SessionState {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Spawning | Self::Running)
    }

    pub fn sql_name(&self) -> &'static str {
        match self {
            Self::Spawning => "SPAWNING",
            Self::Running => "RUNNING",
            Self::Terminated => "TERMINATED",
            Self::Lost { .. } => "LOST",
        }
    }

    pub fn from_sql(value: &str, lost_evidence: Option<LostEvidence>) -> SmResult<Self> {
        match value {
            "SPAWNING" => Ok(Self::Spawning),
            "RUNNING" => Ok(Self::Running),
            "TERMINATED" => Ok(Self::Terminated),
            "LOST" => Ok(Self::Lost {
                evidence: lost_evidence.ok_or_else(|| {
                    SmError::Message("lost session missing lost_evidence".to_string())
                })?,
            }),
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
    #[serde(default)]
    pub labels: Vec<Label>,
    pub state: SessionState,
    pub runtime_pid: u32,
    pub runtime_session: Option<String>,
    pub transcript_path: Option<PathBuf>,
    pub tmux_pane: Option<String>,
    pub agent_config: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Label {
    pub key: String,
    pub value: String,
}

impl FromStr for Label {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        let (key, value) = value
            .split_once('=')
            .ok_or_else(|| SmError::Message(format!("invalid label mutation: {value}")))?;
        Ok(Self {
            key: parse_label_token(key, "label key")?,
            value: parse_label_token(value, "label value")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LabelMutation {
    Set(Label),
    Remove { key: String },
}

impl FromStr for LabelMutation {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        if let Some(key) = value.strip_suffix('-') {
            return Ok(Self::Remove {
                key: parse_label_token(key, "label key")?,
            });
        }
        Ok(Self::Set(Label::from_str(value)?))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[derive(Default)]
pub enum Selector {
    Id {
        id: Uuid,
    },
    Label {
        key: String,
        op: LabelOp,
    },
    Workspace {
        name: String,
    },
    Role {
        name: String,
    },
    #[default]
    All,
}

impl fmt::Display for Selector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::Id { id } => write!(f, "id:{id}"),
            Self::Role { name } => write!(f, "role:{name}"),
            Self::Workspace { name } => write!(f, "workspace:{name}"),
            Self::Label {
                key,
                op: LabelOp::Eq { value },
            } => write!(f, "label:{key}={value}"),
            Self::Label {
                key,
                op: LabelOp::In { values },
            } => write!(f, "label:{key} in ({})", values.join(", ")),
        }
    }
}

impl FromStr for Selector {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        let value = value.trim();
        if value == "all" {
            return Ok(Self::All);
        }
        if let Ok(id) = Uuid::parse_str(value) {
            return Ok(Self::Id { id });
        }
        if let Some(raw) = value.strip_prefix("id:") {
            return Ok(Self::Id {
                id: Uuid::parse_str(raw.trim())?,
            });
        }
        if let Some(raw) = value.strip_prefix("role:") {
            return Ok(Self::Role {
                name: parse_label_token(raw, "role selector")?,
            });
        }
        if let Some(raw) = value.strip_prefix("workspace:") {
            return Ok(Self::Workspace {
                name: parse_label_token(raw, "workspace selector")?,
            });
        }
        if let Some(raw) = value.strip_prefix("label:") {
            return parse_label_selector(raw);
        }
        Err(SmError::Message(format!(
            "unsupported selector: {value} (expected one of: {SELECTOR_GRAMMAR_HINT})"
        )))
    }
}

pub const SELECTOR_GRAMMAR_HINT: &str = "all, <uuid>, id:<uuid>, role:<name>, workspace:<name>, label:<key>=<value>, label:<key> in (v1, v2)";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LabelOp {
    Eq { value: String },
    In { values: Vec<String> },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailStatus {
    Unread,
    Read,
}

impl fmt::Display for MailStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unread => f.write_str("unread"),
            Self::Read => f.write_str("read"),
        }
    }
}

impl FromStr for MailStatus {
    type Err = SmError;

    fn from_str(value: &str) -> SmResult<Self> {
        match value {
            "unread" => Ok(Self::Unread),
            "read" => Ok(Self::Read),
            other => Err(SmError::Message(format!(
                "unsupported mail status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mail {
    pub id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Uuid,
    pub content: String,
    pub sent_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

impl Mail {
    pub fn status(&self) -> MailStatus {
        if self.read_at.is_some() {
            MailStatus::Read
        } else {
            MailStatus::Unread
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Mail,
    Nudge,
}

fn parse_label_selector(value: &str) -> SmResult<Selector> {
    if let Some((key, raw_value)) = value.split_once('=') {
        return Ok(Selector::Label {
            key: parse_label_token(key, "label key")?,
            op: LabelOp::Eq {
                value: parse_label_token(raw_value, "label value")?,
            },
        });
    }
    let (key, raw_values) = value
        .split_once(" in ")
        .ok_or_else(|| SmError::Message(format!("invalid label selector: {value}")))?;
    let values = parse_label_values(raw_values)?;
    Ok(Selector::Label {
        key: parse_label_token(key, "label key")?,
        op: LabelOp::In { values },
    })
}

fn parse_label_values(value: &str) -> SmResult<Vec<String>> {
    let value = value.trim();
    let Some(value) = value
        .strip_prefix('(')
        .and_then(|value| value.strip_suffix(')'))
    else {
        return Err(SmError::Message(format!(
            "invalid label value list: {value}"
        )));
    };
    let values = value
        .split(',')
        .map(|item| parse_label_token(item, "label value"))
        .collect::<SmResult<Vec<_>>>()?;
    if values.is_empty() {
        return Err(SmError::Message("label value list is empty".to_string()));
    }
    Ok(values)
}

fn parse_label_token(value: &str, field: &'static str) -> SmResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(SmError::Message(format!("{field} is empty")));
    }
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn selector_parser_covers_closed_grammar() {
        let id = Uuid::now_v7();

        assert_eq!(Selector::from_str("all").unwrap(), Selector::All);
        assert_eq!(
            Selector::from_str(&format!("id:{id}")).unwrap(),
            Selector::Id { id }
        );
        assert_eq!(
            Selector::from_str(&id.to_string()).unwrap(),
            Selector::Id { id }
        );
        assert_eq!(
            Selector::from_str("role:engineer").unwrap(),
            Selector::Role {
                name: "engineer".to_string()
            }
        );
        assert_eq!(
            Selector::from_str("workspace:test").unwrap(),
            Selector::Workspace {
                name: "test".to_string()
            }
        );
        assert_eq!(
            Selector::from_str("label:area=auth").unwrap(),
            Selector::Label {
                key: "area".to_string(),
                op: LabelOp::Eq {
                    value: "auth".to_string()
                }
            }
        );
        assert_eq!(
            Selector::from_str("label:area in (auth, ui)").unwrap(),
            Selector::Label {
                key: "area".to_string(),
                op: LabelOp::In {
                    values: vec!["auth".to_string(), "ui".to_string()]
                }
            }
        );
    }

    #[test]
    fn selector_display_round_trips_through_from_str() {
        let id = Uuid::now_v7();
        let cases = vec![
            Selector::All,
            Selector::Id { id },
            Selector::Role {
                name: "engineer".to_string(),
            },
            Selector::Workspace {
                name: "test".to_string(),
            },
            Selector::Label {
                key: "area".to_string(),
                op: LabelOp::Eq {
                    value: "auth".to_string(),
                },
            },
            Selector::Label {
                key: "area".to_string(),
                op: LabelOp::In {
                    values: vec!["auth".to_string(), "ui".to_string()],
                },
            },
        ];
        for selector in cases {
            let rendered = selector.to_string();
            let parsed = Selector::from_str(&rendered).unwrap();
            assert_eq!(parsed, selector, "round-trip failed for {rendered}");
        }

        assert_eq!(Selector::All.to_string(), "all");
        assert_eq!(
            Selector::Role {
                name: "engineer".to_string(),
            }
            .to_string(),
            "role:engineer"
        );
        assert_eq!(
            Selector::Label {
                key: "area".to_string(),
                op: LabelOp::Eq {
                    value: "auth".to_string(),
                },
            }
            .to_string(),
            "label:area=auth"
        );
        assert_eq!(
            Selector::Label {
                key: "area".to_string(),
                op: LabelOp::In {
                    values: vec!["auth".to_string(), "ui".to_string()],
                },
            }
            .to_string(),
            "label:area in (auth, ui)"
        );
    }

    #[test]
    fn label_mutation_parser_distinguishes_set_and_remove() {
        assert_eq!(
            LabelMutation::from_str("pri=urgent").unwrap(),
            LabelMutation::Set(Label {
                key: "pri".to_string(),
                value: "urgent".to_string()
            })
        );
        assert_eq!(
            LabelMutation::from_str("pri-").unwrap(),
            LabelMutation::Remove {
                key: "pri".to_string()
            }
        );
    }
}
