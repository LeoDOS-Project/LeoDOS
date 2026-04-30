//! Runtime-neutral buffer pool abstraction.
//!
//! Stack-internal allocations (SRSPP windows, router buffers,
//! reassembly state) flow through the [`BufferPool`] trait
//! rather than reaching for `Box::new` or const-generic inline
//! arrays. This gives every call site:
//!
//! - **Fallible OOM**: [`BufferPool::alloc`] returns `Result`, so a
//!   pool exhaustion is a packet drop or stream rejection rather
//!   than a process-level abort. (The unstable [`core::alloc::Allocator`]
//!   trait would be the ideal vehicle here, but until it stabilizes
//!   we roll our own.)
//! - **Bounded memory**: a single shared pool gives the entire
//!   stack one buffer budget rather than worst-case `× neighbors ×
//!   streams` static allocation.
//! - **Backend choice**: flight builds use a cFE `MemPool` for
//!   deterministic allocation; tests/tokio builds use the
//!   [`HeapBufferPool`] backed by `Box<[u8]>` with a configurable
//!   budget for exercising OOM paths.
//!
//! The pool itself takes `&self` (interior mutability), so a single
//! pool can be shared across many concurrent links and streams
//! without borrow conflicts.

use core::alloc::Layout;
use core::ops::DerefMut;

/// Allocates owned byte buffers for protocol stack consumers.
///
/// `Self::Buf` is a handle that derefs to the underlying byte slice
/// and returns the storage to the pool on `Drop`. Producers
/// allocate at ingress; forwarding paths just move `Buf` handles
/// without copying.
pub trait BufferPool {
    /// Owned buffer handle. Releases on `Drop`.
    type Buf<'a>: DerefMut<Target = [u8]>
    where
        Self: 'a;

    /// Reason an allocation failed (e.g. budget exhausted).
    type Error;

    /// Allocate a buffer satisfying `layout`.
    ///
    /// Returns `Err` when the pool cannot satisfy the request. The
    /// caller decides what to do with that — drop a packet, reject
    /// a new stream, etc.
    fn alloc(&self, layout: Layout) -> Result<Self::Buf<'_>, Self::Error>;

    /// Allocate `size` bytes with no alignment requirement.
    ///
    /// Convenience over [`alloc`] for the common case of byte
    /// buffers (MTU-sized packets, framing scratch, reassembly).
    /// `Layout::from_size_align` only fails on non-power-of-2
    /// alignment or `size > isize::MAX`; with `align = 1` and
    /// realistic buffer sizes neither is reachable, so this hides
    /// the `LayoutError` that would otherwise plumb through every
    /// call site.
    fn alloc_bytes(&self, size: usize) -> Result<Self::Buf<'_>, Self::Error> {
        let layout = Layout::from_size_align(size, 1).expect("align=1 is a power of 2");
        self.alloc(layout)
    }
}

#[cfg(feature = "std")]
mod heap;

#[cfg(feature = "std")]
pub use heap::HeapBufferPool;
#[cfg(feature = "std")]
pub use heap::HeapBuf;
#[cfg(feature = "std")]
pub use heap::OutOfMemory;

#[cfg(feature = "cfs")]
mod cfs;
