use crate::resp::{Resp, RespValue};

pub(crate) struct RadishCommand {
    cmd: String,
    args: Vec<String>,
}

impl RadishCommand {
    pub(crate) fn new(cmd: String, args: Vec<String>) -> Self {
        RadishCommand { cmd, args }
    }

    pub(crate) fn from_bytes(buf: &[u8]) -> Option<Self> {
        let (resp_value, _) = Resp::decode(&buf)?;
        Self::from_resp_value(&resp_value)
    }

    fn from_resp_value(value: &RespValue) -> Option<Self> {
        match value {
            RespValue::Array(items) if !items.is_empty() => {
                let cmd = match &items[0] {
                    RespValue::BulkString(s) => s.clone(),
                    RespValue::SimpleString(s) => s.clone(),
                    _ => return None,
                }.to_uppercase();

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

    pub(crate) fn eval(&self) -> Vec<u8> {
        match self.cmd.to_uppercase().as_str() {
            "PING" => Resp::encode_string("PONG"),
            "ECHO" => {
                if let Some(arg) = self.args.get(0) {
                    Resp::encode_string(arg)
                } else {
                    Resp::encode_error("ECHO command requires an argument")
                }
            }
            _ => Resp::encode_error(&format!("unknown command: {}", self.cmd)),
        }
    }
}