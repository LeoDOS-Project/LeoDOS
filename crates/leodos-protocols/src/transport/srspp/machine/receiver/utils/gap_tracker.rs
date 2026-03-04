use heapless::Vec;

/// A half-open byte range `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interval {
    /// Inclusive start offset.
    pub start: usize,
    /// Exclusive end offset.
    pub end: usize,
}

/// Tracks unfilled gaps in a contiguous byte range.
pub struct GapTracker<const N: usize> {
    /// Sorted list of gap intervals.
    gaps: Vec<Interval, N>,
    /// High-water mark: the end of the highest filled range.
    high: usize,
}

impl<const N: usize> GapTracker<N> {
    /// Create a new empty gap tracker.
    pub fn new() -> Self {
        Self {
            gaps: Vec::new(),
            high: 0,
        }
    }

    /// Mark the range `[start, end)` as filled, merging or splitting gaps.
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

    /// Returns true if the range `[0, offset)` has no gaps.
    pub fn is_complete_to(&self, offset: usize) -> bool {
        !self.gaps.iter().any(|g| g.start < offset)
    }

    /// Shift all offsets left by `amount`, discarding consumed data.
    pub fn shift(&mut self, amount: usize) {
        self.high = self.high.saturating_sub(amount);
        self.gaps.retain(|g| g.end > amount);
        for g in self.gaps.iter_mut() {
            g.start = g.start.saturating_sub(amount);
            g.end -= amount;
        }
    }

    /// Returns true if there are any unfilled gaps.
    pub fn has_gaps(&self) -> bool {
        !self.gaps.is_empty()
    }

    /// Reset the tracker to its initial empty state.
    pub fn reset(&mut self) {
        self.gaps.clear();
        self.high = 0;
    }

    /// Returns the high-water mark.
    pub fn high_water(&self) -> usize {
        self.high
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
