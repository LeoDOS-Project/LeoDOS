//! Distance Minimizing (Physics Aware) Routing Strategy

use core::f32::consts::PI;

use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;

/// Optimization strategy that considers orbital mechanics.
/// It holds the physical configuration required for its calculations.
#[derive(Clone, Copy, Debug)]
pub struct DistanceMinimizing {
    inclination_rad: f32,
}

impl DistanceMinimizing {
    pub fn new(inclination_rad: f32) -> Self {
        Self { inclination_rad }
    }

    /// Helper: Computes the relative distance between two rows at a given column x.
    ///
    /// distance_between_rows(inclination, x, num_cols) =
    ///    sqrt(cos²(Θ) + cos²(inclination) * sin²(Θ))
    /// where
    //     Θ = 2π * x / num_cols
    fn distance_between_rows_at_col(&self, x: u8, num_cols: u8) -> f32 {
        let theta = 2.0 * PI * (x as f32) / (num_cols as f32);
        let cos_theta = libm::cosf(theta);
        let sin_theta = libm::sinf(theta);
        let cos_inc = libm::cosf(self.inclination_rad);
        libm::sqrtf(cos_theta * cos_theta + cos_inc * cos_inc * sin_theta * sin_theta)
    }
}

impl RoutingAlgorithm for DistanceMinimizing {
    fn route(&self, torus: &Torus, current: Point, target: Point) -> Direction {
        if current == target {
            return Direction::Local;
        }

        if current.y == target.y {
            return torus.shortest_path_direction_x(current, target);
        }

        if current.x == target.x {
            return torus.shortest_path_direction_y(current, target);
        }

        let h_dir = torus.shortest_path_direction_x(current, target);
        let (prev_x, next_x) = torus.adjacent_x(current, h_dir);

        let curr_row_dist = self.distance_between_rows_at_col(current.x, torus.num_cols);
        let next_row_dist = self.distance_between_rows_at_col(next_x, torus.num_cols);
        let prev_row_dist = self.distance_between_rows_at_col(prev_x, torus.num_cols);

        let is_minimal = prev_row_dist > curr_row_dist && next_row_dist > curr_row_dist;
        let is_shrinking = next_row_dist < curr_row_dist;

        if is_minimal || is_shrinking {
            h_dir
        } else {
            torus.shortest_path_direction_y(current, target)
        }
    }
}
