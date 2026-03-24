//! Configuration for a SpaceCoMP node.

use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::Torus;
use leodos_protocols::network::spp::Apid;

/// Configuration for a SpaceCoMP node.
pub struct SpaceCompConfig {
    /// Number of orbital planes.
    pub num_orbits: u8,
    /// Number of satellites per orbit.
    pub num_sats: u8,
    /// Orbital altitude in meters.
    pub altitude_m: f32,
    /// Orbital inclination in degrees.
    pub inclination_deg: f32,
    /// APID for SpaceCoMP messages.
    pub apid: Apid,
    /// Retransmission timeout in milliseconds.
    pub rto_ms: u32,
    /// cFS topic ID for sending to the router.
    pub router_send_topic: u16,
    /// cFS topic ID for receiving from the router.
    pub router_recv_topic: u16,
}

impl SpaceCompConfig {
    /// Returns the constellation torus geometry.
    pub fn torus(&self) -> Torus {
        Torus::new(self.num_orbits, self.num_sats)
    }

    /// Returns the orbital shell (torus + altitude + inclination).
    pub fn shell(&self) -> Shell {
        Shell::new(self.torus(), self.altitude_m, self.inclination_deg)
    }
}
