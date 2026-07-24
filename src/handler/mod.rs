//! Command execution / business logic layer.
//!
//! The handler takes a parsed `RadishCommand` and a `SharedStore` reference,
//! evaluates the command, and writes a RESP-encoded response into the caller's
//! write buffer.  It has no knowledge of sockets or raw bytes.

mod dispatch;

pub(crate) use dispatch::Dispatcher;
