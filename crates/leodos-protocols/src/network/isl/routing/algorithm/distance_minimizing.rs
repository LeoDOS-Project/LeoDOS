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

    /// Relative cross-plane distance factor at a given orbital position (row).
    ///
    /// cross_plane_factor(inclination, y, num_rows) =
    ///    sqrt(cos²(Θ) + cos²(inclination) * sin²(Θ))
    /// where
    ///    Θ = 2π * y / num_rows
    ///
    /// Minimum at the poles (Θ = π/2), maximum at the equator (Θ = 0).
    fn cross_plane_factor(&self, y: u8, num_rows: u8) -> f32 {
        let theta = 2.0 * PI * (y as f32) / (num_rows as f32);
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

        if current.orb == target.orb {
            return torus.shortest_path_direction_sat(current, target);
        }

        if current.sat == target.sat {
            return torus.shortest_path_direction_orb(current, target);
        }

        let v_dir = torus.shortest_path_direction_orb(current, target);
        let toward_orb = match v_dir {
            Direction::East => torus.next_orb(current),
            _ => torus.prev_orb(current),
        };

        let curr_factor = self.cross_plane_factor(current.orb, torus.num_orbs);
        let toward_factor = self.cross_plane_factor(toward_orb, torus.num_orbs);

        if toward_factor < curr_factor {
            v_dir
        } else {
            torus.shortest_path_direction_sat(current, target)
        }
    }
}
