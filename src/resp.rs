use bytes::{BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RespValue {
    SimpleString(Bytes),   // +OK\r\n
    Integer(i64),          // :1000\r\n
    BulkString(Bytes),     // $6\r\nfoobar\r\n
    Array(Vec<RespValue>), // *2\r\n...
    Error(Bytes),          // -ERR msg\r\n
    Null,                  // $-1\r\n
}

pub(crate) struct Resp;

impl Resp {
    /// Reads a line terminated by \r\n
    /// Returns (line_without_crlf, remaining_buf)
    fn read_line(buf: Bytes) -> Result<(Bytes, Bytes), &'static str> {
        if buf.len() < 2 {
            return Err("Invalid buffer length");
        }
        let pos = buf
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or("Line end not found")?;

        Ok((buf.slice(..pos), buf.slice(pos + 2..)))
    }

    /// Helper to safely parse ASCII numbers (like array lengths or integers) from Bytes
    pub(crate) fn parse_number<T: std::str::FromStr>(bytes: &Bytes) -> Result<T, &'static str> {
        std::str::from_utf8(bytes)
            .map_err(|_| "Protocol error: expected ASCII number")?
            .parse::<T>()
            .map_err(|_| "Protocol error: invalid number format")
    }

    pub(crate) fn decode(buf: Bytes) -> Result<(RespValue, Bytes), &'static str> {
        let first = buf.first().ok_or("decode called on empty buffer")?;
        let buf = buf.slice(1..);

        match first {
            // Array
            b'*' => {
                let (line, mut remaining) = Self::read_line(buf)?;
                let count = Self::parse_number::<usize>(&line)?;

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
                Ok((RespValue::SimpleString(line), remaining))
            }

            // Integer
            b':' => {
                let (line, remaining) = Self::read_line(buf)?;
                let int_val = Self::parse_number::<i64>(&line)?;
                Ok((RespValue::Integer(int_val), remaining))
            }

            // Error
            b'-' => {
                let (line, remaining) = Self::read_line(buf)?;
                Ok((RespValue::Error(line), remaining))
            }

            // Bulk String
            b'$' => {
                let (line, remaining_after_len) = Self::read_line(buf)?;
                let len = Self::parse_number::<isize>(&line)?;

                // Null bulk string
                if len == -1 {
                    return Ok((RespValue::Null, remaining_after_len));
                }

                if len < 0 {
                    return Err("Invalid bulk string length");
                }

                let len = len as usize;

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

    pub(crate) fn encode(value: &RespValue) -> Bytes {
        let mut buf = BytesMut::with_capacity(4096);
        Self::encode_into(value, &mut buf);
        buf.freeze()
    }

    fn encode_into(value: &RespValue, buf: &mut BytesMut) {
        match value {
            RespValue::SimpleString(s) => {
                buf.put_u8(b'+');
                buf.put_slice(s);
                buf.put_slice(b"\r\n");
            }
            RespValue::BulkString(s) => {
                buf.put_u8(b'$');
                buf.put_slice(s.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");
                buf.put_slice(s);
                buf.put_slice(b"\r\n");
            }
            RespValue::Integer(i) => {
                buf.put_u8(b':');
                buf.put_slice(i.to_string().as_bytes());
                buf.put_slice(b"\r\n");
            }
            RespValue::Error(e) => {
                buf.put_u8(b'-');
                buf.put_slice(e);
                buf.put_slice(b"\r\n");
            }
            RespValue::Null => {
                buf.put_slice(b"$-1\r\n");
            }
            RespValue::Array(arr) => {
                buf.put_u8(b'*');
                buf.put_slice(arr.len().to_string().as_bytes());
                buf.put_slice(b"\r\n");

                for item in arr {
                    Self::encode_into(item, buf);
                }
            }
        }
    }

    pub(crate) fn encode_simple_string(s: &str) -> Bytes {
        let mut buf = BytesMut::with_capacity(s.len() + 3);
        buf.put_u8(b'+');
        buf.put_slice(s.as_bytes());
        buf.put_slice(b"\r\n");
        buf.freeze()
    }

    pub(crate) fn encode_bulk_string_from_bytes(s: Bytes) -> Bytes {
        let mut buf = BytesMut::with_capacity(s.len() + 32);
        buf.put_u8(b'$');
        buf.put_slice(s.len().to_string().as_bytes());
        buf.put_slice(b"\r\n");
        buf.put_slice(&s);
        buf.put_slice(b"\r\n");
        buf.freeze()
    }

    pub(crate) fn encode_error(e: &str) -> Bytes {
        let mut buf = BytesMut::with_capacity(e.len() + 3);
        buf.put_u8(b'-');
        buf.put_slice(e.as_bytes());
        buf.put_slice(b"\r\n");
        buf.freeze()
    }

    pub(crate) fn encode_null() -> Bytes {
        Bytes::from_static(b"$-1\r\n")
    }
}
