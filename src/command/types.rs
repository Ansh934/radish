/// Every command variant recognised by the server.
///
/// `Unknown` borrows the raw command bytes from the parse buffer so we can
/// surface a helpful error message without allocating.
#[derive(Debug, PartialEq)]
pub(crate) enum CommandType<'a> {
    Ping,
    Echo,
    Set,
    Get,
    Ttl,
    Del,
    Expire,
    /// An unrecognised command name (zero-copy borrow of the raw bytes).
    Unknown(&'a [u8]),
}

impl<'a> From<&'a [u8]> for CommandType<'a> {
    fn from(cmd: &'a [u8]) -> Self {
        if cmd.eq_ignore_ascii_case(b"PING") {
            CommandType::Ping
        } else if cmd.eq_ignore_ascii_case(b"ECHO") {
            CommandType::Echo
        } else if cmd.eq_ignore_ascii_case(b"SET") {
            CommandType::Set
        } else if cmd.eq_ignore_ascii_case(b"GET") {
            CommandType::Get
        } else if cmd.eq_ignore_ascii_case(b"TTL") {
            CommandType::Ttl
        } else if cmd.eq_ignore_ascii_case(b"DEL") {
            CommandType::Del
        } else if cmd.eq_ignore_ascii_case(b"EXPIRE") {
            CommandType::Expire
        } else {
            CommandType::Unknown(cmd)
        }
    }
}
