mod connection_guard;

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::command::RadishCommand;
use crate::handler::Dispatcher;
use crate::storage::SharedStore;
pub(crate) use connection_guard::ConnectionGuard;

/// Owns all state for a single accepted TCP connection.
///
/// Buffer layout (sliding window over a `Vec<u8>`):
/// ```
/// [ consumed | unparsed | free ]
///  0        head       tail   len
/// ```
/// Advancing `head` consumes parsed bytes without shifting memory.
/// When all bytes are consumed (`head == tail`), both pointers reset to 0,
/// reclaiming the full buffer in O(1).
pub(crate) struct Connection {
    stream: TcpStream,
    store: SharedStore,
    /// Dropped when `Connection` is dropped, decrementing the active count.
    _guard: ConnectionGuard,
    /// Read ring-buffer (sliding window).
    buffer: Vec<u8>,
    /// Start of unparsed data.
    head: usize,
    /// End of received data (next write position).
    tail: usize,
    /// Accumulated outgoing responses; flushed in one syscall per loop tick.
    write_buf: Vec<u8>,
}

impl Connection {
    pub(crate) fn new(stream: TcpStream, store: SharedStore, guard: ConnectionGuard) -> Self {
        Self {
            stream,
            store,
            _guard: guard,
            buffer: vec![0u8; 8192],
            head: 0,
            tail: 0,
            write_buf: Vec::with_capacity(8192),
        }
    }

    /// Runs the connection event loop until the client disconnects, times out,
    /// or a fatal error occurs.
    pub(crate) async fn run(mut self) {
        loop {
            // ── Buffer management ─────────────────────────────────────────
            if !self.ensure_buffer_space().await {
                break;
            }

            // ── Read ──────────────────────────────────────────────────────
            let n = match self.read_bytes().await {
                Some(n) => n,
                None => break,
            };
            self.tail += n;

            // ── Parse & dispatch ──────────────────────────────────────────
            // Kept inline: `RadishCommand<'a>` borrows from `self.buffer`,
            // so splitting this into a `&mut self` method would run into
            // borrow-checker conflicts with `self.write_buf`.
            while self.head < self.tail {
                match RadishCommand::try_parse(&self.buffer[self.head..self.tail]) {
                    Ok(Some((cmd, consumed))) => {
                        self.head += consumed;
                        Dispatcher::eval(cmd, &self.store, &mut self.write_buf);
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

            // Reset pointers when the buffer is fully consumed — O(1),
            // no memory shifting required.
            if self.head == self.tail {
                self.head = 0;
                self.tail = 0;
            }
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────

    /// Ensures there is space in `buffer[tail..]` for the next read.
    ///
    /// Returns `false` if the buffer exceeded the 1 MB hard limit.
    async fn ensure_buffer_space(&mut self) -> bool {
        if self.tail < self.buffer.len() {
            return true; // space already available
        }

        if self.head > 0 {
            // Reclaim space by shifting unconsumed data to the front.
            self.buffer.copy_within(self.head..self.tail, 0);
            self.tail -= self.head;
            self.head = 0;
        } else {
            // Buffer is full of a single giant unparsed command — grow it.
            let new_len = self.buffer.len() * 2;
            if new_len > 1024 * 1024 {
                let _ = self
                    .stream
                    .write_all(b"-ERR Maximum buffer size exceeded\r\n")
                    .await;
                return false;
            }
            self.buffer.resize(new_len, 0);
        }
        true
    }

    /// Reads bytes from the stream into `buffer[tail..]` with a 30 s timeout.
    ///
    /// Returns `Some(n)` on a successful read, `None` on EOF, timeout, or error.
    async fn read_bytes(&mut self) -> Option<usize> {
        let io_result = match tokio::time::timeout(
            Duration::from_secs(30),
            self.stream.read(&mut self.buffer[self.tail..]),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => {
                let _ = self
                    .stream
                    .write_all(b"-ERR Connection timed out\r\n")
                    .await;
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
        self.stream
            .write_all(&self.write_buf)
            .await
            .map_err(|_| ())?;
        self.write_buf.clear();
        Ok(())
    }
}
