//! Fixed-capacity pool of concurrent futures.
//!
//! All futures have the same concrete type `F` (e.g., from
//! calling the same closure). Up to `N` futures run
//! concurrently. Completed futures free their slot for reuse.
//!
//! ```ignore
//! let mut pool = FuturePool::<MyFut, 4>::new();
//! pool.try_spawn(handler(conn1));
//! pool.try_spawn(handler(conn2));
//! // poll from an outer poll_fn or Future impl
//! pool.poll_all(cx);
//! ```

use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

/// A fixed-capacity pool of concurrent futures of type `F`.
pub struct FuturePool<F, const N: usize> {
    slots: [Option<F>; N],
}

impl<F, const N: usize> FuturePool<F, N> {
    /// Creates an empty pool.
    pub fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| None),
        }
    }

    /// Returns `true` if there is a free slot.
    pub fn has_free_slot(&self) -> bool {
        self.slots.iter().any(|s| s.is_none())
    }

    /// Returns the number of active futures.
    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

impl<F: Future, const N: usize> FuturePool<F, N> {
    /// Spawns a future into a free slot.
    ///
    /// Returns `false` if all slots are occupied.
    pub fn try_spawn(&mut self, fut: F) -> bool {
        let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) else {
            return false;
        };
        *slot = Some(fut);
        true
    }

    /// Polls all active futures. Completed futures are
    /// removed from their slot.
    ///
    /// Must only be called when `self` is pinned (i.e.,
    /// from within a pinned `Future::poll` or `poll_fn`
    /// that captures `&mut self`). The pool must not be
    /// moved after the first call to `poll_all`.
    pub fn poll_all(&mut self, cx: &mut Context<'_>) {
        for slot in &mut self.slots {
            let Some(ref mut fut) = slot else { continue };
            // SAFETY: The FuturePool is intended to be used inside
            // a pinned context (async fn, poll_fn, or a struct that
            // implements Future). Once the containing future is
            // pinned, this array won't move. Futures are never moved
            // out of slots — only dropped in-place via `*slot = None`.
            let pinned = unsafe { Pin::new_unchecked(fut) };
            if pinned.poll(cx).is_ready() {
                *slot = None;
            }
        }
    }
}
