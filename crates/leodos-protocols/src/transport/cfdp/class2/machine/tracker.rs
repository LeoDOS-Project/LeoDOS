//! A helper for tracking received file segments and identifying gaps.

use heapless::Vec;

use crate::transport::cfdp::CfdpError;

/// The maximum number of distinct missing-data gaps we can track.
/// This limits the size of a NAK PDU.
const MAX_GAPS: usize = 4;

/// Sentinel value representing an unused slot in the received ranges array.
const EMPTY_SLOT: (u64, u64) = (u64::MAX, u64::MAX);

/// A data structure to track received segments of a file.
///
/// This implementation uses a sorted, merged list of received ranges to efficiently
/// track progress and identify missing data.
#[derive(Debug, Clone)]
pub struct SegmentTracker {
    /// A sorted list of disjoint ranges representing received data.
    /// Invariant: For any two ranges (s1, e1) and (s2, e2) in the list, e1 < s2.
    received_ranges: [(u64, u64); MAX_GAPS],
    /// The total expected size of the file in bytes.
    file_size: u64,
}

impl SegmentTracker {
    /// Creates a new tracker for a file of a given size.
    pub fn new(file_size: u64) -> Self {
        Self {
            received_ranges: [(u64::MAX, u64::MAX); MAX_GAPS],
            file_size,
        }
    }

    /// Helper to get an iterator over the valid ranges.
    fn valid_ranges(&self) -> impl Iterator<Item = &(u64, u64)> {
        self.received_ranges
            .iter()
            .take_while(|&&r| r != EMPTY_SLOT)
    }

    /// Records that a segment of the file has been received.
    /// This method will merge overlapping or adjacent ranges to maintain a minimal
    /// list of disjoint received segments.
    pub fn add_segment(&mut self, offset: u64, len: u64) -> Result<(), CfdpError> {
        if len == 0 {
            return Ok(());
        }

        let mut new_start = offset;
        let mut new_end = offset + len;

        let mut next_ranges: Vec<(u64, u64), MAX_GAPS, u8> = Vec::new();

        for &(start, end) in self.valid_ranges() {
            if end < new_start {
                // Case 1: Disjoint and before. Keep it.
                next_ranges.push((start, end)).ok();
            } else if start > new_end {
                // Case 2: Disjoint and after. Keep it.
                next_ranges.push((start, end)).ok();
            } else {
                // Case 3: Overlap. Merge by expanding the new range's bounds.
                new_start = new_start.min(start);
                new_end = new_end.max(end);
            }
        }

        if next_ranges.len() == MAX_GAPS {
            return Ok(());
        }
        next_ranges.push((new_start, new_end)).unwrap();

        // Sort the temporary Vec to ensure the final array is sorted.
        next_ranges.sort_unstable_by_key(|(start, _)| *start);

        // Now, copy the result from the temporary Vec back into our array.
        let mut final_ranges = [EMPTY_SLOT; MAX_GAPS];
        for (i, range) in next_ranges.iter().enumerate() {
            final_ranges[i] = *range;
        }
        self.received_ranges = final_ranges;
        Ok(())
    }

    /// Returns a list of ranges (offset, length) that are still missing.
    pub fn get_missing_ranges(&self) -> impl Iterator<Item = (u64, u64)> + '_ {
        let mut last_offset = 0;
        self.received_ranges
            .iter()
            .map(move |(start, end)| {
                let missing_start = last_offset;
                last_offset = *end;
                if *start > missing_start {
                    Some((missing_start, *start))
                } else {
                    None
                }
            })
            .filter_map(|x| x)
            .chain(if last_offset < self.file_size {
                Some((last_offset, self.file_size))
            } else {
                None
            })
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
