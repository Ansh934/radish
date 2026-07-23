use crate::cmd::{CommandType, RadishCommand};
use crate::resp::{Resp, RespValue};
use crate::store::SharedStore;

pub(crate) struct Response;

impl Response {
    pub(crate) fn eval(cmd: RadishCommand, store: &SharedStore, buf: &mut Vec<u8>) {
        match cmd.cmd_type() {
            CommandType::Ping => {
                if cmd.args().is_empty() {
                    Resp::encode_simple_string("PONG", buf);
                } else {
                    Resp::encode_bulk_string_from_slice(cmd.args()[0], buf);
                }
            }
            CommandType::Echo => {
                if cmd.args().is_empty() {
                    Resp::encode_error("ECHO command requires an argument", buf);
                } else {
                    Resp::encode_bulk_string_from_slice(cmd.args()[0], buf);
                }
            }
            CommandType::Set => {
                let args = cmd.args();
                if args.len() < 2 {
                    Resp::encode_error("SET command requires a key and a value", buf);
                    return;
                }

                let key = args[0];
                let value = args[1];
                let mut expiry_ms: Option<i64> = None;

                let mut i = 2;
                while i < args.len() {
                    let arg_slice = args[i];
                    
                    if arg_slice.eq_ignore_ascii_case(b"EX") || arg_slice.eq_ignore_ascii_case(b"PX") {
                        let is_ex = arg_slice.eq_ignore_ascii_case(b"EX");
                        i += 1;
                        
                        if i >= args.len() {
                            Resp::encode_error("SET command with EX/PX requires an expiry time", buf);
                            return;
                        }

                        let val = match Resp::parse_number::<i64>(args[i]) {
                            Ok(v) => v,
                            Err(_) => {
                                Resp::encode_error("ERR value is not an integer or out of range", buf);
                                return;
                            }
                        };

                        expiry_ms = if is_ex {
                            Some(val.saturating_mul(1000))
                        } else {
                            Some(val)
                        };
                    } else {
                        Resp::encode_error("Unknown option for SET command. Only EX and PX are supported.", buf);
                        return;
                    }
                    i += 1;
                }

                let mut store_ref = store.borrow_mut();
                store_ref.set(key, value, expiry_ms);
                Resp::encode_simple_string("OK", buf);
            }
            CommandType::Get => match cmd.args().get(0) {
                Some(key) => {
                    let store_ref = store.borrow();
                    if let Some(value) = store_ref.get(key) {
                        Resp::encode(&RespValue::BulkString(value), buf);
                    } else {
                        Resp::encode_null(buf);
                    }
                }
                None => Resp::encode_error("GET command requires a key", buf),
            },
            CommandType::Ttl => match cmd.args().get(0) {
                Some(key) => {
                    let store_ref = store.borrow();
                    let ttl = store_ref.ttl(key);
                    Resp::encode(&RespValue::Integer(ttl), buf);
                }
                None => Resp::encode_error("TTL command requires a key", buf),
            },
            CommandType::Unknown(name) => {
                Resp::encode_error(&format!("unknown command: {}", String::from_utf8_lossy(name)), buf);
            }
        }
    }
}