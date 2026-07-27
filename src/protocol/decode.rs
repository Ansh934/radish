use super::types::RespValue;
use crate::error::RadishError;

// ── Private helpers ──────────────────────────────────────────────────────

/// Reads a CRLF-terminated line from `buf`.
///
/// Returns `(line_without_crlf, remaining_buf)`.
fn read_line(buf: &[u8]) -> Result<(&[u8], &[u8]), RadishError> {
    if buf.len() < 2 {
        return Err(RadishError::Incomplete(
            "buffer too short for a CRLF line",
        ));
    }
    let pos = buf
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or(RadishError::Incomplete("CRLF line end not found"))?;

    Ok((&buf[..pos], &buf[pos + 2..]))
}

// ── Free utility ─────────────────────────────────────────────────────────

/// Parses an ASCII decimal number from a byte slice.
///
/// This is a general-purpose utility — not RESP-specific — so it lives as
/// a free function rather than a method on `RespValue`.
pub(crate) fn parse_number<T: std::str::FromStr>(bytes: &[u8]) -> Result<T, RadishError> {
    std::str::from_utf8(bytes)
        .map_err(|_| RadishError::Protocol("expected ASCII number"))?
        .parse::<T>()
        .map_err(|_| RadishError::Protocol("invalid number format"))
}

// ── Decode impl on RespValue ─────────────────────────────────────────────

impl<'a> RespValue<'a> {
    /// Decodes one RESP value from `buf`.
    ///
    /// Returns `(value, remaining_buf)` on success.
    /// Returns `Err(RadishError::Incomplete)` if more bytes are needed.
    pub(crate) fn decode(buf: &'a [u8]) -> Result<(Self, &'a [u8]), RadishError> {
        let first = buf
            .first()
            .ok_or(RadishError::Incomplete("decode called on empty buffer"))?;
        let buf = &buf[1..];

        match first {
            b'*' => {
                let (line, mut remaining) = read_line(buf)?;
                let count = parse_number::<usize>(line)?;
                let mut values = Vec::with_capacity(count);

                for _ in 0..count {
                    let (value, rest) = Self::decode(remaining)?;
                    values.push(value);
                    remaining = rest;
                }

                Ok((RespValue::Array(values), remaining))
            }
            b'+' => {
                let (line, remaining) = read_line(buf)?;
                Ok((RespValue::SimpleString(line), remaining))
            }
            b':' => {
                let (line, remaining) = read_line(buf)?;
                let int_val = parse_number::<i64>(line)?;
                Ok((RespValue::Integer(int_val), remaining))
            }
            b'-' => {
                let (line, remaining) = read_line(buf)?;
                Ok((RespValue::Error(line), remaining))
            }
            b'$' => {
                let (line, remaining_after_len) = read_line(buf)?;
                let len = parse_number::<isize>(line)?;

                if len == -1 {
                    return Ok((RespValue::Null, remaining_after_len));
                }
                if len < 0 {
                    return Err(RadishError::Protocol("invalid bulk string length"));
                }

                let len = len as usize;

                if remaining_after_len.len() < len + 2 {
                    return Err(RadishError::Incomplete(
                        "insufficient data for bulk string",
                    ));
                }

                let data = &remaining_after_len[..len];

                if &remaining_after_len[len..len + 2] != b"\r\n" {
                    return Err(RadishError::Protocol("invalid bulk string terminator"));
                }

                Ok((RespValue::BulkString(data), &remaining_after_len[len + 2..]))
            }
            _ => Err(RadishError::Protocol("unrecognised RESP type byte")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_simple_string() {
        let input = b"+OK\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::SimpleString(b"OK"));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_error() {
        let input = b"-ERR unknown\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::Error(b"ERR unknown"));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_integer() {
        let input = b":42\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::Integer(42));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_negative_integer() {
        let input = b":-17\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::Integer(-17));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_bulk_string() {
        let input = b"$6\r\nfoobar\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::BulkString(b"foobar"));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_empty_bulk_string() {
        let input = b"$0\r\n\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::BulkString(b""));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_null() {
        let input = b"$-1\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::Null);
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_array() {
        let input = b"*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(
            val,
            RespValue::Array(vec![
                RespValue::BulkString(b"GET"),
                RespValue::BulkString(b"key"),
            ])
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_empty_array() {
        let input = b"*0\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::Array(vec![]));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_nested_array() {
        let input = b"*1\r\n*2\r\n:1\r\n:2\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(
            val,
            RespValue::Array(vec![RespValue::Array(vec![
                RespValue::Integer(1),
                RespValue::Integer(2),
            ])])
        );
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_returns_remaining_bytes() {
        let input = b"+OK\r\n+NEXT\r\n";
        let (val, rest) = RespValue::decode(input).unwrap();
        assert_eq!(val, RespValue::SimpleString(b"OK"));
        assert_eq!(rest, b"+NEXT\r\n");
    }

    #[test]
    fn decode_incomplete_returns_error() {
        let input = b"+OK";
        let result = RespValue::decode(input);
        assert!(matches!(result, Err(RadishError::Incomplete(_))));
    }

    #[test]
    fn decode_empty_buffer_returns_incomplete() {
        let result = RespValue::decode(b"");
        assert!(matches!(result, Err(RadishError::Incomplete(_))));
    }

    #[test]
    fn decode_invalid_type_byte() {
        let input = b"?invalid\r\n";
        let result = RespValue::decode(input);
        assert!(matches!(result, Err(RadishError::Protocol(_))));
    }

    #[test]
    fn decode_bulk_string_with_embedded_crlf() {
        // A bulk string whose payload contains \r\n — length-based parsing
        // must not be tricked by the embedded line ending.
        let payload = b"$5\r\nab\r\nc\r\n";
        let (val, rest) = RespValue::decode(payload).unwrap();
        assert_eq!(val, RespValue::BulkString(b"ab\r\nc"));
        assert!(rest.is_empty());
    }

    #[test]
    fn decode_invalid_bulk_string_length() {
        let input = b"$-2\r\n";
        let result = RespValue::decode(input);
        assert!(matches!(result, Err(RadishError::Protocol(_))));
    }

    #[test]
    fn parse_number_valid() {
        assert_eq!(parse_number::<i64>(b"42").unwrap(), 42);
        assert_eq!(parse_number::<i64>(b"-7").unwrap(), -7);
        assert_eq!(parse_number::<usize>(b"0").unwrap(), 0);
    }

    #[test]
    fn parse_number_invalid() {
        assert!(parse_number::<i64>(b"abc").is_err());
        assert!(parse_number::<usize>(b"-1").is_err());
    }
}
