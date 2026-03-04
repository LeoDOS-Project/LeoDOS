use heapless::Vec;

#[derive(Clone, Copy)]
pub struct Bitset<const N: usize> {
    bits: u32,
}

impl<const N: usize> Bitset<N> {
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    pub fn set(&mut self, i: usize) {
        debug_assert!(i < N);
        self.bits |= 1 << i;
    }

    pub fn clear(&mut self, i: usize) {
        debug_assert!(i < N);
        self.bits &= !(1 << i);
    }

    pub fn is_set(&self, i: usize) -> bool {
        debug_assert!(i < N);
        self.bits & (1 << i) != 0
    }

    pub fn any(&self) -> bool {
        self.bits & ((1u32 << N) - 1) != 0
    }

    pub fn clear_all(&mut self) {
        self.bits = 0;
    }
}

pub struct SlotMap<const TOTAL: usize, const N: usize, const SLOT: usize> {
    data: [u8; TOTAL],
    lens: [u16; N],
}

impl<const TOTAL: usize, const N: usize, const SLOT: usize>
    SlotMap<TOTAL, N, SLOT>
{
    pub fn new() -> Self {
        Self {
            data: [0u8; TOTAL],
            lens: [0u16; N],
        }
    }

    pub fn write(&mut self, i: usize, payload: &[u8]) {
        debug_assert!(i < N);
        debug_assert!(payload.len() <= SLOT);
        let start = i * SLOT;
        self.data[start..start + payload.len()].copy_from_slice(payload);
        self.lens[i] = payload.len() as u16;
    }

    pub fn read(&self, i: usize, dst: &mut [u8]) -> usize {
        debug_assert!(i < N);
        let start = i * SLOT;
        let len = self.lens[i] as usize;
        dst[..len].copy_from_slice(&self.data[start..start + len]);
        len
    }
}

pub struct BumpSlab<const N: usize> {
    data: [u8; N],
    len: usize,
}

impl<const N: usize> BumpSlab<N> {
    pub const fn new() -> Self {
        Self {
            data: [0u8; N],
            len: 0,
        }
    }

    pub fn alloc(&mut self, payload: &[u8]) -> Option<(usize, usize)> {
        if self.len + payload.len() > N {
            return None;
        }
        let offset = self.len;
        self.data[offset..offset + payload.len()].copy_from_slice(payload);
        self.len += payload.len();
        Some((offset, payload.len()))
    }

    pub fn get(&self, offset: usize, len: usize) -> &[u8] {
        &self.data[offset..offset + len]
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn used(&self) -> usize {
        self.len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    pub start: usize,
    pub end: usize,
}

pub struct GapTracker<const N: usize> {
    gaps: Vec<Interval, N>,
    high: usize,
}

impl<const N: usize> GapTracker<N> {
    pub fn new() -> Self {
        Self {
            gaps: Vec::new(),
            high: 0,
        }
    }

    pub fn fill(&mut self, start: usize, end: usize) {
        if start >= self.high {
            if start > self.high {
                let _ = self.gaps.push(Interval {
                    start: self.high,
                    end: start,
                });
            }
            self.high = end;
            return;
        }
        let mut i = 0;
        while i < self.gaps.len() {
            let g = self.gaps[i];
            if start <= g.start && end >= g.end {
                self.gaps.remove(i);
            } else if end <= g.start || start >= g.end {
                i += 1;
            } else if start <= g.start {
                self.gaps[i].start = end;
                i += 1;
            } else if end >= g.end {
                self.gaps[i].end = start;
                i += 1;
            } else {
                let old_end = self.gaps[i].end;
                self.gaps[i].end = start;
                let _ = self.gaps.insert(
                    i + 1,
                    Interval {
                        start: end,
                        end: old_end,
                    },
                );
                return;
            }
        }
    }

    pub fn is_complete_to(&self, offset: usize) -> bool {
        !self.gaps.iter().any(|g| g.start < offset)
    }

    pub fn shift(&mut self, amount: usize) {
        self.high = self.high.saturating_sub(amount);
        self.gaps.retain(|g| g.end > amount);
        for g in self.gaps.iter_mut() {
            g.start = g.start.saturating_sub(amount);
            g.end -= amount;
        }
    }

    pub fn has_gaps(&self) -> bool {
        !self.gaps.is_empty()
    }

    pub fn reset(&mut self) {
        self.gaps.clear();
        self.high = 0;
    }

    pub fn high_water(&self) -> usize {
        self.high
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

    #[test]
    fn gap_tracker_in_order() {
        let mut gt = GapTracker::<8>::new();
        gt.fill(0, 512);
        gt.fill(512, 1024);
        assert!(gt.is_complete_to(1024));
        assert!(!gt.has_gaps());
    }

    #[test]
    fn gap_tracker_out_of_order() {
        let mut gt = GapTracker::<8>::new();
        gt.fill(0, 512);
        gt.fill(1024, 1536);
        assert!(!gt.is_complete_to(1536));
        assert!(gt.has_gaps());

        gt.fill(512, 1024);
        assert!(gt.is_complete_to(1536));
        assert!(!gt.has_gaps());
    }

    #[test]
    fn gap_tracker_shift() {
        let mut gt = GapTracker::<8>::new();
        gt.fill(0, 512);
        gt.fill(1024, 1536);
        gt.shift(512);
        assert_eq!(gt.high_water(), 1024);
        assert!(!gt.is_complete_to(1024));
        gt.fill(0, 512);
        assert!(gt.is_complete_to(1024));
    }

    #[test]
    fn gap_tracker_split() {
        let mut gt = GapTracker::<8>::new();
        gt.fill(0, 100);
        gt.fill(200, 300);
        assert!(gt.has_gaps());
        gt.fill(130, 170);
        assert!(gt.has_gaps());
        gt.fill(100, 130);
        gt.fill(170, 200);
        assert!(gt.is_complete_to(300));
    }
}
