use crate::resp::{Resp, RespValue};
use crate::error::RadishError;

#[derive(Debug, PartialEq)]
pub(crate) enum CommandType<'a> {
    Ping,
    Echo,
    Set,
    Get,
    Ttl,
    Unknown(&'a [u8]),
}

impl<'a> From<&'a [u8]> for CommandType<'a> {
    fn from(cmd: &'a [u8]) -> Self {
        if cmd.eq_ignore_ascii_case(b"PING") {
            CommandType::Ping
        } else if cmd.eq_ignore_ascii_case(b"ECHO") {
            CommandType::Echo
        } else if cmd.eq_ignore_ascii_case(b"SET") {
            CommandType::Set
        } else if cmd.eq_ignore_ascii_case(b"GET") {
            CommandType::Get
        } else if cmd.eq_ignore_ascii_case(b"TTL") {
            CommandType::Ttl
        } else {
            CommandType::Unknown(cmd)
        }
    }
}

pub(crate) struct RadishCommand<'a> {
    cmd: CommandType<'a>,
    args: Vec<&'a [u8]>,
}

impl<'a> RadishCommand<'a> {
    /// Attempts to parse a command from a continuous buffer slice.
    /// Returns Ok(Some) if a complete command was parsed,
    /// Ok(None) if more data is needed,
    /// Err if the protocol is invalid.
    pub(crate) fn try_parse(buf: &'a [u8]) -> Result<Option<(Self, usize)>, RadishError> {
        if buf.is_empty() {
            return Ok(None);
        }

        match Resp::decode(buf) {
            Ok((resp_value, remaining)) => {
                // Success! Calculate how many bytes were part of this command
                let consumed = buf.len() - remaining.len();

                let cmd = Self::from_resp_value(resp_value)?;
                Ok(Some((cmd, consumed)))
            }
            Err(RadishError::Incomplete(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub(crate) fn from_resp_value(value: RespValue<'a>) -> Result<Self, RadishError> {
        match value {
            RespValue::Array(mut items) if !items.is_empty() => {
                let first_item = items.remove(0);
                let cmd = match first_item {
                    RespValue::BulkString(s) => s,
                    RespValue::SimpleString(s) => s,
                    _ => {
                        return Err(RadishError::InvalidCommand);
                    }
                };
                let cmd = CommandType::from(cmd);

                let args = items
                    .into_iter()
                    .filter_map(|item| match item {
                        RespValue::BulkString(s) => Some(s),
                        RespValue::SimpleString(s) => Some(s),
                        _ => None,
                    })
                    .collect();

                Ok(RadishCommand { cmd, args })
            }
            _ => Err(RadishError::InvalidCommand),
        }
    }

    pub(crate) fn cmd_type(&self) -> &CommandType<'a> {
        &self.cmd
    }

    pub(crate) fn args(&self) -> &[&'a [u8]] {
        &self.args
    }
}
