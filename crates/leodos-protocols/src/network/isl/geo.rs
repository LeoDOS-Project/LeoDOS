//! Geographic coordinate conversions for AOI mapping.
//!
//! Converts between geographic coordinates (latitude/longitude) and
//! Earth-Centered Earth-Fixed (ECEF) Cartesian coordinates.
//!
//! # Equations (from SpaceCoMP paper, Equations 4-6)
//!
//! ```text
//! x = R × cos(φ) × cos(Λ)
//! y = R × cos(φ) × sin(Λ)
//! z = R × sin(φ)
//! ```
//!
//! Where R is Earth's radius, φ is latitude, Λ is longitude.

use core::f32::consts::PI;

const EARTH_RADIUS_M: f32 = 6_371_000.0;

/// A geographic coordinate in degrees of latitude and longitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatLon {
    /// Latitude in degrees (-90 to 90).
    pub lat_deg: f32,
    /// Longitude in degrees (-180 to 180).
    pub lon_deg: f32,
}

impl LatLon {
    /// Creates a new coordinate from latitude and longitude in degrees.
    pub fn new(lat_deg: f32, lon_deg: f32) -> Self {
        Self { lat_deg, lon_deg }
    }

    /// Returns the latitude converted to radians.
    pub fn lat_rad(&self) -> f32 {
        self.lat_deg * PI / 180.0
    }

    /// Returns the longitude converted to radians.
    pub fn lon_rad(&self) -> f32 {
        self.lon_deg * PI / 180.0
    }
}

/// Earth-Centered Earth-Fixed (ECEF) Cartesian coordinate in meters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ecef {
    /// X coordinate in meters.
    pub x: f32,
    /// Y coordinate in meters.
    pub y: f32,
    /// Z coordinate in meters.
    pub z: f32,
}

impl Ecef {
    /// Converts a geographic coordinate to ECEF Cartesian coordinates.
    pub fn from_latlon(coord: LatLon) -> Self {
        let phi = coord.lat_rad();
        let lambda = coord.lon_rad();
        let cos_phi = libm::cosf(phi);

        Self {
            x: EARTH_RADIUS_M * cos_phi * libm::cosf(lambda),
            y: EARTH_RADIUS_M * cos_phi * libm::sinf(lambda),
            z: EARTH_RADIUS_M * libm::sinf(phi),
        }
    }

    /// Converts back to a geographic coordinate.
    pub fn to_latlon(&self) -> LatLon {
        let r = libm::sqrtf(self.x * self.x + self.y * self.y + self.z * self.z);
        let lat_rad = libm::asinf(self.z / r);
        let lon_rad = libm::atan2f(self.y, self.x);

        LatLon {
            lat_deg: lat_rad * 180.0 / PI,
            lon_deg: lon_rad * 180.0 / PI,
        }
    }

    /// Returns the Euclidean distance to another ECEF point in meters.
    pub fn distance(&self, other: &Ecef) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        libm::sqrtf(dx * dx + dy * dy + dz * dz)
    }

    /// Returns the elevation angle (in degrees) from a ground
    /// station at `station` to this ECEF point (a satellite).
    ///
    /// Positive means above the local horizon, negative means
    /// below. A satellite is visible when elevation exceeds a
    /// minimum (typically 5-10°).
    pub fn elevation_from(&self, station: LatLon) -> f32 {
        let gs = Ecef::from_latlon(station);
        let dx = self.x - gs.x;
        let dy = self.y - gs.y;
        let dz = self.z - gs.z;
        let dist = libm::sqrtf(dx * dx + dy * dy + dz * dz);
        if dist < 1.0 {
            return 90.0;
        }

        // Local "up" unit vector at the ground station
        let gs_mag = libm::sqrtf(
            gs.x * gs.x + gs.y * gs.y + gs.z * gs.z,
        );
        let ux = gs.x / gs_mag;
        let uy = gs.y / gs_mag;
        let uz = gs.z / gs_mag;

        // sin(elevation) = dot(d_hat, up_hat)
        let sin_el = (dx * ux + dy * uy + dz * uz) / dist;
        libm::asinf(sin_el) * 180.0 / PI
    }
}

/// A geographic area of interest defined by its bounding box corners.
#[derive(Debug, Clone, Copy)]
pub struct GeoAoi {
    /// Upper-left (northwest) corner of the bounding box.
    pub upper_left: LatLon,
    /// Lower-right (southeast) corner of the bounding box.
    pub lower_right: LatLon,
}

