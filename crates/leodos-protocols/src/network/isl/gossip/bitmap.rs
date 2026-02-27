#[derive(Debug, Clone, Copy, Default)]
struct Bitmap(u128);

impl Bitmap {
    const CAPACITY: u16 = u128::BITS as u16;

    /// Shift the bits left by distance, filling with zeros.
    fn shift(&mut self, distance: u16) {
        if distance >= Self::CAPACITY {
            self.0 = 0;
        } else {
            self.0 <<= distance;
        }
    }

    /// Set bit at specific 0-based index.
    fn mark(&mut self, index: u16) {
        if index >= Self::CAPACITY {
            return;
        }
        self.0 |= 1 << index;
    }

    /// Check bit at specific 0-based index.
    fn is_marked(&self, index: u16) -> bool {
        if index >= Self::CAPACITY {
            return false;
        }
        (self.0 & (1 << index)) != 0
    }
}

#[derive(Debug, Clone, Copy)]
enum ShortestPath {
    /// The points are identical.
    Coincident,
    /// The shortest path follows the direction of the sequence (Future/Newer).
    Forward { distance: u16 },
    /// The shortest path goes against the direction of the sequence (Past/Older).
    Backward { distance: u16 },
}

impl ShortestPath {
    /// Determines the position of `target` relative to `reference` on a u16 circle.
    fn compute(from: u16, to: u16) -> Self {
        let fwd_dist = to.wrapping_sub(from);

        if fwd_dist == 0 {
            return Self::Coincident;
        }

        let bwd_dist = from.wrapping_sub(to);

        if fwd_dist < bwd_dist {
            Self::Forward { distance: fwd_dist }
        } else {
            Self::Backward { distance: bwd_dist }
        }
    }
}

/// Capable of storing up to 129 unique u16 entries and identifying duplicates.
/// * The `head` represents the most recent entry.
/// * The `history` bitmap tracks the previous 128 entries relative to the head.
///
/// The filter works as follows:
/// * Entries older than 128 positions from the head are considered duplicates.
/// * Entries identical to the head are considered duplicates.
/// * Newer entries update the head and adjust the history accordingly.
/// * Older entries within the 128-position range are checked against the history.
#[derive(Debug, Clone, Copy, Default)]
pub struct DuplicateFilter {
    head: u16,
    history: Bitmap,
    initialized: bool,
}

impl DuplicateFilter {
    /// Creates a new empty duplicate filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns `true` if `new` has already been seen, otherwise records it.
    pub fn is_duplicate(&mut self, new: u16) -> bool {
        if !self.initialized {
            self.head = new;
            self.initialized = true;
            return false;
        }

        match ShortestPath::compute(self.head, new) {
            ShortestPath::Coincident => true,
            ShortestPath::Forward { distance } => {
                // `new` is a newer entry than `head`.
                self.history.shift(distance); // Shift history to make room for `new`.
                self.history.mark(distance - 1); // Mark the position of 'head'.
                self.head = new;
                false
            }
            ShortestPath::Backward { distance } => {
                // `new` is an older entry than `head`.
                if distance > Bitmap::CAPACITY {
                    return true; // Too far back.
                }
                if self.history.is_marked(distance - 1) {
                    return true; // Already marked.
                }
                self.history.mark(distance - 1); // Mark the position of 'new'.
                false
            }
        }
    }
}
