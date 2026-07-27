use super::types::RespValue;

// ── Private integer serializers ──────────────────────────────────────────

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

// ── Encode impl on RespValue ─────────────────────────────────────────────

impl RespValue<'_> {
    /// Encodes this RESP value into `buf`.
    pub(crate) fn encode_to(&self, buf: &mut Vec<u8>) {
        match self {
            Self::SimpleString(s) => {
                buf.push(b'+');
                buf.extend_from_slice(s);
                buf.extend_from_slice(b"\r\n");
            }
            Self::BulkString(s) => {
                buf.push(b'$');
                push_usize(buf, s.len());
                buf.extend_from_slice(b"\r\n");
                buf.extend_from_slice(s);
                buf.extend_from_slice(b"\r\n");
            }
            Self::Integer(i) => {
                buf.push(b':');
                push_i64(buf, *i);
                buf.extend_from_slice(b"\r\n");
            }
            Self::Error(e) => {
                buf.push(b'-');
                buf.extend_from_slice(e);
                buf.extend_from_slice(b"\r\n");
            }
            Self::Null => {
                buf.extend_from_slice(b"$-1\r\n");
            }
            Self::Array(arr) => {
                buf.push(b'*');
                push_usize(buf, arr.len());
                buf.extend_from_slice(b"\r\n");
                for item in arr {
                    item.encode_to(buf);
                }
            }
        }
    }

    // ── Convenience writers (encode directly without constructing a value) ──

    /// Encode a simple string (`+<s>\r\n`).
    pub(crate) fn write_simple_string(s: &str, buf: &mut Vec<u8>) {
        buf.push(b'+');
        buf.extend_from_slice(s.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    /// Encode a bulk string from a byte slice (`$N\r\n<s>\r\n`).
    pub(crate) fn write_bulk_string(s: &[u8], buf: &mut Vec<u8>) {
        buf.push(b'$');
        push_usize(buf, s.len());
        buf.extend_from_slice(b"\r\n");
        buf.extend_from_slice(s);
        buf.extend_from_slice(b"\r\n");
    }

    /// Encode a RESP error (`-<e>\r\n`).
    pub(crate) fn write_error(e: &str, buf: &mut Vec<u8>) {
        buf.push(b'-');
        buf.extend_from_slice(e.as_bytes());
        buf.extend_from_slice(b"\r\n");
    }

    /// Encode a null bulk string (`$-1\r\n`).
    pub(crate) fn write_null(buf: &mut Vec<u8>) {
        buf.extend_from_slice(b"$-1\r\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── push_usize / push_i64 ────────────────────────────────────────────

    #[test]
    fn push_usize_zero() {
        let mut buf = Vec::new();
        push_usize(&mut buf, 0);
        assert_eq!(&buf, b"0");
    }

    #[test]
    fn push_usize_large() {
        let mut buf = Vec::new();
        push_usize(&mut buf, 123_456);
        assert_eq!(&buf, b"123456");
    }

    #[test]
    fn push_i64_zero() {
        let mut buf = Vec::new();
        push_i64(&mut buf, 0);
        assert_eq!(&buf, b"0");
    }

    #[test]
    fn push_i64_positive() {
        let mut buf = Vec::new();
        push_i64(&mut buf, 99);
        assert_eq!(&buf, b"99");
    }

    #[test]
    fn push_i64_negative() {
        let mut buf = Vec::new();
        push_i64(&mut buf, -42);
        assert_eq!(&buf, b"-42");
    }

    #[test]
    fn push_i64_min() {
        let mut buf = Vec::new();
        push_i64(&mut buf, i64::MIN);
        assert_eq!(
            std::str::from_utf8(&buf).unwrap(),
            i64::MIN.to_string()
        );
    }

    #[test]
    fn push_i64_max() {
        let mut buf = Vec::new();
        push_i64(&mut buf, i64::MAX);
        assert_eq!(
            std::str::from_utf8(&buf).unwrap(),
            i64::MAX.to_string()
        );
    }

    // ── encode_to ────────────────────────────────────────────────────────

    #[test]
    fn encode_simple_string() {
        let mut buf = Vec::new();
        RespValue::SimpleString(b"OK").encode_to(&mut buf);
        assert_eq!(&buf, b"+OK\r\n");
    }

    #[test]
    fn encode_bulk_string() {
        let mut buf = Vec::new();
        RespValue::BulkString(b"hello").encode_to(&mut buf);
        assert_eq!(&buf, b"$5\r\nhello\r\n");
    }

    #[test]
    fn encode_empty_bulk_string() {
        let mut buf = Vec::new();
        RespValue::BulkString(b"").encode_to(&mut buf);
        assert_eq!(&buf, b"$0\r\n\r\n");
    }

    #[test]
    fn encode_integer() {
        let mut buf = Vec::new();
        RespValue::Integer(1000).encode_to(&mut buf);
        assert_eq!(&buf, b":1000\r\n");
    }

    #[test]
    fn encode_negative_integer() {
        let mut buf = Vec::new();
        RespValue::Integer(-5).encode_to(&mut buf);
        assert_eq!(&buf, b":-5\r\n");
    }

    #[test]
    fn encode_error() {
        let mut buf = Vec::new();
        RespValue::Error(b"ERR bad").encode_to(&mut buf);
        assert_eq!(&buf, b"-ERR bad\r\n");
    }

    #[test]
    fn encode_null() {
        let mut buf = Vec::new();
        RespValue::Null.encode_to(&mut buf);
        assert_eq!(&buf, b"$-1\r\n");
    }

    #[test]
    fn encode_array() {
        let mut buf = Vec::new();
        RespValue::Array(vec![
            RespValue::BulkString(b"SET"),
            RespValue::BulkString(b"k"),
            RespValue::BulkString(b"v"),
        ])
        .encode_to(&mut buf);
        assert_eq!(&buf, b"*3\r\n$3\r\nSET\r\n$1\r\nk\r\n$1\r\nv\r\n");
    }

    #[test]
    fn encode_empty_array() {
        let mut buf = Vec::new();
        RespValue::Array(vec![]).encode_to(&mut buf);
        assert_eq!(&buf, b"*0\r\n");
    }

    // ── Convenience writers ──────────────────────────────────────────────

    #[test]
    fn write_simple_string_convenience() {
        let mut buf = Vec::new();
        RespValue::write_simple_string("PONG", &mut buf);
        assert_eq!(&buf, b"+PONG\r\n");
    }

    #[test]
    fn write_bulk_string_convenience() {
        let mut buf = Vec::new();
        RespValue::write_bulk_string(b"data", &mut buf);
        assert_eq!(&buf, b"$4\r\ndata\r\n");
    }

    #[test]
    fn write_error_convenience() {
        let mut buf = Vec::new();
        RespValue::write_error("ERR oops", &mut buf);
        assert_eq!(&buf, b"-ERR oops\r\n");
    }

    #[test]
    fn write_null_convenience() {
        let mut buf = Vec::new();
        RespValue::write_null(&mut buf);
        assert_eq!(&buf, b"$-1\r\n");
    }

    // ── Round-trip ───────────────────────────────────────────────────────

    #[test]
    fn round_trip_simple_string() {
        let original = RespValue::SimpleString(b"hello");
        let mut buf = Vec::new();
        original.encode_to(&mut buf);
        let (decoded, rest) = RespValue::decode(&buf).unwrap();
        assert_eq!(decoded, original);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_bulk_string() {
        let original = RespValue::BulkString(b"binary\x00data");
        let mut buf = Vec::new();
        original.encode_to(&mut buf);
        let (decoded, rest) = RespValue::decode(&buf).unwrap();
        assert_eq!(decoded, original);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_integer() {
        let original = RespValue::Integer(-999);
        let mut buf = Vec::new();
        original.encode_to(&mut buf);
        let (decoded, rest) = RespValue::decode(&buf).unwrap();
        assert_eq!(decoded, original);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_null() {
        let original = RespValue::Null;
        let mut buf = Vec::new();
        original.encode_to(&mut buf);
        let (decoded, rest) = RespValue::decode(&buf).unwrap();
        assert_eq!(decoded, original);
        assert!(rest.is_empty());
    }

    #[test]
    fn round_trip_array() {
        let original = RespValue::Array(vec![
            RespValue::Integer(1),
            RespValue::BulkString(b"two"),
            RespValue::Null,
        ]);
        let mut buf = Vec::new();
        original.encode_to(&mut buf);
        let (decoded, rest) = RespValue::decode(&buf).unwrap();
        assert_eq!(decoded, original);
        assert!(rest.is_empty());
    }
}
