/// Tracks epoch numbers using a sliding window bitmask to detect duplicates.
/// Handles u16 wrap-around logic internally.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EpochTracker {
    history_mask: u128,
    /// The highest epoch number seen so far.
    highest_seen_epoch: u16,
    initialized: bool,
}

impl EpochTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn is_duplicate(&mut self, epoch: u16) -> bool {
        if !self.initialized {
            self.highest_seen_epoch = epoch;
            self.history_mask = 0;
            self.initialized = true;
            return false;
        }

        // Calculate the difference considering wrap-around.
        let delta = epoch.wrapping_sub(self.highest_seen_epoch);

        // If the epoch is the same as the highest seen, it's a duplicate.
        if delta == 0 {
            return true;
        }

        // If the epoch is newer than the highest seen by less than 32768,
        if delta < u16::MAX.div_ceil(2) {
            if delta >= u8::MAX.div_ceil(2) as u16 {
                self.history_mask = 0;
            } else {
                self.history_mask = (self.history_mask << delta) | 1 << (delta - 1);
            }
            self.highest_seen_epoch = epoch;
            return false;
        }

        let offset = self.highest_seen_epoch.wrapping_sub(epoch);

        if offset > (1 << 7) {
            return true;
        }

        let mask = 1 << (offset - 1);

        if (self.history_mask & mask) != 0 {
            return true;
        }

        self.history_mask |= mask;
        false
    }
}
