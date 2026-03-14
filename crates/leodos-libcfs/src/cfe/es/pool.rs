//! Safe, idiomatic wrappers for the CFE Executive Services Memory Pool API.
//!
//! This module provides a `MemPool` handle and a `PoolBuffer` RAII guard
//! to ensure that memory allocated from a pool is always returned, preventing
//! memory leaks.

use crate::error::{CfsError, Result};
use crate::ffi;
use crate::status::check;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::slice;

// Add these new struct definitions before the `MemPool` struct
/// Statistics about a specific block size within a memory pool.
#[derive(Debug, Clone, Copy)]
pub struct BlockStats {
    /// Number of bytes in each of these blocks.
    pub block_size: u32,
    /// Number of memory blocks of this size created.
    pub num_created: u32,
    /// Number of memory blocks of this size that are free.
    pub num_free: u32,
}

impl From<ffi::CFE_ES_BlockStats_t> for BlockStats {
    fn from(stats: ffi::CFE_ES_BlockStats_t) -> Self {
        Self {
            block_size: stats.BlockSize,
            num_created: stats.NumCreated,
            num_free: stats.NumFree,
        }
    }
}

/// Statistics about a cFE memory pool.
#[derive(Debug, Clone, Copy)]
pub struct MemPoolStats {
    /// Total size of the memory pool in bytes.
    pub pool_size: u32,
    /// Number of times a memory block has been allocated.
    pub num_blocks_requested: u32,
    /// Number of errors detected when freeing a memory block.
    pub check_err_ctr: u32,
    /// Number of bytes never allocated to a block.
    pub num_free_bytes: u32,
    /// Statistics for each available block size.
    pub block_stats: [BlockStats; ffi::CFE_MISSION_ES_POOL_MAX_BUCKETS as usize],
}

impl From<ffi::CFE_ES_MemPoolStats_t> for MemPoolStats {
    fn from(stats: ffi::CFE_ES_MemPoolStats_t) -> Self {
        let mut block_stats = [BlockStats {
            block_size: 0,
            num_created: 0,
            num_free: 0,
        }; ffi::CFE_MISSION_ES_POOL_MAX_BUCKETS as usize];
        for i in 0..block_stats.len() {
            block_stats[i] = stats.BlockStats[i].into();
        }

        Self {
            pool_size: stats.PoolSize,
            num_blocks_requested: stats.NumBlocksRequested,
            check_err_ctr: stats.CheckErrCtr,
            num_free_bytes: stats.NumFreeBytes,
            block_stats,
        }
    }
}

/// A handle to a cFE Executive Services memory pool.
///
/// This struct is an RAII wrapper that ensures the underlying pool is deleted
/// when it goes out of scope. The memory for the pool itself must have a
/// `'static` lifetime.
#[derive(Debug)]
pub struct MemPool {
    handle: ffi::CFE_ES_MemHandle_t,
}

impl MemPool {
    /// Creates a new memory pool from a statically allocated memory region.
    ///
    /// This uses the default cFE block sizes for the pool.
    ///
    /// The pool size must be an integral number of 32-bit words, the
    /// start address must be 32-bit aligned, and 168 bytes are
    /// reserved for internal bookkeeping.
    ///
    /// # Arguments
    /// * `memory`: A mutable static byte slice to be used as the pool's memory.
    /// * `use_mutex`: If `true`, access to the pool will be protected by a mutex.
    ///
    /// # Errors
    /// Returns an error if the memory pool cannot be created, e.g., due to an
    /// invalid memory pointer or size.
    pub fn new(memory: &'static mut [u8], use_mutex: bool) -> Result<Self> {
        let mut handle = ffi::CFE_ES_MEMHANDLE_UNDEFINED;
        let status = if use_mutex {
            unsafe {
                ffi::CFE_ES_PoolCreate(&mut handle, memory.as_mut_ptr() as *mut _, memory.len())
            }
        } else {
            unsafe {
                ffi::CFE_ES_PoolCreateNoSem(
                    &mut handle,
                    memory.as_mut_ptr() as *mut _,
                    memory.len(),
                )
            }
        };
        check(status)?;
        Ok(Self { handle })
    }

