pub(crate) enum RespValue {
    SimpleString(String),  // +OK\r\n
    Integer(i64),          // :1000\r\n
    BulkString(String),    // $6\r\nfoobar\r\n
    Array(Vec<RespValue>), // *2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n
    Error(String),         // -Error message\r\n
    Null,                  // $-1\r\n
}

pub(crate) struct Resp {}

impl Resp {
    fn read_line(buf: &[u8]) -> &[u8] {
        // read until \r\n
        buf.split(|&b| b == b'\r' || b == b'\n')
            .next()
            .unwrap_or(&[])
    }
    pub(crate) fn decode(buf: &[u8]) -> Option<(RespValue, &[u8])> {
        match buf.first() {
            Some(&b) => match b {
                b'*' => {
                    let mut values = Vec::new();
                    let line = Self::read_line(&buf[1..]);
                    let len = 1 + line.len() + 2;
                    let mut remaining = if buf.len() >= len { &buf[len..] } else { &[] };
                    for _ in 0..String::from_utf8_lossy(line).parse().unwrap_or(0) {
                        if let Some((value, rest)) = Self::decode(remaining) {
                            values.push(value);
                            remaining = rest;
                        } else {
                            break;
                        }
                    }
                    Some((RespValue::Array(values), remaining))
                }
                b'+' | b':' | b'$' | b'-' => {
                    let line = Self::read_line(&buf[1..]);
                    let val = match b {
                        b'+' => RespValue::SimpleString(String::from_utf8_lossy(line).to_string()),
                        b':' => {
                            RespValue::Integer(String::from_utf8_lossy(line).parse().unwrap_or(0))
                        }
                        b'$' => RespValue::BulkString(String::from_utf8_lossy(line).to_string()),
                        b'-' => RespValue::Error(String::from_utf8_lossy(line).to_string()),
                        _ => unreachable!(),
                    };
                    let len = 1 + line.len() + 2;
                    let remaining = if buf.len() >= len { &buf[len..] } else { &[] };
                    Some((val, remaining))
                }
                _ => Some((RespValue::Null, &buf[1..])),
            },
            None => None,
        }
    }

    pub(crate) fn encode(value: &RespValue) -> Vec<u8> {
        match value {
            RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
            RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
            RespValue::BulkString(s) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),
            RespValue::Array(arr) => {
                let mut res = format!("*{}\r\n", arr.len()).into_bytes();
                for v in arr {
                    res.extend(Self::encode(v));
                }
                res
            }
            RespValue::Error(e) => format!("-{}\r\n", e).into_bytes(),
            RespValue::Null => b"$-1\r\n".to_vec(),
        }
    }
}
