mod sessions;

use std::path::Path;

use rusqlite::Connection;

use crate::schema::SESSIONS_SCHEMA;

pub use sessions::SessionRowError;

pub struct SqliteStore {
    connection: Connection,
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let connection = Connection::open(path)?;
        connection.execute_batch(SESSIONS_SCHEMA)?;
        Ok(Self { connection })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(SESSIONS_SCHEMA)?;
        Ok(Self { connection })
    }
}
