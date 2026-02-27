//! Manhattan (Pure Topology)

use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;

/// Shortest-hop routing using Manhattan distance on the toroidal grid.
#[derive(Default, Clone, Copy, Debug)]
pub struct Manhattan;

impl RoutingAlgorithm for Manhattan {
    fn route(&self, torus: &Torus, current: Point, target: Point) -> Direction {
        if current == target {
            return Direction::Local;
        }

        // Prioritize Y direction first
        if current.orb != target.orb {
            return torus.shortest_path_direction_orb(current, target);
        }

        torus.shortest_path_direction_sat(current, target)
    }
}
