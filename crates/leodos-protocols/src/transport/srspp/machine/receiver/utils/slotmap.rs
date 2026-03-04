/// Fixed-size slot map with `N` slots of `SLOT` bytes each.
pub struct SlotMap<const TOTAL: usize, const N: usize, const SLOT: usize> {
    /// Flat byte storage for all slots.
    data: [u8; TOTAL],
    /// Per-slot payload lengths.
    lens: [u16; N],
}

impl<const TOTAL: usize, const N: usize, const SLOT: usize>
    SlotMap<TOTAL, N, SLOT>
{
    /// Create a new zeroed slot map.
    pub fn new() -> Self {
        Self {
            data: [0u8; TOTAL],
            lens: [0u16; N],
        }
    }

    /// Write payload into slot `i`.
    pub fn write(&mut self, i: usize, payload: &[u8]) {
        debug_assert!(i < N);
        debug_assert!(payload.len() <= SLOT);
        let start = i * SLOT;
        self.data[start..start + payload.len()].copy_from_slice(payload);
        self.lens[i] = payload.len() as u16;
    }

    /// Copy slot `i` into `dst`, returning the number of bytes copied.
    pub fn read(&self, i: usize, dst: &mut [u8]) -> usize {
        debug_assert!(i < N);
        let start = i * SLOT;
        let len = self.lens[i] as usize;
        dst[..len].copy_from_slice(&self.data[start..start + len]);
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slotmap_write_read() {
        let mut sm = SlotMap::<64, 4, 16>::new();
        sm.write(0, &[1, 2, 3]);
        sm.write(2, &[4, 5]);
        let mut buf = [0u8; 16];
        let len = sm.read(0, &mut buf);
        assert_eq!(&buf[..len], &[1, 2, 3]);
        let len = sm.read(2, &mut buf);
        assert_eq!(&buf[..len], &[4, 5]);
    }
}
