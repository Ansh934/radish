use crate::cmd::{CommandType, RadishCommand};
use crate::resp::{Resp, RespValue};
use crate::store::SharedStore;
use bytes::Bytes;

pub(crate) struct Response {
    pub(crate) data: Bytes,
}

impl Response {
    pub(crate) fn eval(cmd: RadishCommand, store: &SharedStore) -> Self {
        let data = match cmd.cmd_type() {
            CommandType::Ping => {
                if cmd.args().is_empty() {
                    Resp::encode_simple_string("PONG")
                } else {
                    Resp::encode_bulk_string_from_bytes(cmd.args()[0].clone())
                }
            }
            CommandType::Echo => {
                if cmd.args().is_empty() {
                    Resp::encode_error("ECHO command requires an argument")
                } else {
                    Resp::encode_bulk_string_from_bytes(cmd.args()[0].clone())
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
                    let arg_slice = args[i].as_ref(); 
                    
                    if arg_slice.eq_ignore_ascii_case(b"EX") || arg_slice.eq_ignore_ascii_case(b"PX") {
                        let is_ex = arg_slice.eq_ignore_ascii_case(b"EX");
                        i += 1;
                        
                        if i >= args.len() {
                            return Response {
                                data: Resp::encode_error("SET command with EX/PX requires an expiry time"),
                            };
                        }

                        let val = match Resp::parse_number::<i64>(&args[i]) {
                            Ok(v) => v,
                            Err(_) => {
                                return Response {
                                    data: Resp::encode_error("ERR value is not an integer or out of range"),
                                };
                            }
                        };

                        expiry_ms = if is_ex {
                            Some(val.saturating_mul(1000))
                        } else {
                            Some(val)
                        };
                    } else {
                        return Response {
                            data: Resp::encode_error("Unknown option for SET command. Only EX and PX are supported."),
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
            CommandType::Unknown(name) => {
                Resp::encode_error(&format!("unknown command: {}", String::from_utf8_lossy(name)))
            }
        };
        
        Response { data }
    }
}