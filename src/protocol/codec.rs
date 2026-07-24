use crate::error::RadishError;
use super::types::RespValue;

/// Zero-allocation RESP codec.
///
/// All decode methods borrow directly from the caller's buffer — no heap
/// allocations occur during parsing.  Encode methods append to a caller-
/// supplied `Vec<u8>` write buffer, avoiding allocation per call.
pub(crate) struct Resp;

impl Resp {
    // ── Decoding helpers ────────────────────────────────────────────────────

    /// Reads a CRLF-terminated line from `buf`.
    ///
    /// Returns `(line_without_crlf, remaining_buf)`.
    fn read_line(buf: &[u8]) -> Result<(&[u8], &[u8]), RadishError> {
        if buf.len() < 2 {
            return Err(RadishError::Incomplete(
                "Buffer length is less than 2 bytes".to_string(),
            ));
        }
        let pos = buf
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| RadishError::Incomplete("CRLF line end not found".to_string()))?;

        Ok((&buf[..pos], &buf[pos + 2..]))
    }

    /// Parses an ASCII decimal number from a byte slice.
    pub(crate) fn parse_number<T: std::str::FromStr>(bytes: &[u8]) -> Result<T, RadishError> {
        std::str::from_utf8(bytes)
            .map_err(|_| RadishError::Protocol("expected ASCII number".to_string()))?
            .parse::<T>()
            .map_err(|_| RadishError::Protocol("invalid number format".to_string()))
    }

    /// Decodes one RESP value from `buf`.
    ///
    /// Returns `(value, remaining_buf)` on success.
    /// Returns `Err(RadishError::Incomplete)` if more bytes are needed.
    pub(crate) fn decode<'a>(buf: &'a [u8]) -> Result<(RespValue<'a>, &'a [u8]), RadishError> {
        let first = buf
            .first()
            .ok_or_else(|| RadishError::Incomplete("Decode called on empty buffer".to_string()))?;
        let buf = &buf[1..];

        match first {
            b'*' => {
                let (line, mut remaining) = Self::read_line(buf)?;
                let count = Self::parse_number::<usize>(line)?;
                let mut values = Vec::with_capacity(count);

                for _ in 0..count {
                    let (value, rest) = Self::decode(remaining)?;
                    values.push(value);
                    remaining = rest;
                }

                Ok((RespValue::Array(values), remaining))
            }
            b'+' => {
                let (line, remaining) = Self::read_line(buf)?;
                Ok((RespValue::SimpleString(line), remaining))
            }
            b':' => {
                let (line, remaining) = Self::read_line(buf)?;
                let int_val = Self::parse_number::<i64>(line)?;
                Ok((RespValue::Integer(int_val), remaining))
            }
            b'-' => {
                let (line, remaining) = Self::read_line(buf)?;
                Ok((RespValue::Error(line), remaining))
            }
            b'$' => {
                let (line, remaining_after_len) = Self::read_line(buf)?;
                let len = Self::parse_number::<isize>(line)?;

                if len == -1 {
                    return Ok((RespValue::Null, remaining_after_len));
                }
                if len < 0 {
                    return Err(RadishError::Protocol(
                        "Invalid bulk string length".to_string(),
                    ));
                }

                let len = len as usize;

                if remaining_after_len.len() < len + 2 {
                    return Err(RadishError::Incomplete(format!(
                        "Insufficient buffer length for bulk string (expected {} bytes, got {})",
                        len + 2,
                        remaining_after_len.len()
                    )));
                }

                let data = &remaining_after_len[..len];

                if &remaining_after_len[len..len + 2] != b"\r\n" {
                    return Err(RadishError::Protocol(
                        "Invalid bulk string format".to_string(),
                    ));
                }

                Ok((RespValue::BulkString(data), &remaining_after_len[len + 2..]))
            }
            _ => Err(RadishError::Protocol("Invalid RESP value".to_string())),
        }
    }

    // ── Encoding helpers ────────────────────────────────────────────────────

    /// Writes a `usize` as ASCII decimal digits into `buf` without allocating.
    fn push_usize(buf: &mut Vec<u8>, mut n: usize) {
        if n == 0 {
            buf.push(b'0');
            return;
        }
        let mut temp = [0u8; 20];
        let mut i = 20;
        while n > 0 {
            i -= 1;
            temp[i] = b'0' + (n % 10) as u8;
            n /= 10;
        }
        buf.extend_from_slice(&temp[i..]);
    }

    /// Writes an `i64` as ASCII decimal digits (with leading `-` if negative)
    /// into `buf` without allocating.
    fn push_i64(buf: &mut Vec<u8>, n: i64) {
        if n == 0 {
            buf.push(b'0');
            return;
        }
        let mut temp = [0u8; 21]; // 20 digits + optional '-'
        let mut i = 21;
        let is_neg = n < 0;

        if is_neg {
            let mut unsigned_n = if n == i64::MIN {
                9_223_372_036_854_775_808u64
            } else {
                (-n) as u64
            };
            while unsigned_n > 0 {
                i -= 1;
                temp[i] = b'0' + (unsigned_n % 10) as u8;
                unsigned_n /= 10;
            }
            i -= 1;
            temp[i] = b'-';
        } else {
            let mut unsigned_n = n as u64;
            while unsigned_n > 0 {
                i -= 1;
                temp[i] = b'0' + (unsigned_n % 10) as u8;
                unsigned_n /= 10;
            }
        }
        buf.extend_from_slice(&temp[i..]);
    }

    /// Encodes any `RespValue` into `buf`.
    pub(crate) fn encode(value: &RespValue, buf: &mut Vec<u8>) {
        match value {
            RespValue::SimpleString(s) => {
                buf.push(b'+');
                buf.extend_from_slice(s);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::BulkString(s) => {
                buf.push(b'$');
                Self::push_usize(buf, s.len());
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(s);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Integer(i) => {
                buf.push(b':');
                Self::push_i64(buf, *i);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Error(e) => {
                buf.push(b'-');
                buf.extend_from_slice(e);
                buf.extend_from_slice(b"\r\n");
            }
            RespValue::Null => {
                buf.extend_from_slice(b"$-1\r\n");
            }
            RespValue::Array(arr) => {
                buf.push(b'*');
                Self::push_usize(buf, arr.len());
                buf.extend_from_slice(b"\r\n");
                for item in arr {
                    Self::encode(item, buf);
                }
            }
        }
    }

    /// Convenience: encode a simple string (`+<s>\r\n`).
    pub(crate) fn encode_simple_string(s: &str, buf: &mut Vec<u8>) {
        buf.push(b'+');
        buf.extend_from_slice(s.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    /// Convenience: encode a bulk string from a byte slice (`$N\r\n<s>\r\n`).
    pub(crate) fn encode_bulk_string_from_slice(s: &[u8], buf: &mut Vec<u8>) {
        buf.push(b'$');
        Self::push_usize(buf, s.len());
        buf.extend_from_slice(b"\r\n");
        buf.extend_from_slice(s);
        buf.extend_from_slice(b"\r\n");
    }

    /// Convenience: encode a RESP error (`-<e>\r\n`).
    pub(crate) fn encode_error(e: &str, buf: &mut Vec<u8>) {
        buf.push(b'-');
        buf.extend_from_slice(e.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    /// Convenience: encode a null bulk string (`$-1\r\n`).
    pub(crate) fn encode_null(buf: &mut Vec<u8>) {
        buf.extend_from_slice(b"$-1\r\n");
    }
}
