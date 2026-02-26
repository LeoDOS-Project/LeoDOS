//! Projects satellite grid positions to geographic coordinates and vice versa.
//!
//! Uses Walker Delta geometry to compute each satellite's nadir (ground-track)
//! position, enabling conversion between geographic AOIs and grid AOIs.

use core::f32::consts::PI;

use heapless::Vec;

use super::geo::{GeoAoi, LatLon};
use super::shell::Shell;
use super::torus::Point;
use crate::mission::spacecomp::scheduler::aoi::Aoi;

pub struct Projection {
    shell: Shell,
}

impl Projection {
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

    /// Converts a geographic AOI to a grid AOI by finding the bounding box
    /// of all satellites whose nadir falls within the geographic region.
    pub fn geo_to_grid_aoi(&self, geo_aoi: &GeoAoi) -> Option<Aoi> {
        let covering: Vec<Point, 256> = self.satellites_in_geo_aoi(geo_aoi);

        if covering.is_empty() {
            return None;
        }

        let mut min_y = u8::MAX;
        let mut max_y = u8::MIN;
        let mut min_x = u8::MAX;
        let mut max_x = u8::MIN;

        for &p in &covering {
            min_y = min_y.min(p.orb);
            max_y = max_y.max(p.orb);
            min_x = min_x.min(p.sat);
            max_x = max_x.max(p.sat);
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
}
