//! Projects satellite grid positions to geographic coordinates and vice versa.
//!
//! Uses Walker Delta geometry to compute each satellite's nadir (ground-track)
//! position, enabling conversion between geographic AOIs and grid AOIs.

use core::f32::consts::PI;

use heapless::Vec;

use super::geo::{Ecef, GeoAoi, LatLon};
use super::shell::Shell;
use super::torus::Point;
use crate::application::spacecomp::plan::aoi::Aoi;

/// Sidereal day in seconds (Earth's rotation period relative
/// to the stars).
const SIDEREAL_DAY_S: f32 = 86_164.1;

/// Projects satellite grid positions to geographic coordinates and vice versa.
pub struct Projection {
    shell: Shell,
}

impl Projection {
    /// Creates a new projection from a constellation shell.
    pub fn new(shell: Shell) -> Self {
        Self { shell }
    }

    /// Computes the nadir (ground-track) position of a satellite.
    ///
    /// For a Walker Delta constellation:
    /// - RAAN of plane x: `x * 360/N` degrees
    /// - True anomaly of satellite y: `y * 360/M` degrees
    /// - Latitude: `arcsin(sin(i) * sin(ν))`
    /// - Longitude: `Ω + atan2(cos(i) * sin(ν), cos(ν))`
    pub fn nadir(&self, point: Point) -> LatLon {
        let num_planes = self.shell.torus.num_sats as f32;
        let sats_per_plane = self.shell.torus.num_orbs as f32;

        let raan = 2.0 * PI * (point.sat as f32) / num_planes;
        let true_anomaly = 2.0 * PI * (point.orb as f32) / sats_per_plane;

        let sin_i = libm::sinf(self.shell.inclination_rad);
        let cos_i = libm::cosf(self.shell.inclination_rad);
        let sin_v = libm::sinf(true_anomaly);
        let cos_v = libm::cosf(true_anomaly);

        let lat_rad = libm::asinf(sin_i * sin_v);
        let lon_rad = raan + libm::atan2f(cos_i * sin_v, cos_v);

        let mut lon_deg = lon_rad * 180.0 / PI;
        if lon_deg > 180.0 {
            lon_deg -= 360.0;
        } else if lon_deg < -180.0 {
            lon_deg += 360.0;
        }

        LatLon::new(lat_rad * 180.0 / PI, lon_deg)
    }

    /// Returns all satellites whose nadir falls within the geographic AOI.
    pub fn satellites_in_geo_aoi<const N: usize>(&self, geo_aoi: &GeoAoi) -> Vec<Point, N> {
        let mut result = Vec::new();

        for x in 0..self.shell.torus.num_sats {
            for y in 0..self.shell.torus.num_orbs {
                let point = Point::new(y, x);
                let pos = self.nadir(point);
                if geo_aoi.contains(pos) && result.push(point).is_err() {
                    return result;
                }
            }
        }

        result
    }

    /// Computes the ECEF position of a satellite at a given time.
    ///
    /// `time_s` is seconds since epoch (t=0 corresponds to the
    /// constellation snapshot from `nadir()`). Accounts for
    /// both orbital motion and Earth rotation.
    pub fn satellite_ecef(&self, point: Point, time_s: f32) -> Ecef {
        let num_planes = self.shell.torus.num_sats as f32;
        let sats_per_plane = self.shell.torus.num_orbs as f32;

        let raan = 2.0 * PI * (point.sat as f32) / num_planes;
        let base_anomaly = 2.0 * PI * (point.orb as f32) / sats_per_plane;
        let mean_motion = 2.0 * PI / self.shell.orbital_period_s();
        let true_anomaly = base_anomaly + mean_motion * time_s;

        let r = self.shell.orbital_radius();
        let cos_v = libm::cosf(true_anomaly);
        let sin_v = libm::sinf(true_anomaly);
        let cos_i = libm::cosf(self.shell.inclination_rad);
        let sin_i = libm::sinf(self.shell.inclination_rad);
        let cos_raan = libm::cosf(raan);
        let sin_raan = libm::sinf(raan);

        // ECI coordinates (perifocal → inertial)
        let x_eci = r * (cos_raan * cos_v - sin_raan * cos_i * sin_v);
        let y_eci = r * (sin_raan * cos_v + cos_raan * cos_i * sin_v);
        let z_eci = r * sin_i * sin_v;

        // ECI → ECEF (rotate by Earth's sidereal angle)
        let earth_angle = 2.0 * PI * time_s / SIDEREAL_DAY_S;
        let cos_e = libm::cosf(earth_angle);
        let sin_e = libm::sinf(earth_angle);

        Ecef {
            x: cos_e * x_eci + sin_e * y_eci,
            y: -sin_e * x_eci + cos_e * y_eci,
            z: z_eci,
        }
    }

    /// Finds the satellite with the highest elevation angle
    /// above `min_elevation_deg` from a ground station.
    ///
    /// Returns `None` if no satellite has line-of-sight.
    pub fn find_gateway(
        &self,
        station: LatLon,
        time_s: f32,
        min_elevation_deg: f32,
    ) -> Option<Point> {
        let mut best: Option<(Point, f32)> = None;

        for s in 0..self.shell.torus.num_sats {
            for o in 0..self.shell.torus.num_orbs {
                let p = Point::new(o, s);
                let sat_pos = self.satellite_ecef(p, time_s);
                let elev = sat_pos.elevation_from(station);
                if elev > min_elevation_deg {
                    if best.map_or(true, |(_, best_el)| elev > best_el) {
                        best = Some((p, elev));
                    }
                }
            }
        }

        best.map(|(p, _)| p)
    }

