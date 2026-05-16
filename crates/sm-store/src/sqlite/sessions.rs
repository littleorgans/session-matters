use std::str::FromStr;

use chrono::{DateTime, Utc};
use rusqlite::{Row, params, params_from_iter};
use sm_core::{RuntimeKind, Session, SessionState};
use thiserror::Error;
use uuid::Uuid;

use super::SqliteStore;

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
            Some(id) => self.query_sessions("SELECT * FROM sessions WHERE id = ?1", [id]),
            None => self.query_sessions("SELECT * FROM sessions ORDER BY created_at", []),
        }
    }

    fn query_sessions<const N: usize>(
        &self,
        sql: &str,
        params: [&str; N],
    ) -> Result<Vec<Session>, SessionRowError> {
        let mut statement = self.connection.prepare(sql)?;
        let mut rows = statement.query(params_from_iter(params))?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(session_from_row(row)?);
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
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("created_at")?)
            .map(DateTime::<Utc>::from)?,
        started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("started_at")?)
            .map(DateTime::<Utc>::from)?,
        terminated_at: optional_timestamp(row, "terminated_at")?,
        exit_code: optional_i32(row, "exit_code")?,
        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("updated_at")?)
            .map(DateTime::<Utc>::from)?,
    })
}

fn optional_timestamp(
    row: &Row<'_>,
    column: &'static str,
) -> Result<Option<DateTime<Utc>>, SessionRowError> {
    row.get::<_, Option<String>>(column)?
        .map(|timestamp| DateTime::parse_from_rfc3339(&timestamp).map(DateTime::<Utc>::from))
        .transpose()
        .map_err(SessionRowError::from)
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
}
