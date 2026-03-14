//! Distance Minimizing (Physics Aware) Routing Strategy

use core::f32::consts::PI;

use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::algorithm::gateway::GatewayTable;
use crate::network::isl::shell::Shell;
use crate::network::isl::torus::{Direction, Hop};
use crate::network::isl::torus::Point;

/// Physics-aware routing that considers orbital mechanics to
/// minimize ISL distance. Prefers cross-plane hops when the
/// satellite is near the poles (where planes converge).
pub struct DistanceMinimizing<const N: usize> {
    shell: Shell,
    gateway_table: GatewayTable<N>,
}

impl<const N: usize> DistanceMinimizing<N> {
    /// Creates a new distance-minimizing router.
    pub fn new(
        shell: Shell,
        gateway_table: GatewayTable<N>,
    ) -> Self {
        Self { shell, gateway_table }
    }

    /// Relative cross-plane distance factor at a given
    /// effective anomaly angle.
    ///
    /// Minimum at the poles, maximum at the equator.
    fn cross_plane_factor(&self, anomaly: f32) -> f32 {
        let cos_theta = libm::cosf(anomaly);
        let sin_theta = libm::sinf(anomaly);
        let cos_inc = libm::cosf(self.shell.inclination_rad);
        libm::sqrtf(
            cos_theta * cos_theta
                + cos_inc * cos_inc * sin_theta * sin_theta,
        )
    }

    /// Computes the effective anomaly for a satellite at grid
    /// position `orb`, accounting for orbital motion over time.
    fn effective_anomaly(
        &self,
        orb: u8,
        time_s: u32,
    ) -> f32 {
        let num_orbs = self.shell.torus.num_orbs as f32;
        let base = 2.0 * PI * (orb as f32) / num_orbs;
        base + 2.0 * PI * time_s as f32
            / self.shell.orbital_period_s()
    }

    fn route_to_point(
        &self,
        current: Point,
        target: Point,
        time_s: u32,
    ) -> Direction {
        let torus = &self.shell.torus;

        if current.orb == target.orb {
            return torus.direction_to_sat(
                current, target,
            );
        }

        if current.sat == target.sat {
            return torus.direction_to_orb(
                current, target,
            );
        }

        let v_dir =
            torus.direction_to_orb(current, target);
        let toward_orb = match v_dir {
            Direction::East => torus.next_orb(current),
            _ => torus.prev_orb(current),
        };

        let curr_anomaly =
            self.effective_anomaly(current.orb, time_s);
        let toward_anomaly =
            self.effective_anomaly(toward_orb, time_s);

        let curr_factor = self.cross_plane_factor(curr_anomaly);
        let toward_factor =
            self.cross_plane_factor(toward_anomaly);

        if toward_factor < curr_factor {
            v_dir
        } else {
            torus.direction_to_sat(current, target)
        }
    }
}

impl<const N: usize> RoutingAlgorithm for DistanceMinimizing<N> {
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
        Hop::Isl(self.route_to_point(current, target_point, time_s))
    }
}
