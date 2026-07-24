//! Command parsing layer.
//!
//! Converts raw RESP-decoded values into typed `RadishCommand` structs.
//! This layer has no knowledge of the store or the network; it only
//! translates bytes → commands.

mod parser;
mod types;

pub(crate) use parser::RadishCommand;
pub(crate) use types::CommandType;
