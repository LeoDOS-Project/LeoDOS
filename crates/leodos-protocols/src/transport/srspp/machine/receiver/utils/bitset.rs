/// Fixed-size bitset backed by a single `u32` (max 32 bits).
#[derive(Clone, Copy)]
pub struct Bitset<const N: usize> {
    /// Underlying bit storage.
    bits: u32,
}

impl<const N: usize> Bitset<N> {
    /// Create a new empty bitset.
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Set bit `i`.
    pub fn set(&mut self, i: usize) {
        debug_assert!(i < N);
        self.bits |= 1 << i;
    }

    /// Clear bit `i`.
    pub fn clear(&mut self, i: usize) {
        debug_assert!(i < N);
        self.bits &= !(1 << i);
    }

    /// Returns true if bit `i` is set.
    pub fn is_set(&self, i: usize) -> bool {
        debug_assert!(i < N);
        self.bits & (1 << i) != 0
    }

    /// Returns true if any bit is set.
    pub fn any(&self) -> bool {
        self.bits & ((1u32 << N) - 1) != 0
    }

    /// Clear all bits.
    pub fn clear_all(&mut self) {
        self.bits = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitset_basic() {
        let mut bs = Bitset::<8>::new();
        assert!(!bs.is_set(0));
        assert!(!bs.any());

        bs.set(3);
        assert!(bs.is_set(3));
        assert!(!bs.is_set(2));
        assert!(bs.any());

        bs.clear(3);
        assert!(!bs.is_set(3));
        assert!(!bs.any());
    }
}
