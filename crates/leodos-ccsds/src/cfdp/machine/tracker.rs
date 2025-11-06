//! A helper for tracking received file segments and identifying gaps.

use heapless::Vec;
use zerocopy::byteorder::network_endian::U64;

/// The maximum number of distinct missing-data gaps we can track.
/// This limits the size of a NAK PDU.
const MAX_GAPS: usize = 32;

/// A data structure to track received segments of a file.
///
/// This implementation uses a sorted, merged list of received ranges to efficiently
/// track progress and identify missing data.
#[derive(Debug, Clone)]
pub struct SegmentTracker {
    /// A sorted list of disjoint ranges representing received data.
    /// Invariant: For any two ranges (s1, e1) and (s2, e2) in the list, e1 < s2.
    received_ranges: Vec<(u64, u64), MAX_GAPS>,
    file_size: u64,
}

impl SegmentTracker {
    /// Creates a new tracker for a file of a given size.
    pub fn new(file_size: u64) -> Self {
        Self {
            received_ranges: Vec::new(),
            file_size,
        }
    }

    /// Records that a segment of the file has been received.
    /// This method will merge overlapping or adjacent ranges to maintain a minimal
    /// list of disjoint received segments.
    pub fn add_segment(&mut self, offset: u64, len: u64) {
        if len == 0 {
            return;
        }
        let mut new_start = offset;
        let mut new_end = offset + len;

        let mut next_ranges = Vec::new();

        // Use core::mem::take to iterate over the existing ranges while building a new list.
        for (start, end) in core::mem::take(&mut self.received_ranges) {
            // Case 1: The existing range is completely disjoint and before the new one.
            if end < new_start {
                next_ranges.push((start, end)).ok();
            // Case 2: The existing range is completely disjoint and after the new one.
            } else if start > new_end {
                next_ranges.push((start, end)).ok();
            // Case 3: The ranges overlap or are adjacent. Merge them.
            } else {
                new_start = new_start.min(start);
                new_end = new_end.max(end);
            }
        }

        // Add the new, possibly merged, range.
        next_ranges.push((new_start, new_end)).ok();

        // The list might not be sorted if a merge occurred. Sort it to maintain the invariant.
        next_ranges.sort_unstable_by_key(|(start, _)| *start);

        self.received_ranges = next_ranges;
    }

    /// Returns a list of ranges (offset, length) that are still missing.
    pub fn get_missing_ranges(&self) -> Vec<(U64, U64), MAX_GAPS> {
        let mut missing = Vec::new();
        let mut last_offset: u64 = 0;

        for (start, end) in self.received_ranges.iter() {
            if *start > last_offset {
                missing
                    .push((U64::new(last_offset), U64::new(*start - last_offset)))
                    .unwrap_or_default();
            }
            last_offset = *end;
        }

        if last_offset < self.file_size {
            missing
                .push((
                    U64::new(last_offset),
                    U64::new(self.file_size - last_offset),
                ))
                .unwrap_or_default();
        }

        missing
    }

    /// Checks if the entire file has been received.
    pub fn is_complete(&self) -> bool {
        // The file is complete if there is exactly one range,
        // and that range is [0, file_size).
        if self.received_ranges.len() == 1 {
            if let Some((start, end)) = self.received_ranges.first() {
                return *start == 0 && *end >= self.file_size;
            }
        }
        false
    }
}
