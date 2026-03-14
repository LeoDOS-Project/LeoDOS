//! Manhattan (Pure Topology)

use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::algorithm::gateway::GatewayTable;
use crate::network::isl::shell::Shell;
use crate::network::isl::torus::{Direction, Hop};
use crate::network::isl::torus::Point;

/// Shortest-hop routing using Manhattan distance on the toroidal grid.
pub struct Manhattan<const N: usize> {
    shell: Shell,
    gateway_table: GatewayTable<N>,
}

impl<const N: usize> Manhattan<N> {
    /// Creates a new Manhattan router.
    pub fn new(shell: Shell, gateway_table: GatewayTable<N>) -> Self {
        Self { shell, gateway_table }
    }

    fn route_to_point(
        &self,
        current: Point,
        target: Point,
    ) -> Direction {
        let torus = &self.shell.torus;
        if current.orb != target.orb {
            return torus.direction_to_orb(
                current, target,
            );
        }
        torus.direction_to_sat(current, target)
    }
}

impl<const N: usize> RoutingAlgorithm for Manhattan<N> {
    fn route(
        &self,
        current: Point,
        target: Address,
        time_s: u32,
    ) -> Hop {
        let (target_point, local_hop) = match target {
            Address::Satellite(p) => (p, Hop::Local),
            Address::Ground { station } => {
                let gw = self
                    .gateway_table
                    .gateway(&self.shell, station, time_s)
                    .unwrap_or(Point {
                        orb: 0,
                        sat: station,
                    });
                (gw, Hop::Ground)
            }
            Address::ServiceArea { orb } => (
                Point { orb: current.orb, sat: orb },
                Hop::Local,
            ),
        };
        if current == target_point {
            return local_hop;
        }
        Hop::Isl(self.route_to_point(current, target_point))
    }
}
