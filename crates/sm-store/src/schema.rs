pub const SESSIONS_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY NOT NULL,
    runtime TEXT NOT NULL,
    role TEXT NOT NULL,
    workspace TEXT NOT NULL,
    state TEXT NOT NULL,
    runtime_pid INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    started_at TEXT NOT NULL,
    terminated_at TEXT,
    exit_code INTEGER,
    updated_at TEXT NOT NULL
);
"#;
