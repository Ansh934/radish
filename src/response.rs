use crate::cmd::{CommandType, RadishCommand};
use crate::resp::{Resp, RespValue};
use crate::store::SharedStore;

pub(crate) struct Response {
    pub(crate) data: Vec<u8>,
}

impl Response {
    pub(crate) fn eval(cmd: &RadishCommand, store: &SharedStore) -> Self {
        let data = match cmd.cmd_type() {
            CommandType::Ping => {
                if cmd.args().is_empty() {
                    Resp::encode_simple_string("PONG")
                } else {
                    Resp::encode_bulk_string(&cmd.args()[0])
                }
            }
            CommandType::Echo => {
                if let Some(arg) = cmd.args().get(0) {
                    Resp::encode_bulk_string(arg)
                } else {
                    Resp::encode_error("ECHO command requires an argument")
                }
            }
            CommandType::Set => {
                let args = cmd.args();
                if args.len() < 2 {
                    return Response {
                        data: Resp::encode_error("SET command requires a key and a value"),
                    };
                }

                let key = args[0].clone();
                let value = args[1].clone();
                let mut expiry_ms: Option<i64> = None;

                let mut i = 2;
                while i < args.len() {
                    let arg = args[i].to_uppercase();
                    if arg == "EX" || arg == "PX" {
                        i += 1;
                        if i >= args.len() {
                            return Response {
                                data: Resp::encode_error(
                                    "SET command with EX/PX requires an expiry time",
                                ),
                            };
                        }
                        match args[i].parse::<i64>() {
                            Ok(val) => {
                                expiry_ms = if arg == "EX" {
                                    Some(val.saturating_mul(1000))
                                } else {
                                    Some(val)
                                };
                            }
                            Err(_) => {
                                return Response {
                                    data: Resp::encode_error("Invalid expiry time for SET command"),
                                };
                            }
                        }
                    } else {
                        return Response {
                            data: Resp::encode_error(
                                "Unknown option for SET command. Only EX and PX are supported.",
                            ),
                        };
                    }
                    i += 1;
                }

                let mut store_ref = store.borrow_mut();
                store_ref.set(key, RespValue::BulkString(value), expiry_ms);
                Resp::encode_simple_string("OK")
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
            CommandType::Ttl => match cmd.args().get(0) {
                Some(key) => {
                    let store_ref = store.borrow();
                    let ttl = store_ref.ttl(key);
                    Resp::encode(&RespValue::Integer(ttl))
                }
                None => Resp::encode_error("TTL command requires a key"),
            },
            CommandType::Unknown(name) => Resp::encode_error(&format!("unknown command: {}", name)),
        };
        Response { data }
    }
}
