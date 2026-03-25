//! Fixed-capacity pool of concurrent futures.
//!
//! All futures have the same concrete type `F` (e.g., from
//! calling the same closure). Up to `N` futures run
//! concurrently. Completed futures free their slot for reuse.
//!
//! ```ignore
//! let mut pool = FuturePool::<MyFut, 4>::new();
//! pin_mut!(pool);
//! pool.as_mut().try_spawn(handler(conn1));
//! pool.as_mut().try_spawn(handler(conn2));
//! pool.as_mut().poll_all(cx);
//! ```

use core::future::Future;
use core::pin::Pin;
use core::task::Context;

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
    /// Returns `Some(fut)` if all slots are occupied.
    /// Writing to an empty slot does not move any existing
    /// pinned future — only the `None` variant is overwritten.
    pub fn try_spawn(self: Pin<&mut Self>, fut: F) -> Option<F> {
        // SAFETY: We only write to a slot that is currently None.
        // No pinned future is moved or accessed.
        let this = unsafe { self.get_unchecked_mut() };
        let Some(slot) = this.slots.iter_mut().find(|s| s.is_none()) else {
            return Some(fut);
        };
        *slot = Some(fut);
        None
    }

    /// Polls all active futures. Completed futures are
    /// removed from their slot.
    pub fn poll_all(self: Pin<&mut Self>, cx: &mut Context<'_>) {
        // SAFETY: self is pinned, so the slots array has a
        // stable address. We never move a future out of a
        // slot — completed futures are dropped in-place by
        // assigning None. Each slot is accessed independently.
        let this = unsafe { self.get_unchecked_mut() };
        for slot in &mut this.slots {
            let Some(ref mut fut) = slot else { continue };
            let pinned = unsafe { Pin::new_unchecked(fut) };
            if pinned.poll(cx).is_ready() {
                *slot = None;
            }
        }
    }
}
