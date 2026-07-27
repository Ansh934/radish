use crate::error::RadishError;
use crate::protocol::RespValue;
use super::types::CommandType;

/// A fully-parsed client command, holding zero-copy references into the
/// underlying read buffer.
pub(crate) struct RadishCommand<'a> {
    cmd: CommandType<'a>,
    args: Vec<&'a [u8]>,
}

impl<'a> RadishCommand<'a> {
    /// Attempts to parse one complete RESP command from `buf`.
    ///
    /// - `Ok(Some((cmd, bytes_consumed)))` — a full command was decoded.
    /// - `Ok(None)` — the buffer is incomplete; wait for more data.
    /// - `Err(e)` — the bytes are protocol-invalid; drop the connection.
    pub(crate) fn try_parse(buf: &'a [u8]) -> Result<Option<(Self, usize)>, RadishError> {
        if buf.is_empty() {
            return Ok(None);
        }

        match RespValue::decode(buf) {
            Ok((resp_value, remaining)) => {
                let consumed = buf.len() - remaining.len();
                let cmd = Self::from_resp_value(resp_value)?;
                Ok(Some((cmd, consumed)))
            }
            Err(RadishError::Incomplete(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Converts a decoded `RespValue` into a `RadishCommand`.
    ///
    /// The RESP array's first element is treated as the command name; the rest
    /// are arguments.
    pub(crate) fn from_resp_value(value: RespValue<'a>) -> Result<Self, RadishError> {
        match value {
            RespValue::Array(mut items) if !items.is_empty() => {
                let first_item = items.remove(0);
                let cmd_bytes = match first_item {
                    RespValue::BulkString(s) | RespValue::SimpleString(s) => s,
                    _ => return Err(RadishError::InvalidCommand),
                };

                let cmd = CommandType::from(cmd_bytes);
                let args = items
                    .into_iter()
                    .filter_map(|item| match item {
                        RespValue::BulkString(s) | RespValue::SimpleString(s) => Some(s),
                        _ => None,
                    })
                    .collect();

                Ok(RadishCommand { cmd, args })
            }
            _ => Err(RadishError::InvalidCommand),
        }
    }

    /// Returns the parsed command variant.
    pub(crate) fn cmd_type(&self) -> &CommandType<'a> {
        &self.cmd
    }

    /// Returns the argument slices (zero-copy borrows of the read buffer).
    pub(crate) fn args(&self) -> &[&'a [u8]] {
        &self.args
    }
}