    /// Converts a geographic AOI to a grid AOI by finding the
    /// bounding box of all satellites whose nadir falls within
    /// the geographic region.
    pub fn geo_to_grid_aoi(&self, geo_aoi: &GeoAoi) -> Option<Aoi> {
        let mut min_y = u8::MAX;
        let mut max_y = u8::MIN;
        let mut min_x = u8::MAX;
        let mut max_x = u8::MIN;
        let mut found = false;

        for x in 0..self.shell.torus.num_sats {
            for y in 0..self.shell.torus.num_orbs {
                let point = Point::new(y, x);
                if geo_aoi.contains(self.nadir(point)) {
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    found = true;
                }
            }
        }

        if !found {
            return None;
        }
        Some(Aoi::new(Point::new(min_y, min_x), Point::new(max_y, max_x)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::isl::torus::Torus;

    #[test]
    fn test_nadir_equator_first_plane() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let pos = proj.nadir(Point::new(0, 0));
        assert!((pos.lat_deg).abs() < 0.1, "lat={}", pos.lat_deg);
        assert!((pos.lon_deg).abs() < 0.1, "lon={}", pos.lon_deg);
    }

    #[test]
    fn test_nadir_near_pole() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        // y=5 out of 20 → true anomaly = 90° → near max latitude
        let pos = proj.nadir(Point::new(5, 0));
        assert!(pos.lat_deg > 80.0, "lat={}", pos.lat_deg);
    }

    #[test]
    fn test_satellites_in_geo_aoi() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let geo_aoi = GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0));

        let covering: Vec<Point, 64> = proj.satellites_in_geo_aoi(&geo_aoi);
        assert!(!covering.is_empty(), "should find satellites near equator");
    }

    #[test]
    fn test_geo_to_grid_aoi() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let geo_aoi = GeoAoi::new(LatLon::new(10.0, -5.0), LatLon::new(-10.0, 5.0));

        let grid_aoi = proj.geo_to_grid_aoi(&geo_aoi);
        assert!(grid_aoi.is_some());
    }

    #[test]
    fn test_satellite_ecef_at_t0_matches_nadir() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let p = Point::new(0, 0);
        let ecef = proj.satellite_ecef(p, 0.0);
        let nadir = proj.nadir(p);
        let nadir_ecef = Ecef::from_latlon(nadir);

        // Satellite should be directly above its nadir
        // (same direction, larger magnitude)
        let sat_r = libm::sqrtf(ecef.x * ecef.x + ecef.y * ecef.y + ecef.z * ecef.z);
        let nadir_r = libm::sqrtf(
            nadir_ecef.x * nadir_ecef.x + nadir_ecef.y * nadir_ecef.y + nadir_ecef.z * nadir_ecef.z,
        );
        assert!(
            sat_r > nadir_r,
            "sat_r={} should exceed nadir_r={}",
            sat_r,
            nadir_r
        );

        // Direction vectors should be similar (dot product
        // close to 1)
        let dot = (ecef.x * nadir_ecef.x + ecef.y * nadir_ecef.y + ecef.z * nadir_ecef.z)
            / (sat_r * nadir_r);
        assert!(dot > 0.99, "dot={} — satellite not above nadir", dot);
    }

    #[test]
    fn test_satellite_moves_with_time() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let p = Point::new(0, 0);
        let pos0 = proj.satellite_ecef(p, 0.0);
        let pos1 = proj.satellite_ecef(p, 600.0); // 10 min

        let dist = pos0.distance(&pos1);
        assert!(
            dist > 100_000.0,
            "satellite should move significantly in 10 min, dist={}",
            dist
        );
    }

    #[test]
    fn test_elevation_from_directly_below() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let p = Point::new(0, 0);
        let sat = proj.satellite_ecef(p, 0.0);
        let nadir = proj.nadir(p);

        let elev = sat.elevation_from(nadir);
        assert!(
            elev > 85.0,
            "elevation from nadir should be ~90°, got {}",
            elev
        );
    }

    #[test]
    fn test_find_gateway_at_t0() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        // Ground station at the equator / prime meridian —
        // should have a satellite overhead at t=0
        let station = LatLon::new(0.0, 0.0);
        let gw = proj.find_gateway(station, 0.0, 5.0);
        assert!(gw.is_some(), "should find a gateway for equatorial station");
    }

    #[test]
    fn test_find_gateway_changes_with_time() {
        let torus = Torus::new(20, 72);
        let shell = Shell::new(torus, 550_000.0, 87.0);
        let proj = Projection::new(shell);

        let station = LatLon::new(0.0, 0.0);
        let gw0 = proj.find_gateway(station, 0.0, 5.0);
        // Half an orbital period later (~47 min), the
        // best satellite should be different
        let half_period = shell.orbital_period_s() / 2.0;
        let gw1 = proj.find_gateway(station, half_period, 5.0);

        assert!(gw0 != gw1, "gateway should change: {:?} vs {:?}", gw0, gw1);
    }
}
