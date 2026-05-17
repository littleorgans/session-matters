mod labels;
mod mail;
mod sessions;
mod time;

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, Result};

use crate::schema::SESSIONS_SCHEMA;

pub use mail::MailRowError;
pub use sessions::SessionRowError;

pub struct SqliteStore {
    connection: Connection,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        Self::from_connection(connection)
    }

    fn from_connection(connection: Connection) -> Result<Self> {
        connection.execute_batch(SESSIONS_SCHEMA)?;
        migrate_sessions(&connection)?;
        Ok(Self { connection })
    }
}

fn migrate_sessions(connection: &Connection) -> Result<()> {
    let columns = session_columns(connection)?;
    if !columns.contains("started_at") {
        connection.execute("ALTER TABLE sessions ADD COLUMN started_at TEXT", [])?;
        connection.execute(
            "UPDATE sessions SET started_at = created_at WHERE started_at IS NULL",
            [],
        )?;
    }
    if !columns.contains("terminated_at") {
        connection.execute("ALTER TABLE sessions ADD COLUMN terminated_at TEXT", [])?;
    }
    if !columns.contains("exit_code") {
        connection.execute("ALTER TABLE sessions ADD COLUMN exit_code INTEGER", [])?;
    }
    if !columns.contains("runtime_session") {
        connection.execute("ALTER TABLE sessions ADD COLUMN runtime_session TEXT", [])?;
    }
    if !columns.contains("transcript_path") {
        connection.execute("ALTER TABLE sessions ADD COLUMN transcript_path TEXT", [])?;
    }
    if !columns.contains("agent_config") {
        connection.execute("ALTER TABLE sessions ADD COLUMN agent_config TEXT", [])?;
    }
    Ok(())
}

fn session_columns(connection: &Connection) -> Result<HashSet<String>> {
    let mut statement = connection.prepare("PRAGMA table_info(sessions)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>("name"))?;
    rows.collect()
}
