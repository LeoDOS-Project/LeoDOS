//
//! Safe, idiomatic wrapper for querying OSAL heap statistics.

use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// A snapshot of statistics for the system's dynamic memory heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapInfo {
    /// Total number of free bytes available in the heap.
    pub free_bytes: usize,
    /// Total number of free blocks in the heap.
    pub free_blocks: usize,
    /// The size of the largest contiguous free block available.
    pub largest_free_block: usize,
}

impl From<ffi::OS_heap_prop_t> for HeapInfo {
    fn from(prop: ffi::OS_heap_prop_t) -> Self {
        Self {
            free_bytes: prop.free_bytes,
            free_blocks: prop.free_blocks,
            largest_free_block: prop.largest_free_block,
        }
    }
}

impl HeapInfo {
    /// Retrieves statistics about the current state of the system heap.
    ///
    /// This function is a safe wrapper around `OS_HeapGetInfo`.
    pub fn query() -> Result<HeapInfo> {
        let mut prop = MaybeUninit::uninit();
        check(unsafe { ffi::OS_HeapGetInfo(prop.as_mut_ptr()) })?;
        Ok(HeapInfo::from(unsafe { prop.assume_init() }))
    }
}
