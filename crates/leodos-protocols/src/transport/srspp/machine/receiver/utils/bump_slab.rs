/// Append-only bump allocator over a fixed byte buffer.
pub struct BumpSlab<const N: usize> {
    /// Backing byte storage.
    data: [u8; N],
    /// Number of bytes currently allocated.
    len: usize,
}

impl<const N: usize> BumpSlab<N> {
    /// Create a new empty slab.
    pub const fn new() -> Self {
        Self {
            data: [0u8; N],
            len: 0,
        }
    }

    /// Append `payload` and return `(offset, len)`, or `None` if full.
    pub fn alloc(&mut self, payload: &[u8]) -> Option<(usize, usize)> {
        if self.len + payload.len() > N {
            return None;
        }
        let offset = self.len;
        self.data[offset..offset + payload.len()].copy_from_slice(payload);
        self.len += payload.len();
        Some((offset, payload.len()))
    }

    /// Get a slice of `len` bytes starting at `offset`.
    pub fn get(&self, offset: usize, len: usize) -> &[u8] {
        &self.data[offset..offset + len]
    }

    /// Reset the allocator, freeing all space.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Number of bytes currently allocated.
    pub fn used(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumpslab_alloc_get_clear() {
        let mut slab = BumpSlab::<64>::new();
        let (off1, len1) = slab.alloc(&[10, 20, 30]).unwrap();
        let (off2, len2) = slab.alloc(&[40, 50]).unwrap();
        assert_eq!(slab.get(off1, len1), &[10, 20, 30]);
        assert_eq!(slab.get(off2, len2), &[40, 50]);
        assert_eq!(slab.used(), 5);
        slab.clear();
        assert_eq!(slab.used(), 0);
    }

    #[test]
    fn bumpslab_full() {
        let mut slab = BumpSlab::<4>::new();
        assert!(slab.alloc(&[1, 2, 3]).is_some());
        assert!(slab.alloc(&[4, 5]).is_none());
    }
}
