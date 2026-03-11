//! Manhattan (Pure Topology)

use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::algorithm::gateway::GatewayTable;
use crate::network::isl::shell::Shell;
use crate::network::isl::torus::Direction;
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
        if current == target {
            return Direction::Local;
        }
        let torus = &self.shell.torus;
        if current.orb != target.orb {
            return torus.shortest_path_direction_orb(
                current, target,
            );
        }
        torus.shortest_path_direction_sat(current, target)
    }
}

impl<const N: usize> RoutingAlgorithm for Manhattan<N> {
    fn route(
        &self,
        current: Point,
        target: Address,
        time_s: u32,
    ) -> Direction {
        let (target_point, local_dir) = match target {
            Address::Satellite(p) => (p, Direction::Local),
            Address::Ground { station } => {
                let gw = self
                    .gateway_table
                    .gateway(&self.shell, station, time_s)
                    .unwrap_or(Point {
                        orb: 0,
                        sat: station,
                    });
                (gw, Direction::Ground)
            }
            Address::ServiceArea { orb } => (
                Point { orb: current.orb, sat: orb },
                Direction::Local,
            ),
        };
        if current == target_point {
            return local_dir;
        }
        self.route_to_point(current, target_point)
    }
}
