use bytes::{Buf, Bytes};
use std::str;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RespValue {
    SimpleString(Bytes),   // +OK\r\n
    Integer(i64),        // :1000\r\n
    BulkString(Bytes),     // $6\r\nfoobar\r\n
    Array(Vec<RespValue>), // *2\r\n...
    Error(Bytes),          // -ERR msg\r\n
    Null,                  // $-1\r\n
}

pub(crate) struct Resp;

impl Resp {
    fn check_utf8(bytes: &Bytes) -> Result<(), &'static str> {
        str::from_utf8(bytes).map_err(|_| "Invalid UTF-8 string provided")?;
        Ok(())
    }

    fn check_integer(bytes: &Bytes) -> Result<(), &'static str> {
        str::from_utf8(bytes)
            .map_err(|_| "Invalid UTF-8 string provided")?
            .parse::<i64>()
            .map_err(|_| "Invalid integer provided")?;
        Ok(())
    }

    /// Reads a line terminated by \r\n
    /// Returns (line_without_crlf, remaining_buf)
    /// Errors if the buffer is too short or if the line is not found
    fn read_line(buf: Bytes) -> Result<(Bytes, Bytes), &'static str> {
        if buf.len() < 2 {
            return Err("Invalid buffer length");
        }
        let pos = buf
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| "Line end not found")?;
        Ok((buf.slice(..pos), buf.slice(pos + 2..)))
    }

    pub(crate) fn decode(buf: Bytes) -> Result<(RespValue, Bytes), &'static str> {
        let first = buf.first().ok_or_else(|| "decode called on empty buffer")?;
        let buf = buf.slice(1..);
        match first {
            // Array
            b'*' => {
                let (line, mut remaining) = Self::read_line(buf)?;

                let count: usize = std::str::from_utf8(&line)
                    .expect("Invalid utf8 string provided")
                    .parse()
                    .map_err(|_| "Invalid array count provided")?;

                let mut values = Vec::with_capacity(count);

                for _ in 0..count {
                    let (value, rest) = Self::decode(remaining)?;
                    values.push(value);
                    remaining = rest;
                }

                Ok((RespValue::Array(values), remaining))
            }

            // Simple String
            b'+' => {
                let (line, remaining) = Self::read_line(buf)?;
                // Check if the line is valid UTF-8
                str::from_utf8(&line).expect("Invalid utf8 string provided");
                Ok((RespValue::SimpleString(line), remaining))
            }

            // Integer
            b':' => {
                let (line, remaining) = Self::read_line(buf)?;
                // Check if the line is valid UTF-8
                str::from_utf8(&line)
                    .expect("Invalid utf8 string provided")
                    .parse::<i64>()
                    .expect("Invalid integer provided");
                Ok((RespValue::Integer(line), remaining))
            }

            // Error
            b'-' => {
                let (line, remaining) = Self::read_line(buf)?;
                // Check if the line is valid UTF-8
                str::from_utf8(&line).expect("Invalid utf8 string provided");
                Ok((RespValue::Error(line), remaining))
            }

            // Bulk String
            b'$' => {
                let (line, remaining_after_len) = Self::read_line(buf)?;

                let len: isize = std::str::from_utf8(&line)
                    .expect("Invalid utf8 string provided")
                    .parse()
                    .map_err(|_| "Invalid bulk string length")?;

                // Null bulk string
                if len == -1 {
                    return Ok((RespValue::Null, remaining_after_len));
                }

                if len < 0 {
                    return Err("Invalid bulk string length");
                }

                let len = len as usize;

                // Need:
                // data bytes + trailing \r\n
                if remaining_after_len.len() < len + 2 {
                    return Err("Insufficient buffer length for bulk string");
                }

                let data = remaining_after_len.slice(..len);

                // Validate trailing \r\n
                if &remaining_after_len[len..len + 2] != b"\r\n" {
                    return Err("Invalid bulk string format");
                }

                let remaining = remaining_after_len.slice(len + 2..);

                Ok((RespValue::BulkString(data), remaining))
            }

            _ => Err("Invalid RESP value"),
        }
    }

    pub(crate) fn encode(value: &RespValue) -> Vec<u8> {
        match value {
            RespValue::SimpleString(s) => {
                format!("+{}\r\n", unsafe { String::from_utf8_unchecked(s.into()) }).into_bytes()
            }
            RespValue::BulkString(s) => format!("${}\r\n{}\r\n", s.len(), unsafe {
                String::from_utf8_unchecked(s.into())
            })
            .into_bytes(),
            RespValue::Integer(mut i) => format!(":{}\r\n", i.get_i64()).into_bytes(),
            RespValue::Error(e) => {
                format!("-{}\r\n", unsafe { String::from_utf8_unchecked(e.into()) }).into_bytes()
            }
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
        Self::encode(RespValue::SimpleString(Bytes::from(s.to_string())))
    }
    pub(crate) fn encode_bulk_string(s: &str) -> Vec<u8> {
        Self::encode(RespValue::BulkString(Bytes::from(s.to_string())))
    }
    pub(crate) fn encode_bulk_string_from_bytes(s: Bytes) -> Vec<u8> {
        Self::encode(RespValue::BulkString(s.clone()))
    }
    pub(crate) fn encode_error(e: &str) -> Vec<u8> {
        Self::encode(RespValue::Error(Bytes::from(e.to_string())))
    }
    pub(crate) fn encode_null() -> Vec<u8> {
        Self::encode(RespValue::Null)
    }
}

// todo implement asref pattern for allowing both &str and String to be used as keys in store
