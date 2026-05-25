use crate::cmd::{CommandType, RadishCommand};
use crate::resp::{Resp, RespValue};
use crate::store::SharedStore;

pub(crate) struct Response {
    pub(crate) data: Vec<u8>,
}

impl Response {
    pub(crate) fn eval(cmd: &RadishCommand, store: &SharedStore) -> Self {
        let data = match cmd.cmd_type() {
            CommandType::Ping => Resp::encode_simple_string("PONG"),
            CommandType::Echo => {
                if let Some(arg) = cmd.args().get(0) {
                    Resp::encode_bulk_string(arg)
                } else {
                    Resp::encode_error("ECHO command requires an argument")
                }
            }
            CommandType::Set => {
                if let (Some(key), Some(value)) = (cmd.args().get(0), cmd.args().get(1)) {
                    let mut store_ref = store.borrow_mut();
                    store_ref.set(key.clone(), RespValue::BulkString(value.clone()), None);
                    Resp::encode_simple_string("OK")
                } else {
                    Resp::encode_error("SET command requires a key and a value")
                }
            }
            CommandType::Get => match cmd.args().get(0) {
                Some(key) => {
                    let store_ref = store.borrow();
                    if let Some(value) = store_ref.get(key) {
                        Resp::encode(value)
                    } else {
                        Resp::encode_null()
                    }
                }
                None => Resp::encode_error("GET command requires a key"),
            },
            CommandType::Ttl => Resp::encode_error("TTL not implemented yet"),
            CommandType::Unknown(name) => Resp::encode_error(&format!("unknown command: {}", name)),
        };
        Response { data }
    }
}
