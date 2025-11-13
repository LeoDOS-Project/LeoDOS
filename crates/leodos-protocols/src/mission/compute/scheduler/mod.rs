//! Bipartite matching scheduler for SpaceCoMP MapReduce.
//!
//! This module provides algorithms for assigning map tasks to processor satellites
//! in a LEO constellation. Given N tasks at various locations and N available
//! processors, it finds an assignment that minimizes total cost (e.g., data
//! transfer time, hop count).
//!
//! # Architecture
//!
//! - [`Solver`]: Trait for assignment algorithms (Hungarian, Jonker-Volgenant, etc.)
//! - [`CostModel`]: Trait for computing task-to-processor costs
//! - [`Hungarian`]: O(n³) Kuhn-Munkres algorithm implementation
//! - [`ManhattanCost`]: Simple hop-count based cost model
//!
//! # Example
//!
//! ```ignore
//! use leodos_protocols::mission::compute::scheduler::*;
//! use leodos_protocols::network::isl::torus::{Point, Torus};
//!
//! let torus = Torus::new(8, 8);
//! let cost_model = ManhattanCost::default();
//!
//! // Task locations and processor locations
//! let tasks = [Point::new(0, 0), Point::new(1, 1), Point::new(2, 2)];
//! let processors = [Point::new(0, 1), Point::new(1, 0), Point::new(2, 3)];
//!
//! // Build cost matrix
//! let mut matrix = [[0u32; 4]; 4];
//! for (i, &task) in tasks.iter().enumerate() {
//!     for (j, &proc) in processors.iter().enumerate() {
//!         matrix[i][j] = cost_model.cost(&torus, task, proc);
//!     }
//! }
//!
//! // Solve
//! let mut solver = Hungarian::<u32, 4>::new();
//! let assignment = solver.solve(&matrix, 3);
//!
//! // assignment[task_idx] = processor_idx
//! ```

pub mod aoi;
pub mod cost;
pub mod hungarian;
pub mod job;
pub mod lapjv;
pub mod solver;

pub use aoi::Aoi;
pub use cost::manhattan::ManhattanCost;
pub use cost::spacecomp::SpaceCompCost;
pub use cost::CostModel;
pub use hungarian::Hungarian;
pub use job::{JobCost, JobParams};
pub use lapjv::JonkerVolgenant;
pub use solver::{Bounded, Solver};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::isl::torus::{Point, Torus};

    #[test]
    fn test_full_workflow() {
        let torus = Torus::new(4, 4);
        let cost_model = ManhattanCost { hop_cost: 10 };

        let tasks = [
            Point::new(0, 0),
            Point::new(1, 1),
            Point::new(2, 2),
            Point::new(3, 3),
        ];

        let processors = [
            Point::new(0, 1),
            Point::new(1, 0),
            Point::new(2, 3),
            Point::new(3, 2),
        ];

        let mut matrix = [[0u32; 8]; 8];
        for (i, &task) in tasks.iter().enumerate() {
            for (j, &proc) in processors.iter().enumerate() {
                matrix[i][j] = cost_model.cost(&torus, task, proc);
            }
        }

        let mut solver = Hungarian::<u32, 8>::new();
        let assignment = solver.solve(&matrix, 4);

        assert_eq!(assignment.len(), 4);

        let total: u32 = assignment
            .iter()
            .enumerate()
            .map(|(i, &j)| matrix[i][j])
            .sum();

        assert_eq!(total, 10 * 4);
    }

    #[test]
    fn test_optimal_assignment_diagonal() {
        let torus = Torus::new(4, 4);
        let cost_model = ManhattanCost { hop_cost: 1 };

        let tasks = [Point::new(0, 0), Point::new(1, 1), Point::new(2, 2)];
        let processors = [Point::new(0, 0), Point::new(1, 1), Point::new(2, 2)];

        let mut matrix = [[0u32; 4]; 4];
        for (i, &task) in tasks.iter().enumerate() {
            for (j, &proc) in processors.iter().enumerate() {
                matrix[i][j] = cost_model.cost(&torus, task, proc);
            }
        }

        let mut solver = Hungarian::<u32, 4>::new();
        let assignment = solver.solve(&matrix, 3);

        let total: u32 = assignment
            .iter()
            .enumerate()
            .map(|(i, &j)| matrix[i][j])
            .sum();

        assert_eq!(total, 0);
    }
}
