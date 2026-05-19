use thiserror::Error;

pub type SmResult<T> = Result<T, SmError>;

#[derive(Debug, Error)]
pub enum SmError {
    #[error("unsupported runtime: {0}")]
    UnsupportedRuntime(String),
    #[error("home directory is not available")]
    MissingHome,
    #[error(transparent)]
    Paths(#[from] sm_paths::SmPathsError),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error("{0}")]
    Message(String),
}
