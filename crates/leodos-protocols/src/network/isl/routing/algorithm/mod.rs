use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;

/// Physics-aware routing that minimizes physical ISL distance.
pub mod distance_minimizing;
/// Shortest-hop Manhattan routing on the toroidal grid.
pub mod manhattan;

/// Defines how a node decides which neighbor to forward a packet to.
pub trait RoutingAlgorithm {
    /// Returns the next-hop direction for a packet at `current` heading to `target`.
    fn route(&self, torus: &Torus, current: Point, target: Point) -> Direction;
}
