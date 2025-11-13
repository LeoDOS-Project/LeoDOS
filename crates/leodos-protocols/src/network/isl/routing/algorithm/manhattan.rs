//! Manhattan (Pure Topology)

use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;

#[derive(Default, Clone, Copy, Debug)]
pub struct Manhattan;

impl RoutingAlgorithm for Manhattan {
    fn route(&self, torus: &Torus, current: Point, target: Point) -> Direction {
        if current == target {
            return Direction::Local;
        }

        // Prioritize Y direction first
        if current.y != target.y {
            return torus.shortest_path_direction_y(current, target);
        }

        torus.shortest_path_direction_x(current, target)
    }
}
