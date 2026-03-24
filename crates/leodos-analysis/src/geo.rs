//! Geospatial coordinate transforms and utilities.

use core::f32::consts::PI;

/// Earth's mean radius in meters (WGS-84 mean).
pub const EARTH_RADIUS_M: f32 = 6_371_000.0;

/// Degrees to radians.
pub fn deg2rad(deg: f32) -> f32 {
    deg * PI / 180.0
}

/// Radians to degrees.
pub fn rad2deg(rad: f32) -> f32 {
    rad * 180.0 / PI
}

/// A geographic coordinate.
#[derive(Debug, Copy, Clone)]
pub struct LatLon {
    /// Latitude in degrees (-90 to 90).
    pub lat: f32,
    /// Longitude in degrees (-180 to 180).
    pub lon: f32,
}

impl LatLon {
    /// Creates a new coordinate.
    pub fn new(lat: f32, lon: f32) -> Self {
        Self { lat, lon }
    }
}

/// A geographic bounding box (west, south, east, north).
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct GeoBounds {
    pub west: f32,
    pub south: f32,
    pub east: f32,
    pub north: f32,
}

impl GeoBounds {
    /// Returns `true` if the point lies within the bounds.
    pub fn contains(&self, lat: f32, lon: f32) -> bool {
        lat >= self.south && lat <= self.north && lon >= self.west && lon <= self.east
    }

    /// Maps a pixel coordinate to a geographic coordinate
    /// within this bounding box (linear interpolation).
    pub fn pixel_to_latlon(&self, px: f32, py: f32, width: f32, height: f32) -> LatLon {
        LatLon {
            lat: self.north - py / height * (self.north - self.south),
            lon: self.west + px / width * (self.east - self.west),
        }
    }
}

/// Haversine distance between two points in meters.
pub fn haversine_distance(a: LatLon, b: LatLon) -> f32 {
    let dlat = deg2rad(b.lat - a.lat);
    let dlon = deg2rad(b.lon - a.lon);
    let lat1 = deg2rad(a.lat);
    let lat2 = deg2rad(b.lat);

    let sin_dlat = libm::sinf(dlat / 2.0);
    let sin_dlon = libm::sinf(dlon / 2.0);
    let h = sin_dlat * sin_dlat + libm::cosf(lat1) * libm::cosf(lat2) * sin_dlon * sin_dlon;
    2.0 * EARTH_RADIUS_M * libm::asinf(libm::sqrtf(h))
}

/// Ground sample distance (GSD) in meters per pixel.
///
/// `altitude_m` — satellite altitude in meters.
/// `focal_length_mm` — camera focal length in millimeters.
/// `pixel_pitch_um` — sensor pixel pitch in micrometers.
pub fn ground_sample_distance(altitude_m: f32, focal_length_mm: f32, pixel_pitch_um: f32) -> f32 {
    altitude_m * (pixel_pitch_um * 1e-6) / (focal_length_mm * 1e-3)
}

/// Swath width in meters.
///
/// `altitude_m` — satellite altitude.
/// `fov_deg` — field of view in degrees.
pub fn swath_width(altitude_m: f32, fov_deg: f32) -> f32 {
    2.0 * altitude_m * libm::tanf(deg2rad(fov_deg / 2.0))
}

/// Pixel coordinate to geographic coordinate (nadir approximation).
///
/// Assumes the image center is at `nadir` and pixels are
/// uniformly spaced at `gsd` meters.
pub fn pixel_to_latlon(
    px: f32,
    py: f32,
    center_px: f32,
    center_py: f32,
    nadir: LatLon,
    gsd: f32,
) -> LatLon {
    let dx_m = (px - center_px) * gsd;
    let dy_m = (py - center_py) * gsd;

    let dlat = rad2deg(dy_m / EARTH_RADIUS_M);
    let dlon = rad2deg(dx_m / (EARTH_RADIUS_M * libm::cosf(deg2rad(nadir.lat))));

    LatLon::new(nadir.lat - dlat, nadir.lon + dlon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_same_point() {
        let p = LatLon::new(59.3293, 18.0686);
        let d = haversine_distance(p, p);
        assert!(d < 1.0);
    }

    #[test]
    fn haversine_stockholm_gothenburg() {
        let sthlm = LatLon::new(59.3293, 18.0686);
        let gbg = LatLon::new(57.7089, 11.9746);
        let d = haversine_distance(sthlm, gbg);
        assert!((d - 398_000.0).abs() < 10_000.0);
    }

    #[test]
    fn gsd_calculation() {
        let gsd = ground_sample_distance(500_000.0, 50.0, 5.5);
        assert!((gsd - 55.0).abs() < 1.0);
    }

    #[test]
    fn swath_width_calculation() {
        let sw = swath_width(500_000.0, 10.0);
        assert!(sw > 80_000.0);
        assert!(sw < 90_000.0);
    }

    #[test]
    fn pixel_to_latlon_center() {
        let nadir = LatLon::new(59.0, 18.0);
        let ll = pixel_to_latlon(512.0, 512.0, 512.0, 512.0, nadir, 10.0);
        assert!((ll.lat - 59.0).abs() < 0.001);
        assert!((ll.lon - 18.0).abs() < 0.001);
    }
}
