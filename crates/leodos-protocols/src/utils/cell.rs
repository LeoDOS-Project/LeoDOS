use core::cell::RefCell;

/// Interior-mutable cell that only allows access through sync closures.
///
/// Wraps `RefCell<T>` but restricts borrows to non-`async` closures,
/// making it a compile error to hold a borrow across an `.await` point.
pub(crate) struct SyncRefCell<T>(RefCell<T>);

impl<T> SyncRefCell<T> {
    pub(crate) fn new(val: T) -> Self {
        Self(RefCell::new(val))
    }

    pub(crate) fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(&self.0.borrow())
    }

    pub(crate) fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        f(&mut self.0.borrow_mut())
    }
}
