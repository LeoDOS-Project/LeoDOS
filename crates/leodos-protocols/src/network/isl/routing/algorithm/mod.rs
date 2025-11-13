use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;

pub mod distance_minimizing;
pub mod manhattan;

/// Defines how a node decides which neighbor to forward a packet to.
pub trait RoutingAlgorithm {
    fn route(&self, torus: &Torus, current: Point, target: Point) -> Direction;
}
