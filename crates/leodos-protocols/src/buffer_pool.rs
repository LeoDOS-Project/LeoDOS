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
