//! Single-task cell for static storage.
//!
//! [`TaskLocalCell`] wraps a value in an [`UnsafeCell`] with a
//! `Sync` impl, allowing it to be used as a `static`. This is
//! sound in cFS apps where each app runs as exactly one task.

use core::cell::UnsafeCell;

/// A cell for static storage in single-task cFS apps.
///
/// Provides `&mut T` access without runtime overhead. Only
/// sound when the static is accessed by a single cFS task.
pub struct TaskLocalCell<T>(UnsafeCell<T>);

// SAFETY: cFS apps run as a single task — no concurrent access.
unsafe impl<T> Sync for TaskLocalCell<T> {}

impl<T> TaskLocalCell<T> {
    /// Creates a new cell with the given value.
    pub const fn new(val: T) -> Self {
        Self(UnsafeCell::new(val))
    }

    /// Returns a mutable reference to the inner value.
    ///
    /// # Safety contract
    ///
    /// The caller must ensure that only one cFS task accesses
    /// this cell. This is guaranteed by default for any static
    /// inside a single cFS app's `.so`.
    pub fn get_mut(&self) -> &mut T {
        // SAFETY: single cFS task, no concurrent access.
        unsafe { &mut *self.0.get() }
    }
}
