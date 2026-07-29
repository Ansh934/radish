use chrono::{DateTime, Utc};
use std::collections::VecDeque;

use crate::error::RadishError;
use crate::protocol::RespValue;

/// The kind of data a key holds.
///
/// Redis is a multi-model store: the same key-space can hold strings, lists,
/// sets, etc.  Adding a new model means adding a variant here and teaching
/// [`StoreValue::dump_aof_command`] how to serialise it.
#[derive(Debug)]
pub(crate) enum DataType {
    String(Vec<u8>),
    #[allow(dead_code)]
    List(VecDeque<Vec<u8>>),
}

/// A single stored value with an optional expiry timestamp.
///
/// This is the leaf node inside the `HashMap` of the store.  It owns both the
/// user data and the per-key metadata (currently just `expiry`).
#[derive(Debug)]
pub(crate) struct StoreValue {
    data: DataType,
    expiry: Option<DateTime<Utc>>,
}

impl StoreValue {
    // ── Constructors ─────────────────────────────────────────────────────

    /// Creates a new string value with an optional expiry.
    pub(crate) fn new_string(value: Vec<u8>, expiry: Option<DateTime<Utc>>) -> Self {
        Self {
            data: DataType::String(value),
            expiry,
        }
    }

    /// Creates a new list value (initially empty) with an optional expiry.
    #[allow(dead_code)]
    pub(crate) fn new_list(expiry: Option<DateTime<Utc>>) -> Self {
        Self {
            data: DataType::List(VecDeque::new()),
            expiry,
        }
    }

    // ── Accessors ────────────────────────────────────────────────────────

    /// Returns the raw bytes if the value is a `String`, or `Err(WrongType)`.
    pub(crate) fn as_string(&self) -> Result<&[u8], RadishError> {
        match &self.data {
            DataType::String(v) => Ok(v.as_slice()),
            _ => Err(RadishError::WrongType),
        }
    }

    /// Returns a mutable reference to the inner list, or `Err(WrongType)`.
    #[allow(dead_code)]
    pub(crate) fn as_list_mut(&mut self) -> Result<&mut VecDeque<Vec<u8>>, RadishError> {
        match &mut self.data {
            DataType::List(l) => Ok(l),
            _ => Err(RadishError::WrongType),
        }
    }

    /// Returns a shared reference to the inner list, or `Err(WrongType)`.
    #[allow(dead_code)]
    pub(crate) fn as_list(&self) -> Result<&VecDeque<Vec<u8>>, RadishError> {
        match &self.data {
            DataType::List(l) => Ok(l),
            _ => Err(RadishError::WrongType),
        }
    }

    // ── Expiry ───────────────────────────────────────────────────────────

    pub(crate) fn expiry(&self) -> Option<DateTime<Utc>> {
        self.expiry
    }

    pub(crate) fn set_expiry(&mut self, expiry: DateTime<Utc>) {
        self.expiry = Some(expiry);
    }

    /// Returns `true` if this value has passed its expiry time.
    pub(crate) fn is_expired(&self) -> bool {
        self.expiry
            .map_or(false, |exp| exp <= Utc::now())
    }

    // ── AOF persistence ──────────────────────────────────────────────────

    /// Writes the RESP command(s) needed to reconstruct this key into `writer`.
    ///
    /// This is allocation-free — individual `RespValue::BulkString` frames are
    /// written sequentially rather than building a temporary `Vec<RespValue>`.
    pub(crate) fn dump_aof_command<W: std::io::Write>(
        &self,
        key: &[u8],
        writer: &mut W,
    ) -> std::io::Result<()> {
        match &self.data {
            DataType::String(val) => {
                // *3\r\n  $3\r\nSET\r\n  $<klen>\r\n<key>\r\n  $<vlen>\r\n<val>\r\n
                writer.write_all(b"*3\r\n")?;
                RespValue::BulkString(b"SET").encode_to_writer(writer)?;
                RespValue::BulkString(key).encode_to_writer(writer)?;
                RespValue::BulkString(val).encode_to_writer(writer)?;
            }
            DataType::List(list) => {
                // *<2+N>\r\n  $5\r\nRPUSH\r\n  $<klen>\r\n<key>\r\n  ...elements...
                writer.write_all(b"*")?;
                writer.write_all((2 + list.len()).to_string().as_bytes())?;
                writer.write_all(b"\r\n")?;
                RespValue::BulkString(b"RPUSH").encode_to_writer(writer)?;
                RespValue::BulkString(key).encode_to_writer(writer)?;
                for item in list {
                    RespValue::BulkString(item).encode_to_writer(writer)?;
                }
            }
        }
        Ok(())
    }
}
