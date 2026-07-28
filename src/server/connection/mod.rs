mod connection_guard;
mod read_buffer;

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::command::RadishCommand;
use crate::handler::Dispatcher;
use crate::storage::SharedStore;
pub(crate) use connection_guard::ConnectionGuard;
use read_buffer::ReadBuffer;

const MAX_BUFFER_SIZE_ERROR_RESPONSE: &[u8] = b"-ERR Maximum buffer size exceeded\r\n";
const CONNECTION_TIMEOUT_RESPONSE: &[u8] = b"-ERR Connection timed out\r\n";

/// Owns all state for a single accepted TCP connection.
///
/// Read buffering is delegated to [`ReadBuffer`], which encapsulates the
/// sliding-window strategy (compact / grow / 1 MiB cap) behind a clean API.
pub(crate) struct Connection {
    stream: TcpStream,
    store: SharedStore,
    /// Dropped when `Connection` is dropped, decrementing the active count.
    _guard: ConnectionGuard,
    /// Sliding-window read buffer for incoming RESP data.
    read_buf: ReadBuffer,
    /// Accumulated outgoing responses; flushed in one syscall per loop tick.
    write_buf: Vec<u8>,
}

impl Connection {
    pub(crate) fn new(stream: TcpStream, store: SharedStore, guard: ConnectionGuard) -> Self {
        Self {
            stream,
            store,
            _guard: guard,
            read_buf: ReadBuffer::new(8192),
            write_buf: Vec::with_capacity(8192),
        }
    }

    /// Runs the connection event loop until the client disconnects, times out,
    /// or a fatal error occurs.
    pub(crate) async fn run(mut self) {
        loop {
            // ── Buffer management ─────────────────────────────────────────
            if self.read_buf.ensure_space().is_err() {
                let _ = self.stream.write_all(MAX_BUFFER_SIZE_ERROR_RESPONSE).await;
                break;
            }

            // ── Read ──────────────────────────────────────────────────────
            let n = match self.read_bytes().await {
                Some(n) => n,
                None => break,
            };
            self.read_buf.fill(n);

            // ── Parse & dispatch ──────────────────────────────────────────
            // `read_buf` and `write_buf` are disjoint fields, so borrowing
            // `read_buf.unparsed()` no longer conflicts with `write_buf`.
            while !self.read_buf.unparsed().is_empty() {
                match RadishCommand::try_parse(self.read_buf.unparsed()) {
                    Ok(Some((cmd, consumed))) => {
                        Dispatcher::eval(cmd, &self.store, &mut self.write_buf);
                        self.read_buf.consume(consumed);
                    }
                    Ok(None) => break, // incomplete — wait for the next read
                    Err(e) => {
                        let msg = format!("-ERR {}\r\n", e);
                        self.write_buf.extend_from_slice(msg.as_bytes());
                        break; // flush the error, then drop the connection
                    }
                }
            }

            // ── Flush ─────────────────────────────────────────────────────
            if self.flush_writes().await.is_err() {
                break;
            }

            self.read_buf.reset_if_empty();
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Reads bytes from the stream into the read buffer with a 30 s timeout.
    ///
    /// Returns `Some(n)` on a successful read, `None` on EOF, timeout, or error.
    async fn read_bytes(&mut self) -> Option<usize> {
        let io_result = match tokio::time::timeout(
            Duration::from_secs(30),
            self.stream.read(self.read_buf.spare()),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => {
                let _ = self.stream.write_all(CONNECTION_TIMEOUT_RESPONSE).await;
                return None;
            }
        };

        match io_result {
            Ok(0) => None, // clean EOF — client closed the connection
            Ok(n) => Some(n),
            Err(_) => None, // I/O error
        }
    }

    /// Flushes all accumulated responses in `write_buf` to the stream in one
    /// `write_all` syscall, then clears the buffer for reuse.
    async fn flush_writes(&mut self) -> Result<(), ()> {
        if self.write_buf.is_empty() {
            return Ok(());
        }
        // println!("Flushing {} bytes to client",String::from_utf8_lossy(&self.write_buf));
        self.stream
            .write_all(&self.write_buf)
            .await
            .map_err(|_| ())?;
        self.write_buf.clear();
        Ok(())
    }
}
