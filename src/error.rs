use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub(crate) enum RadishError {
    #[error("Incomplete data: {0}")]
    Incomplete(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid command")]
    InvalidCommand,
}
