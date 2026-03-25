//! Solver trait for the linear sum assignment problem.
//!
//! Given an N×N cost matrix where `matrix[i][j]` is the cost of assigning task `i`
//! to processor `j`, find an assignment that minimizes total cost. Each task must
//! be assigned to exactly one processor and vice versa.
//!
//! Different algorithms (Hungarian, Jonker-Volgenant, Auction) can implement this
//! trait, allowing users to swap solvers without changing the rest of their code.

/// Kuhn-Munkres (Hungarian) algorithm.
pub mod hungarian;
/// Jonker-Volgenant (LAPJV) algorithm.
pub mod lapjv;

use heapless::Vec;

/// Solver for the linear sum assignment problem.
///
/// Implementations take an N×N cost matrix and return an assignment where
/// `result[task]` is the processor assigned to that task.
pub trait Solver<C, const N: usize> {
    /// Solves the assignment problem and returns task-to-processor mapping.
    fn solve(&mut self, matrix: &[[C; N]; N], size: usize) -> Vec<usize, N>;
}

/// Trait for types that have a maximum value, used as infinity in algorithms.
pub trait Bounded {
    /// A large sentinel value used as infinity in assignment algorithms.
    const MAX: Self;
}

impl Bounded for u32 {
    const MAX: Self = u32::MAX / 2;
}

impl Bounded for u64 {
    const MAX: Self = u64::MAX / 2;
}

impl Bounded for i32 {
    const MAX: Self = i32::MAX / 2;
}

impl Bounded for i64 {
    const MAX: Self = i64::MAX / 2;
}
