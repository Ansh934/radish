
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RespValue {
    SimpleString(String),  // +OK\r\n
    Integer(i64),          // :1000\r\n
    BulkString(String),    // $6\r\nfoobar\r\n
    Array(Vec<RespValue>), // *2\r\n...
    Error(String),         // -ERR msg\r\n
    Null,                  // $-1\r\n
}

pub(crate) struct Resp;

impl Resp {
    /// Reads a line terminated by \r\n
    /// Returns (line_without_crlf, remaining_buf)
    fn read_line(buf: &[u8]) -> Option<(&[u8], &[u8])> {
        if buf.len() < 2 {
            return None;
        }
        let pos = buf.windows(2).position(|w| w == b"\r\n")?;
        Some((&buf[..pos], &buf[pos + 2..]))
    }

    pub(crate) fn decode(buf: &[u8]) -> Option<(RespValue, &[u8])> {
        let first = *buf.first()?;
        match first {
            // Array
            b'*' => {
                let (line, mut remaining) = Self::read_line(&buf[1..])?;

                let count: usize = String::from_utf8_lossy(line).parse().ok()?;

                let mut values = Vec::with_capacity(count);

                for _ in 0..count {
                    let (value, rest) = Self::decode(remaining)?;
                    values.push(value);
                    remaining = rest;
                }

                Some((RespValue::Array(values), remaining))
            }

            // Simple String
            b'+' => {
                let (line, remaining) = Self::read_line(&buf[1..])?;

                Some((
                    RespValue::SimpleString(String::from_utf8_lossy(line).to_string()),
                    remaining,
                ))
            }

            // Integer
            b':' => {
                let (line, remaining) = Self::read_line(&buf[1..])?;

                let value = String::from_utf8_lossy(line).parse().ok()?;

                Some((RespValue::Integer(value), remaining))
            }

            // Error
            b'-' => {
                let (line, remaining) = Self::read_line(&buf[1..])?;

                Some((
                    RespValue::Error(String::from_utf8_lossy(line).to_string()),
                    remaining,
                ))
            }

            // Bulk String
            b'$' => {
                let (line, remaining_after_len) = Self::read_line(&buf[1..])?;

                let len: isize = String::from_utf8_lossy(line).parse().ok()?;

                // Null bulk string
                if len == -1 {
                    return Some((RespValue::Null, remaining_after_len));
                }

                if len < 0 {
                    return None;
                }

                let len = len as usize;

                // Need:
                // data bytes + trailing \r\n
                if remaining_after_len.len() < len + 2 {
                    return None;
                }

                let data = &remaining_after_len[..len];

                // Validate trailing \r\n
                if &remaining_after_len[len..len + 2] != b"\r\n" {
                    return None;
                }

                let remaining = &remaining_after_len[len + 2..];

                Some((
                    RespValue::BulkString(String::from_utf8_lossy(data).to_string()),
                    remaining,
                ))
            }

            _ => None,
        }
    }

    pub(crate) fn encode(value: &RespValue) -> Vec<u8> {
        match value {
            RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
            RespValue::BulkString(s) => format!("${}\r\n{}\r\n", s.len(), s).into_bytes(),
            RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
            RespValue::Error(e) => format!("-{}\r\n", e).into_bytes(),
            RespValue::Null => b"$-1\r\n".to_vec(),
            RespValue::Array(arr) => {
                let mut out = format!("*{}\r\n", arr.len()).into_bytes();

                for value in arr {
                    out.extend(Self::encode(value));
                }

                out
            }
        }
    }

    pub(crate) fn encode_simple_string(s: &str) -> Vec<u8> {
        Self::encode(&RespValue::SimpleString(s.to_string()))
    }
    pub(crate) fn encode_bulk_string(s: &str) -> Vec<u8> {
        Self::encode(&RespValue::BulkString(s.to_string()))
    }
    pub(crate) fn encode_error(e: &str) -> Vec<u8> {
        Self::encode(&RespValue::Error(e.to_string()))
    }
    pub(crate) fn encode_null() -> Vec<u8> {
        Self::encode(&RespValue::Null)
    }
}

// todo implement asref pattern for allowing both &str and String to be used as keys in store