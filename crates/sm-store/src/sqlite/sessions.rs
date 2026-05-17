use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::{Row, params, params_from_iter};
use sm_core::{LabelOp, RuntimeKind, Selector, Session, SessionState};
use thiserror::Error;
use uuid::Uuid;

use super::SqliteStore;
use super::time::{parse_optional_timestamp, parse_timestamp};

#[derive(Debug, Error)]
pub enum SessionRowError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Chrono(#[from] chrono::ParseError),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error(transparent)]
    Core(#[from] sm_core::SmError),
    #[error("{field} out of range: {value}")]
    IntegerOutOfRange { field: &'static str, value: i64 },
}

impl SqliteStore {
    pub fn insert_session(&self, session: &Session) -> Result<(), SessionRowError> {
        self.connection.execute(
            "INSERT INTO sessions
                (id, runtime, role, workspace, state, runtime_pid, created_at,
                 started_at, terminated_at, exit_code, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                session.id.to_string(),
                session.runtime.to_string(),
                &session.role,
                &session.workspace,
                session.state.to_string(),
                session.runtime_pid,
                session.created_at.to_rfc3339(),
                session.started_at.to_rfc3339(),
                session
                    .terminated_at
                    .map(|timestamp| timestamp.to_rfc3339()),
                session.exit_code,
                session.updated_at.to_rfc3339(),
            ],
        )?;
        self.insert_session_labels(&session.id, &session.labels)?;
        Ok(())
    }

    pub fn get_session(&self, id: &Uuid) -> Result<Option<Session>, SessionRowError> {
        let id = id.to_string();
        Ok(self
            .query_sessions("SELECT * FROM sessions WHERE id = ?1", [&id])?
            .into_iter()
            .next())
    }

    pub fn list_sessions(&self, id: Option<&str>) -> Result<Vec<Session>, SessionRowError> {
        match id {
            Some(id) => {
                let id = Uuid::parse_str(id)?;
                self.list_sessions_by_selector(&Selector::Id { id })
            }
            None => self.list_sessions_by_selector(&Selector::All),
        }
    }

    pub fn list_sessions_by_selector(
        &self,
        selector: &Selector,
    ) -> Result<Vec<Session>, SessionRowError> {
        match selector {
            Selector::All => self.query_sessions(
                "SELECT * FROM sessions ORDER BY created_at",
                std::iter::empty::<String>(),
            ),
            Selector::Id { id } => self.query_sessions(
                "SELECT * FROM sessions WHERE id = ?1 ORDER BY created_at",
                [id.to_string()],
            ),
            Selector::Role { name } => self.query_sessions(
                "SELECT * FROM sessions WHERE role = ?1 ORDER BY created_at",
                [name.clone()],
            ),
            Selector::Workspace { name } => self.query_sessions(
                "SELECT * FROM sessions WHERE workspace = ?1 ORDER BY created_at",
                [name.clone()],
            ),
            Selector::Label {
                key,
                op: LabelOp::Eq { value },
            } => self.query_sessions(
                "SELECT s.*
                 FROM sessions s
                 JOIN labels l ON l.session_id = s.id
                 WHERE l.key = ?1 AND l.value = ?2
                 ORDER BY s.created_at",
                [key.clone(), value.clone()],
            ),
            Selector::Label {
                key,
                op: LabelOp::In { values },
            } => self.query_label_in_sessions(key, values),
        }
    }

    fn query_label_in_sessions(
        &self,
        key: &str,
        values: &[String],
    ) -> Result<Vec<Session>, SessionRowError> {
        if values.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (2..values.len() + 2)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT s.*
             FROM sessions s
             JOIN labels l ON l.session_id = s.id
             WHERE l.key = ?1 AND l.value IN ({placeholders})
             ORDER BY s.created_at"
        );
        let params = std::iter::once(key.to_string())
            .chain(values.iter().cloned())
            .collect::<Vec<_>>();
        self.query_sessions(&sql, params)
    }

    fn query_sessions<P>(&self, sql: &str, params: P) -> Result<Vec<Session>, SessionRowError>
    where
        P: IntoIterator,
        P::Item: rusqlite::ToSql,
    {
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = statement.query(params_from_iter(params))?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(session_from_row(row)?);
        }
        drop(rows);
        drop(statement);
        for session in &mut sessions {
            session.labels = self.labels_for_session(&session.id)?;
        }
        Ok(sessions)
    }

    pub fn mark_session_terminated(
        &self,
        id: &Uuid,
        exit_code: Option<i32>,
        terminated_at: DateTime<Utc>,
    ) -> Result<Option<Session>, SessionRowError> {
        self.connection.execute(
            "UPDATE sessions
             SET state = ?1, exit_code = ?2, terminated_at = ?3, updated_at = ?4
             WHERE id = ?5",
            params![
                SessionState::Terminated.to_string(),
                exit_code,
                terminated_at.to_rfc3339(),
                terminated_at.to_rfc3339(),
                id.to_string(),
            ],
        )?;
        self.get_session(id)
    }
}

