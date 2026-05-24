use crate::resp::{Resp, RespValue};

#[derive(Debug, PartialEq)]
pub(crate) enum CommandType {
    Ping,
    Echo,
    Set,
    Get,
    Ttl,
    Unknown(String),
}

impl From<&str> for CommandType {
    fn from(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PING" => CommandType::Ping,
            "ECHO" => CommandType::Echo,
            "SET" => CommandType::Set,
            "GET" => CommandType::Get,
            "TTL" => CommandType::Ttl,
            _ => CommandType::Unknown(s.to_string()),
        }
    }
}

pub(crate) struct RadishCommand {
    cmd: CommandType,
    args: Vec<String>,
}

impl RadishCommand {
    pub(crate) fn from_bytes(buf: &[u8]) -> Option<Self> {
        let (resp_value, _) = Resp::decode(&buf)?;
        Self::from_resp_value(&resp_value)
    }

    fn from_resp_value(value: &RespValue) -> Option<Self> {
        match value {
            RespValue::Array(items) if !items.is_empty() => {
                let cmd_str = match &items[0] {
                    RespValue::BulkString(s) => s.clone(),
                    RespValue::SimpleString(s) => s.clone(),
                    _ => return None,
                };
                let cmd = CommandType::from(cmd_str.as_str());

                let args = items[1..]
                    .iter()
                    .filter_map(|item| match item {
                        RespValue::BulkString(s) => Some(s.clone()),
                        RespValue::SimpleString(s) => Some(s.clone()),
                        _ => None,
                    })
                    .collect();

                Some(RadishCommand { cmd, args })
            }
            _ => None,
        }
    }

    pub(crate) fn cmd_type(&self) -> &CommandType {
        &self.cmd
    }

    pub(crate) fn args(&self) -> &[String] {
        &self.args
    }
}