use crate::network::isl::address::Address;
use crate::network::isl::torus::Hop;
use crate::network::isl::torus::Point;

/// Physics-aware routing that minimizes physical ISL distance.
pub mod distance_minimizing;
/// Ground station gateway resolution with LOS calculation.
pub mod gateway;
/// Shortest-hop Manhattan routing on the toroidal grid.
pub mod manhattan;

/// Decides the next hop for a packet.
pub trait RoutingAlgorithm {
    /// Returns the next hop from `current` toward `target`
    /// at the given simulation time.
    fn route(
        &self,
        current: Point,
        target: Address,
        time_s: u32,
    ) -> Hop;
}
