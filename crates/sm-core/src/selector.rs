use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::label::parse_label_token;
use crate::{SmError, SmResult};

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
}
