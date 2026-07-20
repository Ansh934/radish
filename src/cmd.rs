use bytes::Bytes;

use crate::resp::{Resp, RespValue};

#[derive(Debug, PartialEq)]
pub(crate) enum CommandType {
    Ping,
    Echo,
    Set,
    Get,
    Ttl,
    Unknown(Bytes),
}

// const PING: Bytes = Bytes::from_static("PING".as_bytes());
// const ECHO: Bytes = Bytes::from_static("ECHO".as_bytes());
// const SET: Bytes = Bytes::from_static("SET".as_bytes());
// const GET: Bytes = Bytes::from_static("GET".as_bytes());
// const TTL: Bytes = Bytes::from_static("TTL".as_bytes());

impl From<Bytes> for CommandType {
    fn from(b: Bytes) -> Self {
        let cmd = b.as_ref();

        if cmd.eq_ignore_ascii_case(b"PING") {
            CommandType::Ping
        } else if cmd.eq_ignore_ascii_case(b"ECHO") {
            CommandType::Echo
        } else if cmd.eq_ignore_ascii_case(b"SET") {
            CommandType::Set
        } else if cmd.eq_ignore_ascii_case(b"GET") {
            CommandType::Get
        } else {
            CommandType::Unknown(b.clone())
        }
    }
}

pub(crate) struct RadishCommand {
    cmd: CommandType,
    args: Vec<Bytes>,
}

impl RadishCommand {
    pub(crate) fn from_bytes(buf: Bytes) -> Result<Self, &'static str> {
        let (resp_value, _) = Resp::decode(buf)?;
        Self::from_resp_value(resp_value)
    }

    fn from_resp_value(value: RespValue) -> Result<Self, &'static str> {
        match value {
            RespValue::Array(mut items) if !items.is_empty() => {
                let first_item = items.remove(0);
                let cmd = match first_item {
                    RespValue::BulkString(s) => s,
                    RespValue::SimpleString(s) => s,
                    _ => {
                        return Err("Invalid command");
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
            _ => Err("Invalid command"),
        }
    }

    pub(crate) fn cmd_type(&self) -> &CommandType {
        &self.cmd
    }

    pub(crate) fn args(&self) -> &[Bytes] {
        &self.args
    }
}
