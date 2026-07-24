//! RESP (REdis Serialization Protocol) wire-format layer.
//!
//! This module owns all encode/decode logic and the `RespValue` type.
//! No business logic lives here — it is purely a protocol codec.

mod codec;
mod types;

pub(crate) use codec::Resp;
pub(crate) use types::RespValue;
