//! The [`SpaceCompJob`] trait — user-defined distributed computation.

use crate::Collector;
use crate::Mapper;
use crate::Reducer;
use crate::Schema;

/// Defines the three phases of a distributed computation.
///
/// Implement this trait to plug your domain logic into
/// the SpaceCoMP framework. The library handles transport,
/// coordination, and phase signaling.
pub trait SpaceCompJob {
    /// Record type flowing from collector to mapper.
    type Collected: Schema;
    /// Record type flowing from mapper to reducer.
    type Mapped: Schema;
    /// Record type in the final result.
    type Result: Schema;

    /// Creates a collector for this job.
    fn collector(&mut self) -> impl Collector<Input = Self::Collected, Output = Self::Collected>;

    /// Creates a mapper for this job.
    fn mapper(&mut self) -> impl Mapper<Input = Self::Collected, Output = Self::Mapped>;

    /// Creates a reducer for this job.
    fn reducer(&mut self) -> impl Reducer<Input = Self::Mapped, Output = Self::Result>;
}
