/// Fixed-capacity byte ring buffer for variable-length packets.
///
/// Stores packets contiguously with a 2-byte length prefix.
/// When a packet doesn't fit between the write cursor and the
/// buffer end, the gap is skipped and the packet wraps to
/// position 0. Worst case wastes MTU bytes once — same as one
/// `Deque` slot — but typical utilization is much better since
/// small packets pack tightly.
///
/// When the ring is full, `push` returns `false` and the
/// packet is dropped (tail-drop policy).
pub struct RingBuffer<const N: usize> {
    buf: [u8; N],
    /// Write cursor (next push position).
    head: usize,
    /// Read cursor (next pop position).
    tail: usize,
    /// Number of packets currently stored.
    count: usize,
}

/// Length prefix size in bytes.
const LEN_SIZE: usize = 2;

impl<const N: usize> RingBuffer<N> {
    /// Create a new empty ring buffer.
    pub const fn new() -> Self {
        Self {
            buf: [0u8; N],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Push a packet into the ring. Returns `false` if full.
    pub fn push(&mut self, data: &[u8]) -> bool {
        let needed = LEN_SIZE + data.len();
        if needed > N {
            return false;
        }

        // Try to fit between head and end of buffer.
        let space_to_end = N - self.head;
        let head = if space_to_end < needed {
            // Not enough contiguous space — skip the gap.
            // Check if wrapping would overwrite unread data.
            if self.count > 0 && self.head >= self.tail {
                // Gap between head..N is wasted. We need
                // room at 0..tail for the new packet.
                if needed > self.tail {
                    return false;
                }
            }
            // Mark gap with a zero-length sentinel so pop
            // knows to skip it.
            self.buf[self.head] = 0;
            self.buf[self.head + 1] = 0;
            0
        } else {
            self.head
        };

        // Check for overlap with unread data.
        if self.count > 0 && head < self.tail && head + needed > self.tail {
            return false;
        }

        let len = data.len() as u16;
        self.buf[head] = (len >> 8) as u8;
        self.buf[head + 1] = len as u8;
        self.buf[head + LEN_SIZE..head + needed].copy_from_slice(data);
        self.head = head + needed;
        if self.head == N {
            self.head = 0;
        }
        self.count += 1;
        true
    }

    /// Peek at the front packet without removing it.
    pub fn front(&self) -> Option<&[u8]> {
        if self.count == 0 {
            return None;
        }
        let tail = self.skip_sentinel(self.tail);
        let len = self.read_len(tail);
        Some(&self.buf[tail + LEN_SIZE..tail + LEN_SIZE + len])
    }

    /// Remove the front packet.
    pub fn pop(&mut self) {
        if self.count == 0 {
            return;
        }
        let tail = self.skip_sentinel(self.tail);
        let len = self.read_len(tail);
        self.tail = tail + LEN_SIZE + len;
        if self.tail == N {
            self.tail = 0;
        }
        self.count -= 1;
        if self.count == 0 {
            self.head = 0;
            self.tail = 0;
        }
    }

    /// Number of packets currently stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` if the ring contains no packets.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Read the 2-byte big-endian length at position `pos`.
    fn read_len(&self, pos: usize) -> usize {
        ((self.buf[pos] as usize) << 8) | self.buf[pos + 1] as usize
    }

    /// Skip past a zero-length sentinel at `pos`, wrapping
    /// to 0 if found.
    fn skip_sentinel(&self, pos: usize) -> usize {
        if pos + LEN_SIZE <= N && self.read_len(pos) == 0 && pos != self.head {
            0
        } else {
            pos
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_pop_single() {
        let mut ring = RingBuffer::<64>::new();
        assert!(ring.push(&[1, 2, 3]));
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.front(), Some([1, 2, 3].as_slice()));
        ring.pop();
        assert!(ring.is_empty());
    }

    #[test]
    fn push_pop_multiple() {
        let mut ring = RingBuffer::<64>::new();
        assert!(ring.push(&[10, 20]));
        assert!(ring.push(&[30, 40, 50]));
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.front(), Some([10, 20].as_slice()));
        ring.pop();
        assert_eq!(ring.front(), Some([30, 40, 50].as_slice()));
        ring.pop();
        assert!(ring.is_empty());
    }

    #[test]
    fn wraps_around() {
        // 16-byte ring. Each packet uses LEN_SIZE + data.
        let mut ring = RingBuffer::<16>::new();
        // 2+4 = 6 bytes
        assert!(ring.push(&[1, 2, 3, 4]));
        // 2+4 = 6 bytes (total 12)
        assert!(ring.push(&[5, 6, 7, 8]));
        // Pop first to free space at the start.
        ring.pop();
        // 2+4 = 6 bytes — doesn't fit in remaining 4
        // bytes, should wrap to start.
        assert!(ring.push(&[9, 10, 11, 12]));
        assert_eq!(ring.front(), Some([5, 6, 7, 8].as_slice()));
        ring.pop();
        assert_eq!(ring.front(), Some([9, 10, 11, 12].as_slice()));
        ring.pop();
        assert!(ring.is_empty());
    }

    #[test]
    fn full_returns_false() {
        let mut ring = RingBuffer::<8>::new();
        // 2+3 = 5 bytes
        assert!(ring.push(&[1, 2, 3]));
        // 2+3 = 5 bytes — only 3 bytes left, won't fit.
        assert!(!ring.push(&[4, 5, 6]));
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn too_large_packet() {
        let mut ring = RingBuffer::<8>::new();
        // 2+7 = 9 > 8
        assert!(!ring.push(&[0; 7]));
    }

    #[test]
    fn many_small_packets() {
        let mut ring = RingBuffer::<32>::new();
        // Each packet: 2 + 1 = 3 bytes. Can fit 10.
        for i in 0..10u8 {
            assert!(ring.push(&[i]));
        }
        assert_eq!(ring.len(), 10);
        for i in 0..10u8 {
            assert_eq!(ring.front(), Some([i].as_slice()));
            ring.pop();
        }
        assert!(ring.is_empty());
    }

    #[test]
    fn reuse_after_drain() {
        let mut ring = RingBuffer::<16>::new();
        for _ in 0..5 {
            assert!(ring.push(&[1, 2, 3]));
            ring.pop();
        }
        assert!(ring.is_empty());
        assert!(ring.push(&[4, 5, 6, 7, 8]));
        assert_eq!(ring.front(), Some([4, 5, 6, 7, 8].as_slice()));
    }
}
