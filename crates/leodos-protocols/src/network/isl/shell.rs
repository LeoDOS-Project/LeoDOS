//! Walker Delta constellation shell with physical ISL distances.
//!
//! A shell consists of orbital planes at the same altitude and inclination.
//! This module extends the logical Torus topology with physical parameters
//! to compute actual ISL distances in meters.
//!
//! Within-plane distances are constant. Cross-plane distances vary cyclically
//! as planes converge near poles and diverge at the equator.

use core::f32::consts::PI;

use super::torus::Torus;

const EARTH_RADIUS_M: f32 = 6_371_000.0;
const GRAVITATIONAL_PARAM: f32 = 3.986e14;

fn chord_length(radius: f32, angle_rad: f32) -> f32 {
    radius * libm::sqrtf(2.0 * (1.0 - libm::cosf(angle_rad)))
}

/// A Walker Delta constellation shell with physical ISL parameters.
#[derive(Debug, Clone, Copy)]
pub struct Shell {
    /// The logical toroidal grid topology.
    pub torus: Torus,
    /// Orbital altitude above Earth's surface in meters.
    pub altitude_m: f32,
    /// Orbital inclination in radians.
    pub inclination_rad: f32,
}

impl Shell {
    /// Creates a new shell from topology, altitude, and inclination (in degrees).
    pub fn new(torus: Torus, altitude_m: f32, inclination_deg: f32) -> Self {
        Self {
            torus,
            altitude_m,
            inclination_rad: inclination_deg * PI / 180.0,
        }
    }

    /// Returns the orbital radius (Earth radius + altitude) in meters.
    pub fn orbital_radius(&self) -> f32 {
        EARTH_RADIUS_M + self.altitude_m
    }

    /// Returns the orbital period in seconds (Kepler's third law).
    pub fn orbital_period_s(&self) -> f32 {
        let r = self.orbital_radius();
        2.0 * PI * libm::sqrtf(r * r * r / GRAVITATIONAL_PARAM)
    }

    fn satellite_spacing_angle(&self) -> f32 {
        2.0 * PI / self.torus.num_orbs as f32
    }

    fn plane_spacing_angle(&self) -> f32 {
        2.0 * PI / self.torus.num_sats as f32
    }

    /// Returns the chord distance between adjacent satellites in the same plane.
    pub fn within_plane_distance(&self) -> f32 {
        chord_length(self.orbital_radius(), self.satellite_spacing_angle())
    }

    /// Returns the equatorial chord distance between adjacent orbital planes.
    pub fn cross_plane_base_distance(&self) -> f32 {
        chord_length(self.orbital_radius(), self.plane_spacing_angle())
    }

    /// Returns the cross-plane ISL distance at a given orbital phase (0.0-1.0).
    pub fn cross_plane_distance(&self, phase: f32) -> f32 {
        let theta = 2.0 * PI * phase;
        let cos_theta = libm::cosf(theta);
        let sin_theta = libm::sinf(theta);
        let cos_inc = libm::cosf(self.inclination_rad);

        let factor = libm::sqrtf(cos_theta * cos_theta + cos_inc * cos_inc * sin_theta * sin_theta);
        self.cross_plane_base_distance() * factor
    }

    /// Returns the cross-plane ISL distance for a satellite at the given row.
    pub fn cross_plane_distance_at_row(&self, row: u8) -> f32 {
        let phase = row as f32 / self.torus.num_orbs as f32;
        self.cross_plane_distance(phase)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_orbital_period() {
        let torus = Torus::new(22, 72);
        let shell = Shell::new(torus, 550_000.0, 53.0);
        let period = shell.orbital_period_s();
        assert!(approx_eq(period, 5700.0, 100.0), "period={}", period);
    }

    #[test]
    fn test_within_plane_distance() {
        let torus = Torus::new(22, 72);
        let shell = Shell::new(torus, 550_000.0, 53.0);
        let dist = shell.within_plane_distance();
        assert!(dist > 1_000_000.0 && dist < 2_500_000.0, "dist={}", dist);
    }

    #[test]
    fn test_cross_plane_varies() {
        let torus = Torus::new(22, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let at_equator = shell.cross_plane_distance(0.0);
        let at_pole = shell.cross_plane_distance(0.25);
        assert!(at_pole < at_equator, "pole {} < equator {}", at_pole, at_equator);
    }

    #[test]
    fn test_high_inclination_more_variation() {
        let torus = Torus::new(22, 72);
        let low = Shell::new(torus, 550_000.0, 53.0);
        let high = Shell::new(torus, 550_000.0, 87.0);

        let low_ratio = low.cross_plane_distance(0.0) / low.cross_plane_distance(0.25);
        let high_ratio = high.cross_plane_distance(0.0) / high.cross_plane_distance(0.25);

        assert!(high_ratio > low_ratio, "high {} > low {}", high_ratio, low_ratio);
    }
}
