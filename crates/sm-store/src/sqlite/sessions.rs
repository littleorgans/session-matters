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
    #[error("runtime pid out of range: {0}")]
    PidOutOfRange(i64),
}

impl SqliteStore {
    pub fn insert_session(&self, session: &Session) -> Result<(), SessionRowError> {
        self.connection.execute(
            "INSERT INTO sessions
                (id, runtime, role, workspace, state, runtime_pid, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                session.id.to_string(),
                session.runtime.to_string(),
                &session.role,
                &session.workspace,
                session.state.to_string(),
                session.runtime_pid,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
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
}

fn session_from_row(row: &Row<'_>) -> Result<Session, SessionRowError> {
    let runtime_pid = row.get::<_, i64>("runtime_pid")?;
    let runtime_pid =
        u32::try_from(runtime_pid).map_err(|_| SessionRowError::PidOutOfRange(runtime_pid))?;

    Ok(Session {
        id: Uuid::parse_str(&row.get::<_, String>("id")?)?,
        runtime: RuntimeKind::from_str(&row.get::<_, String>("runtime")?)?,
        role: row.get("role")?,
        workspace: row.get("workspace")?,
        state: SessionState::from_str(&row.get::<_, String>("state")?)?,
        runtime_pid,
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("created_at")?)
            .map(DateTime::<Utc>::from)?,
        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>("updated_at")?)
            .map(DateTime::<Utc>::from)?,
    })
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
            updated_at: now,
        };

        store.insert_session(&session).expect("session inserts");

        assert_eq!(
            store.list_sessions(None).expect("sessions list"),
            vec![session]
        );
    }
}
