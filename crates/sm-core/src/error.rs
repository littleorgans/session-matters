use thiserror::Error;

pub type SmResult<T> = Result<T, SmError>;

#[derive(Debug, Error)]
pub enum SmError {
    #[error("unsupported runtime: {0}")]
    UnsupportedRuntime(String),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error("{0}")]
    Message(String),
}
