//! RESP (REdis Serialization Protocol) wire-format layer.
//!
//! This module owns all encode/decode logic and the `RespValue` type.
//! No business logic lives here — it is purely a protocol codec.
//!
//! - Decoding and encoding are methods on [`RespValue`].
//! - [`parse_number`] is a general-purpose utility re-exported for callers
//!   that need to parse ASCII integers outside of RESP framing.

mod decode;
mod encode;
mod types;

pub(crate) use decode::parse_number;
pub(crate) use types::RespValue;
