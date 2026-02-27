use core::future::Future;

use crate::application::spacecomp::schema::Schema;

/// An asynchronous data sink that accepts schema-typed values.
pub trait Sink {
    /// The schema type consumed by this sink.
    type Input: Schema;
    /// The error type returned on write failure.
    type Error;

    /// Writes a single value to the sink.
    fn write(&mut self, val: &Self::Input) -> impl Future<Output = Result<(), Self::Error>>;
}
