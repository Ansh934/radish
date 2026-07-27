use crate::command::{CommandType, RadishCommand};
use crate::protocol::{parse_number, RespValue};
use crate::storage::SharedStore;

/// Evaluates parsed commands against the store and encodes responses.
///
/// `Dispatcher` is stateless — it holds no data.  It exists as a named type
/// (rather than a free function) because it represents a coherent role:
/// "the component that maps a command + store → a RESP response".
/// Future extensions (e.g. ACL checks, middleware, metrics) would live here.
pub(crate) struct Dispatcher;

impl Dispatcher {
    /// Executes `cmd` against `store` and appends the RESP-encoded response
    /// to `buf`.  No I/O occurs here — all writes are batched by the caller.
    pub(crate) fn eval(cmd: RadishCommand, store: &SharedStore, buf: &mut Vec<u8>) {
        match cmd.cmd_type() {
            CommandType::Ping => {
                if cmd.args().is_empty() {
                    RespValue::write_simple_string("PONG", buf);
                } else {
                    RespValue::write_bulk_string(cmd.args()[0], buf);
                }
            }

            CommandType::Echo => {
                if cmd.args().is_empty() {
                    RespValue::write_error("ECHO command requires an argument", buf);
                } else {
                    RespValue::write_bulk_string(cmd.args()[0], buf);
                }
            }

            CommandType::Set => {
                let args = cmd.args();
                if args.len() < 2 {
                    RespValue::write_error("SET command requires a key and a value", buf);
                    return;
                }

                let key = args[0];
                let value = args[1];
                let mut expiry_ms: Option<i64> = None;

                let mut i = 2;
                while i < args.len() {
                    let flag = args[i];

                    if flag.eq_ignore_ascii_case(b"EX") || flag.eq_ignore_ascii_case(b"PX") {
                        let is_seconds = flag.eq_ignore_ascii_case(b"EX");
                        i += 1;

                        if i >= args.len() {
                            RespValue::write_error(
                                "SET command with EX/PX requires an expiry time",
                                buf,
                            );
                            return;
                        }

                        let val = match parse_number::<i64>(args[i]) {
                            Ok(v) => v,
                            Err(_) => {
                                RespValue::write_error(
                                    "ERR value is not an integer or out of range",
                                    buf,
                                );
                                return;
                            }
                        };

                        expiry_ms = if is_seconds {
                            Some(val.saturating_mul(1000))
                        } else {
                            Some(val)
                        };
                    } else {
                        RespValue::write_error(
                            "Unknown option for SET command. Only EX and PX are supported.",
                            buf,
                        );
                        return;
                    }
                    i += 1;
                }

                store.borrow_mut().set(key, value, expiry_ms);
                RespValue::write_simple_string("OK", buf);
            }

            CommandType::Get => match cmd.args().first() {
                Some(key) => {
                    let store_ref = store.borrow();
                    if let Some(value) = store_ref.get(key) {
                        RespValue::BulkString(value).encode_to(buf);
                    } else {
                        RespValue::write_null(buf);
                    }
                }
                None => RespValue::write_error("GET command requires a key", buf),
            },

            CommandType::Ttl => match cmd.args().first() {
                Some(key) => {
                    let store_ref = store.borrow();
                    let ttl = store_ref.ttl(key);
                    RespValue::Integer(ttl).encode_to(buf);
                }
                None => RespValue::write_error("TTL command requires a key", buf),
            },

            CommandType::Unknown(name) => {
                RespValue::write_error(
                    &format!("unknown command: {}", String::from_utf8_lossy(name)),
                    buf,
                );
            }
        }
    }
}