fn session_from_row(row: &Row<'_>) -> Result<Session, SessionRowError> {
    let runtime_pid = row.get::<_, i64>("runtime_pid")?;
    let runtime_pid =
        u32::try_from(runtime_pid).map_err(|_| integer_out_of_range("runtime_pid", runtime_pid))?;

    Ok(Session {
        id: Uuid::parse_str(&row.get::<_, String>("id")?)?,
        runtime: RuntimeKind::from_str(&row.get::<_, String>("runtime")?)?,
        role: row.get("role")?,
        workspace: row.get("workspace")?,
        state: SessionState::from_str(&row.get::<_, String>("state")?)?,
        runtime_pid,
        created_at: parse_timestamp(&row.get::<_, String>("created_at")?)?,
        started_at: parse_timestamp(&row.get::<_, String>("started_at")?)?,
        terminated_at: parse_optional_timestamp(row.get::<_, Option<String>>("terminated_at")?)?,
        exit_code: optional_i32(row, "exit_code")?,
        updated_at: parse_timestamp(&row.get::<_, String>("updated_at")?)?,
        labels: Vec::new(),
    })
}

fn optional_i32(row: &Row<'_>, column: &'static str) -> Result<Option<i32>, SessionRowError> {
    row.get::<_, Option<i64>>(column)?
        .map(|value| i32::try_from(value).map_err(|_| integer_out_of_range(column, value)))
        .transpose()
}

