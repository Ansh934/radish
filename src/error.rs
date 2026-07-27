use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub(crate) enum RadishError {
    #[error("Incomplete data: {0}")]
    Incomplete(&'static str),

    #[error("Protocol error: {0}")]
    Protocol(&'static str),

    #[error("Invalid command")]
    InvalidCommand,
}
