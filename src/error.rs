use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub(crate) enum RadishError {
    #[error("Incomplete data: {0}")]
    Incomplete(&'static str),

    #[error("Protocol error: {0}")]
    Protocol(&'static str),

    #[error("WRONGTYPE Operation against a key holding the wrong kind of value")]
    WrongType,

    #[error("Invalid command")]
    InvalidCommand,
}
