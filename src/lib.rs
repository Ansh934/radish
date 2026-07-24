//! # Radish
//!
//! A Redis-compatible in-memory key-value server.
//!
//! ## Module layout
//!
//! | Module       | Responsibility                                      |
//! |--------------|-----------------------------------------------------|
//! | `error`      | Crate-wide error type                               |
//! | `protocol`   | RESP wire-format codec (encode / decode)            |
//! | `command`    | Command parsing (`RadishCommand`, `CommandType`)    |
//! | `storage`    | In-memory key-value store                           |
//! | `handler`    | Command evaluation / business logic                 |
//! | `server`     | TCP listener and per-connection I/O loop            |

pub(crate) mod error;
pub(crate) mod protocol;
pub(crate) mod command;
pub(crate) mod storage;
pub(crate) mod handler;
mod server;

pub use server::Server;