impl GeoAoi {
    /// Creates a new geographic AOI from its bounding box corners.
    pub fn new(upper_left: LatLon, lower_right: LatLon) -> Self {
        Self {
            upper_left,
            lower_right,
        }
    }

    /// Returns `true` if the given point lies within this AOI.
    pub fn contains(&self, point: LatLon) -> bool {
        let lat_ok =
            point.lat_deg <= self.upper_left.lat_deg && point.lat_deg >= self.lower_right.lat_deg;
        let lon_ok =
            point.lon_deg >= self.upper_left.lon_deg && point.lon_deg <= self.lower_right.lon_deg;
        lat_ok && lon_ok
    }

    /// Returns the center point of this AOI.
    pub fn center(&self) -> LatLon {
        LatLon {
            lat_deg: (self.upper_left.lat_deg + self.lower_right.lat_deg) / 2.0,
            lon_deg: (self.upper_left.lon_deg + self.lower_right.lon_deg) / 2.0,
        }
    }

    /// Returns the longitudinal width in degrees.
    pub fn width_deg(&self) -> f32 {
        self.lower_right.lon_deg - self.upper_left.lon_deg
    }

    /// Returns the latitudinal height in degrees.
    pub fn height_deg(&self) -> f32 {
        self.upper_left.lat_deg - self.lower_right.lat_deg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, epsilon: f32) -> bool {
        (a - b).abs() < epsilon
    }

    #[test]
    fn test_ecef_equator_prime_meridian() {
        let coord = LatLon::new(0.0, 0.0);
        let ecef = Ecef::from_latlon(coord);

        assert!(approx_eq(ecef.x, EARTH_RADIUS_M, 1.0));
        assert!(approx_eq(ecef.y, 0.0, 1.0));
        assert!(approx_eq(ecef.z, 0.0, 1.0));
    }

    #[test]
    fn test_ecef_north_pole() {
        let coord = LatLon::new(90.0, 0.0);
        let ecef = Ecef::from_latlon(coord);

        assert!(approx_eq(ecef.x, 0.0, 1.0));
        assert!(approx_eq(ecef.y, 0.0, 1.0));
        assert!(approx_eq(ecef.z, EARTH_RADIUS_M, 1.0));
    }

    #[test]
    fn test_roundtrip() {
        let original = LatLon::new(45.0, -122.0);
        let ecef = Ecef::from_latlon(original);
        let back = ecef.to_latlon();

        assert!(approx_eq(original.lat_deg, back.lat_deg, 0.001));
        assert!(approx_eq(original.lon_deg, back.lon_deg, 0.001));
    }

    #[test]
    fn test_geo_aoi_contains() {
        let aoi = GeoAoi::new(LatLon::new(50.0, -10.0), LatLon::new(40.0, 10.0));

        assert!(aoi.contains(LatLon::new(45.0, 0.0)));
        assert!(!aoi.contains(LatLon::new(60.0, 0.0)));
        assert!(!aoi.contains(LatLon::new(45.0, 20.0)));
    }

    #[test]
    fn test_geo_aoi_center() {
        let aoi = GeoAoi::new(LatLon::new(50.0, -10.0), LatLon::new(40.0, 10.0));
        let center = aoi.center();

        assert!(approx_eq(center.lat_deg, 45.0, 0.001));
        assert!(approx_eq(center.lon_deg, 0.0, 0.001));
    }

    #[test]
    fn test_elevation_directly_above() {
        // Satellite directly above (0,0) at 550 km
        let sat = Ecef {
            x: EARTH_RADIUS_M + 550_000.0,
            y: 0.0,
            z: 0.0,
        };
        let station = LatLon::new(0.0, 0.0);
        let elev = sat.elevation_from(station);
        assert!(
            approx_eq(elev, 90.0, 0.1),
            "directly above: elev={}",
            elev
        );
    }

    #[test]
    fn test_elevation_below_horizon() {
        // Satellite on the opposite side of the Earth
        let sat = Ecef {
            x: -(EARTH_RADIUS_M + 550_000.0),
            y: 0.0,
            z: 0.0,
        };
        let station = LatLon::new(0.0, 0.0);
        let elev = sat.elevation_from(station);
        assert!(elev < 0.0, "opposite side: elev={}", elev);
    }
}
