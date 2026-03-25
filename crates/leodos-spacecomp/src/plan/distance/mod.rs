//! Cost models for task-to-processor assignment.
//!
//! A cost model computes the cost of assigning a task at one location to a
//! processor at another location on the satellite mesh. Different models
//! can account for hop count, physical distance, transmission time, etc.

pub mod manhattan;
pub mod spacecomp;

use leodos_protocols::network::isl::torus::{Point, Torus};
use super::assignment::Bounded;

/// Computes the cost of assigning a task to a processor.
pub trait CostModel {
    /// The numeric cost type (must support arithmetic and comparison).
    type Cost: Ord
        + Copy
        + Default
        + core::ops::Add<Output = Self::Cost>
        + core::ops::Sub<Output = Self::Cost>
        + Bounded;

    /// Computes the cost of assigning a task at one point to a processor at another.
    fn cost(&self, torus: &Torus, task: Point, processor: Point) -> Self::Cost;
}