fn integer_out_of_range(field: &'static str, value: i64) -> SessionRowError {
    SessionRowError::IntegerOutOfRange { field, value }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use sm_core::{Label, LabelOp, Selector};

    use super::*;

    #[test]
    fn inserts_and_lists_sessions() {
        let store = SqliteStore::open_in_memory().expect("store opens");
        let now = Utc::now();
        let session = Session {
            id: Uuid::now_v7(),
            runtime: RuntimeKind::Claude,
            role: "general".to_string(),
            workspace: "test".to_string(),
            state: SessionState::Running,
            runtime_pid: 42,
            created_at: now,
            started_at: now,
            terminated_at: None,
            exit_code: None,
            updated_at: now,
            labels: Vec::new(),
        };

        store.insert_session(&session).expect("session inserts");

        assert_eq!(
            store.list_sessions(None).expect("sessions list"),
            vec![session]
        );
    }

    #[test]
    fn marks_session_terminated() {
        let store = SqliteStore::open_in_memory().expect("store opens");
        let now = Utc::now();
        let session = Session {
            id: Uuid::now_v7(),
            runtime: RuntimeKind::Claude,
            role: "general".to_string(),
            workspace: "test".to_string(),
            state: SessionState::Running,
            runtime_pid: 42,
            created_at: now,
            started_at: now,
            terminated_at: None,
            exit_code: None,
            updated_at: now,
            labels: Vec::new(),
        };
        store.insert_session(&session).expect("session inserts");

        let terminated_at = Utc::now();
        let terminated = store
            .mark_session_terminated(&session.id, Some(137), terminated_at)
            .expect("session updates")
            .expect("session exists");

        assert_eq!(terminated.state, SessionState::Terminated);
        assert_eq!(terminated.exit_code, Some(137));
        assert_eq!(terminated.terminated_at, Some(terminated_at));
    }

    #[test]
    fn selector_queries_return_sessions_with_labels() {
        let store = SqliteStore::open_in_memory().expect("store opens");
        let auth_pm = test_session(
            "pm",
            "test",
            vec![
                Label {
                    key: "area".to_string(),
                    value: "auth".to_string(),
                },
                Label {
                    key: "pri".to_string(),
                    value: "high".to_string(),
                },
            ],
        );
        let auth_engineer = test_session(
            "engineer",
            "test",
            vec![Label {
                key: "area".to_string(),
                value: "auth".to_string(),
            }],
        );
        let ui_engineer = test_session(
            "engineer",
            "test",
            vec![Label {
                key: "area".to_string(),
                value: "ui".to_string(),
            }],
        );
        for session in [&auth_pm, &auth_engineer, &ui_engineer] {
            store.insert_session(session).expect("session inserts");
        }

        let engineers = store
            .list_sessions_by_selector(&Selector::Role {
                name: "engineer".to_string(),
            })
            .expect("role selector resolves");
        assert_eq!(
            session_ids(&engineers),
            vec![auth_engineer.id, ui_engineer.id]
        );

        let auth_area = store
            .list_sessions_by_selector(&Selector::Label {
                key: "area".to_string(),
                op: LabelOp::Eq {
                    value: "auth".to_string(),
                },
            })
            .expect("label selector resolves");
        assert_eq!(session_ids(&auth_area), vec![auth_pm.id, auth_engineer.id]);
        assert_eq!(
            auth_area[0].labels,
            vec![
                Label {
                    key: "area".to_string(),
                    value: "auth".to_string(),
                },
                Label {
                    key: "pri".to_string(),
                    value: "high".to_string(),
                },
            ]
        );

        let in_area = store
            .list_sessions_by_selector(&Selector::Label {
                key: "area".to_string(),
                op: LabelOp::In {
                    values: vec!["auth".to_string(), "ui".to_string()],
                },
            })
            .expect("label in selector resolves");
        assert_eq!(
            session_ids(&in_area),
            vec![auth_pm.id, auth_engineer.id, ui_engineer.id]
        );
    }

    #[test]
    fn migrates_pass1_schema() {
        let path = std::env::temp_dir().join(format!("sm-store-{}.db", Uuid::now_v7()));
        let created_at = Utc::now().to_rfc3339();
        {
            let connection = rusqlite::Connection::open(&path).expect("v0 database opens");
            connection
                .execute_batch(
                    "CREATE TABLE sessions (
                        id TEXT PRIMARY KEY NOT NULL,
                        runtime TEXT NOT NULL,
                        role TEXT NOT NULL,
                        workspace TEXT NOT NULL,
                        state TEXT NOT NULL,
                        runtime_pid INTEGER NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    );",
                )
                .expect("v0 schema creates");
            connection
                .execute(
                    "INSERT INTO sessions
                        (id, runtime, role, workspace, state, runtime_pid, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        Uuid::now_v7().to_string(),
                        RuntimeKind::Claude.to_string(),
                        "general",
                        "test",
                        SessionState::Running.to_string(),
                        42,
                        created_at,
                        created_at,
                    ],
                )
                .expect("v0 row inserts");
        }

        let store = SqliteStore::open(&path).expect("store migrates");
        let sessions = store.list_sessions(None).expect("sessions list");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].started_at, sessions[0].created_at);
        assert_eq!(sessions[0].terminated_at, None);
        assert_eq!(sessions[0].exit_code, None);
        let _ = std::fs::remove_file(path);
    }

    fn test_session(role: &str, workspace: &str, labels: Vec<Label>) -> Session {
        let now = Utc::now();
        Session {
            id: Uuid::now_v7(),
            runtime: RuntimeKind::Claude,
            role: role.to_string(),
            workspace: workspace.to_string(),
            state: SessionState::Running,
            runtime_pid: 42,
            created_at: now,
            started_at: now,
            terminated_at: None,
            exit_code: None,
            updated_at: now,
            labels,
        }
    }

    fn session_ids(sessions: &[Session]) -> Vec<Uuid> {
        sessions.iter().map(|session| session.id).collect()
    }
}
