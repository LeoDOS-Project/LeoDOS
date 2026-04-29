//! `Box<[u8]>`-backed pool for tests and the tokio runtime.
//!
//! Tracks a byte budget atomically so OOM paths in pool consumers
//! are exercisable without flooding the system allocator. Layout
//! alignment up to the system allocator's natural alignment is
//! satisfied; higher alignments are unsupported (use a flight-grade
//! pool if you need them).

use core::alloc::Layout;
use core::ops::Deref;
use core::ops::DerefMut;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use crate::buffer_pool::BufferPool;

/// Returned when the pool's byte budget cannot satisfy an allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutOfMemory;

impl core::fmt::Display for OutOfMemory {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("buffer pool exhausted")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OutOfMemory {}

/// Heap-backed buffer pool with a fixed byte budget.
///
/// Allocations beyond the budget return [`OutOfMemory`]. The budget
/// is global to the pool — sharing one pool across many links is
/// the intended usage, since that's what gives a unified memory
/// envelope for the whole stack.
pub struct HeapBufferPool {
    budget: usize,
    in_use: AtomicUsize,
}

impl HeapBufferPool {
    /// Create a pool with `budget` bytes available across all
    /// outstanding allocations.
    pub const fn new(budget: usize) -> Self {
        Self {
            budget,
            in_use: AtomicUsize::new(0),
        }
    }

    /// Bytes currently allocated from this pool.
    pub fn bytes_in_use(&self) -> usize {
        self.in_use.load(Ordering::Acquire)
    }

    /// Total budget of the pool.
    pub fn budget(&self) -> usize {
        self.budget
    }
}

impl BufferPool for HeapBufferPool {
    type Buf<'a> = HeapBuf<'a>;
    type Error = OutOfMemory;

    fn alloc(&self, layout: Layout) -> Result<HeapBuf<'_>, OutOfMemory> {
        let size = layout.size();
        // Reserve budget first so concurrent callers can't both
        // succeed past the limit.
        let prev = self.in_use.fetch_add(size, Ordering::AcqRel);
        if prev + size > self.budget {
            self.in_use.fetch_sub(size, Ordering::AcqRel);
            return Err(OutOfMemory);
        }
        let buf = vec![0u8; size].into_boxed_slice();
        Ok(HeapBuf { buf, pool: self })
    }
}

/// Owned buffer handle from a [`HeapBufferPool`]. Drops the
/// underlying `Box<[u8]>` and credits the pool budget on `Drop`.
pub struct HeapBuf<'a> {
    buf: Box<[u8]>,
    pool: &'a HeapBufferPool,
}

impl Deref for HeapBuf<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.buf
    }
}

impl DerefMut for HeapBuf<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}

impl Drop for HeapBuf<'_> {
    fn drop(&mut self) {
        self.pool.in_use.fetch_sub(self.buf.len(), Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(n: usize) -> Layout {
        Layout::from_size_align(n, 1).unwrap()
    }

    #[test]
    fn alloc_within_budget() {
        let pool = HeapBufferPool::new(64);
        let mut buf = pool.alloc(layout(16)).unwrap();
        assert_eq!(buf.len(), 16);
        buf[0] = 42;
        assert_eq!(buf[0], 42);
        assert_eq!(pool.bytes_in_use(), 16);
    }

    #[test]
    fn drop_returns_budget() {
        let pool = HeapBufferPool::new(64);
        {
            let _buf = pool.alloc(layout(40)).unwrap();
            assert_eq!(pool.bytes_in_use(), 40);
        }
        assert_eq!(pool.bytes_in_use(), 0);
    }

    #[test]
    fn exceeds_budget_returns_oom() {
        let pool = HeapBufferPool::new(32);
        let _a = pool.alloc(layout(20)).unwrap();
        assert!(matches!(pool.alloc(layout(20)), Err(OutOfMemory)));
        assert_eq!(pool.bytes_in_use(), 20);
    }

    #[test]
    fn reclaim_after_oom() {
        let pool = HeapBufferPool::new(32);
        let a = pool.alloc(layout(20)).unwrap();
        assert!(matches!(pool.alloc(layout(20)), Err(OutOfMemory)));
        drop(a);
        let _b = pool.alloc(layout(20)).unwrap();
        assert_eq!(pool.bytes_in_use(), 20);
    }

    #[test]
    fn many_small_allocations() {
        let pool = HeapBufferPool::new(1024);
        let mut bufs = Vec::new();
        for _ in 0..100 {
            bufs.push(pool.alloc(layout(8)).unwrap());
        }
        assert_eq!(pool.bytes_in_use(), 800);
        bufs.clear();
        assert_eq!(pool.bytes_in_use(), 0);
    }
}
