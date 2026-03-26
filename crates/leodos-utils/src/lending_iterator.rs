//! A lending iterator whose items can borrow from the
//! iterator itself.
//!
//! Standard `Iterator` requires items to be independent of
//! the iterator. `LendingIterator` uses GATs to tie the
//! item lifetime to each `next()` call, enabling zero-copy
//! iteration over reused buffers.
//!
//! ```ignore
//! while let Some(item) = iter.next() {
//!     process(item);
//! } // item dropped before next call
//! ```

/// An iterator that lends items borrowing from `self`.
pub trait LendingIterator {
    /// The item type, parameterized by the borrow lifetime.
    type Item<'a> where Self: 'a;

    /// Returns the next item, or `None` if exhausted.
    fn next(&mut self) -> Option<Self::Item<'_>>;
}
