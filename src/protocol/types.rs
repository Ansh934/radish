/// All RESP (REdis Serialization Protocol) value variants.
///
/// Every value carries zero-copy slices into the original read buffer,
/// so lifetimes are tied to the buffer that was passed to `Resp::decode`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum RespValue<'a> {
    /// `+OK\r\n`
    SimpleString(&'a [u8]),
    /// `:1000\r\n`
    Integer(i64),
    /// `$6\r\nfoobar\r\n`
    BulkString(&'a [u8]),
    /// `*2\r\n...`
    Array(Vec<RespValue<'a>>),
    /// `-ERR msg\r\n`
    Error(&'a [u8]),
    /// `$-1\r\n`
    Null,
}