    /// Creates a new memory pool with user-defined block sizes.
    ///
    /// # Arguments
    /// * `memory`: A mutable static byte slice for the pool's memory.
    /// * `use_mutex`: If `true`, access to the pool is protected by a mutex.
    /// * `block_sizes`: A slice of `usize` defining the bucket sizes for the pool.
    ///
    /// # Errors
    /// Returns an error if the pool cannot be created, e.g., due to an invalid
    /// argument, too many block sizes, or an external resource failure (like
    /// failing to create a mutex).
    pub fn new_ex(
        memory: &'static mut [u8],
        use_mutex: bool,
        block_sizes: &[usize],
    ) -> Result<Self> {
        let mut handle = ffi::CFE_ES_MEMHANDLE_UNDEFINED;
        check(unsafe {
            ffi::CFE_ES_PoolCreateEx(
                &mut handle,
                memory.as_mut_ptr() as *mut _,
                memory.len(),
                block_sizes.len() as u16,
                block_sizes.as_ptr(),
                use_mutex,
            )
        })?;
        Ok(Self { handle })
    }

    /// Allocates a buffer of at least `size` bytes from the pool.
    ///
    /// The actual allocation is at least 12 bytes larger than
    /// requested (internal block header overhead). The returned
    /// buffer size is rounded up to the next available block size
    /// in the pool.
    ///
    /// Returns a `PoolBuffer` guard. When this guard is dropped,
    /// the memory is automatically returned to the pool.
    ///
    /// # Errors
    /// Returns an error if a buffer cannot be allocated, for example, if the pool
    /// is out of memory or the requested size is larger than the largest available
    /// block size.
    pub fn get_buf(&self, size: usize) -> Result<PoolBuffer<'_>> {
        let mut buf_ptr = core::ptr::null_mut();
        let actual_size = unsafe { ffi::CFE_ES_GetPoolBuf(&mut buf_ptr, self.handle, size) };
        if actual_size < 0 {
            return Err(CfsError::from(actual_size));
        }

        Ok(PoolBuffer {
            ptr: buf_ptr,
            size: actual_size as usize,
            pool_handle: self.handle,
            _phantom: PhantomData,
        })
    }

    /// Retrieves statistics about this memory pool.
    ///
    /// # Errors
    /// Returns an error if the pool handle is invalid or the underlying CFE
    /// call fails.
    pub fn stats(&self) -> Result<MemPoolStats> {
        let mut stats = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetMemPoolStats(stats.as_mut_ptr(), self.handle) })?;
        Ok(unsafe { stats.assume_init() }.into())
    }

    /// Gets information about a buffer previously allocated from this pool.
    ///
    /// Returns the allocated size of the buffer.
    ///
    /// # Errors
    /// Returns an error if the pool handle is invalid or the provided buffer
    /// pointer does not belong to this pool.
    pub fn get_buf_info(&self, buf: &PoolBuffer) -> Result<usize> {
        let status = unsafe { ffi::CFE_ES_GetPoolBufInfo(self.handle, buf.ptr) };
        if status < 0 {
            Err(CfsError::from(status))
        } else {
            Ok(status as usize)
        }
    }
}

impl Drop for MemPool {
    /// Deletes the memory pool when the `MemPool` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::CFE_ES_PoolDelete(self.handle) };
    }
}

/// An RAII guard for a buffer allocated from a `MemPool`.
///
/// When this struct is dropped, its memory is automatically released back to the
/// originating pool by calling `CFE_ES_PutPoolBuf`. It provides safe `&[u8]` and
/// `&mut [u8]` views into the buffer's memory.
#[derive(Debug)]
#[must_use = "if unused the buffer will be immediately returned to the pool"]
pub struct PoolBuffer<'a> {
    ptr: ffi::CFE_ES_MemPoolBuf_t,
    size: usize,
    pool_handle: ffi::CFE_ES_MemHandle_t,
    _phantom: PhantomData<&'a MemPool>,
}

impl<'a> Deref for PoolBuffer<'a> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.ptr as *const u8, self.size) }
    }
}

impl<'a> DerefMut for PoolBuffer<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.ptr as *mut u8, self.size) }
    }
}

impl<'a> Drop for PoolBuffer<'a> {
    /// Automatically releases the buffer back to the memory pool.
    fn drop(&mut self) {
        let _ = unsafe { ffi::CFE_ES_PutPoolBuf(self.pool_handle, self.ptr) };
    }
}
