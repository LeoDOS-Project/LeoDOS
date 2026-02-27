use core::future::Future;

use crate::application::spacecomp::schema::Schema;

/// An asynchronous data source that yields schema-typed values.
pub trait Source {
    /// The schema type produced by this source.
    type Output: Schema;
    /// The error type returned on read failure.
    type Error: core::error::Error;

    /// Reads the next value, returning `None` when the source is exhausted.
    fn read(&mut self) -> impl Future<Output = Option<Result<Self::Output, Self::Error>>>;
}
