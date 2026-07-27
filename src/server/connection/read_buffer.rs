/// A growable sliding-window byte buffer for streaming reads.
///
/// Buffer layout:
/// ```text
/// [ consumed | unparsed | free ]
///  0        head       tail   len
/// ```
/// Advancing `head` consumes parsed bytes without shifting memory.
/// When all bytes are consumed (`head == tail`), both pointers reset to 0,
/// reclaiming the full buffer in O(1).
pub(crate) struct ReadBuffer {
    buf: Vec<u8>,
    /// Start of unparsed data.
    head: usize,
    /// End of received data (next write position).
    tail: usize,
}

impl ReadBuffer {
    /// Hard upper limit on buffer growth (1 MiB).
    const MAX_SIZE: usize = 1024 * 1024;

    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            head: 0,
            tail: 0,
        }
    }

    /// Returns unparsed bytes as a contiguous slice.
    pub(crate) fn unparsed(&self) -> &[u8] {
        &self.buf[self.head..self.tail]
    }

    /// Returns the writable tail region for `AsyncReadExt::read()`.
    pub(crate) fn spare(&mut self) -> &mut [u8] {
        &mut self.buf[self.tail..]
    }

    /// Record `n` freshly-read bytes at the tail.
    pub(crate) fn fill(&mut self, n: usize) {
        self.tail += n;
    }

    /// Advance the head past `n` consumed (parsed) bytes.
    pub(crate) fn consume(&mut self, n: usize) {
        self.head += n;
    }

    /// Reset head and tail when the buffer is fully consumed — O(1).
    pub(crate) fn reset_if_empty(&mut self) {
        if self.head == self.tail {
            self.head = 0;
            self.tail = 0;
        }
    }

    /// Ensures there is space in the tail region for the next read.
    ///
    /// - If the tail hasn't reached the end, returns `Ok(())` immediately.
    /// - If consumed bytes exist at the front, compacts by shifting data left.
    /// - Otherwise doubles the buffer, failing if this would exceed `MAX_SIZE`.
    pub(crate) fn ensure_space(&mut self) -> Result<(), ()> {
        if self.tail < self.buf.len() {
            return Ok(()); // space already available
        }

        if self.head > 0 {
            // Reclaim space by shifting unconsumed data to the front.
            self.buf.copy_within(self.head..self.tail, 0);
            self.tail -= self.head;
            self.head = 0;
        } else {
            // Buffer is full of a single giant unparsed payload — grow it.
            let new_len = self.buf.len() * 2;
            if new_len > Self::MAX_SIZE {
                return Err(());
            }
            self.buf.resize(new_len, 0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_starts_empty() {
        let mut buf = ReadBuffer::new(64);
        assert!(buf.unparsed().is_empty());
        assert_eq!(buf.spare().len(), 64);
    }

    #[test]
    fn fill_and_consume_round_trip() {
        let mut buf = ReadBuffer::new(64);
        buf.spare()[..5].copy_from_slice(b"hello");
        buf.fill(5);

        assert_eq!(buf.unparsed(), b"hello");

        buf.consume(3);
        assert_eq!(buf.unparsed(), b"lo");
    }

    #[test]
    fn reset_if_empty_reclaims_buffer() {
        let mut buf = ReadBuffer::new(64);
        buf.spare()[..4].copy_from_slice(b"test");
        buf.fill(4);
        buf.consume(4);

        assert!(buf.unparsed().is_empty());
        // Before reset, spare is only 60 bytes.
        assert_eq!(buf.spare().len(), 60);

        buf.reset_if_empty();
        // After reset, full 64 bytes available again.
        assert_eq!(buf.spare().len(), 64);
    }

    #[test]
    fn ensure_space_compacts_when_possible() {
        let mut buf = ReadBuffer::new(8);
        // Fill the entire buffer.
        buf.spare()[..8].copy_from_slice(b"abcdefgh");
        buf.fill(8);
        // Consume the first 6 bytes.
        buf.consume(6);

        // No spare space left, but compaction should reclaim it.
        assert!(buf.ensure_space().is_ok());
        assert_eq!(buf.unparsed(), b"gh");
        assert_eq!(buf.spare().len(), 6);
    }

    #[test]
    fn ensure_space_grows_when_nothing_consumed() {
        let mut buf = ReadBuffer::new(8);
        buf.spare()[..8].copy_from_slice(b"abcdefgh");
        buf.fill(8);

        // Nothing consumed, head == 0. Must grow.
        assert!(buf.ensure_space().is_ok());
        assert_eq!(buf.unparsed(), b"abcdefgh");
        assert_eq!(buf.spare().len(), 8); // doubled from 8 → 16
    }

    #[test]
    fn ensure_space_rejects_beyond_max_size() {
        // Start at MAX_SIZE already.
        let mut buf = ReadBuffer::new(ReadBuffer::MAX_SIZE);
        // Fill it completely with head == 0.
        buf.fill(ReadBuffer::MAX_SIZE);

        assert!(buf.ensure_space().is_err());
    }
}
