use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::network::isl::torus::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable)]
pub struct SpacecraftId(u8);

/// Unique identifier for an orbit within the satellite constellation.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct OrbitId(u8);

/// Unique identifier for a satellite within its orbit.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
#[repr(transparent)]
pub struct SatelliteId(u8);

impl SatelliteId {
    pub fn is_in_service_area(self, min_sat_id: SatelliteId, max_sat_id: SatelliteId) -> bool {
        if min_sat_id <= max_sat_id {
            (min_sat_id..=max_sat_id).contains(&self)
        } else {
            (min_sat_id..).contains(&self) || (..=max_sat_id).contains(&self)
        }
    }
}

#[repr(C, packed)]
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, IntoBytes, FromBytes, Immutable, KnownLayout,
)]
pub struct Address {
    pub orbit_id: OrbitId,
    pub satellite_id: SatelliteId,
}

impl Address {
    pub fn new(orb_id: OrbitId, sat_id: SatelliteId) -> Self {
        Self {
            orbit_id: orb_id,
            satellite_id: sat_id,
        }
    }
}

impl From<Address> for Point {
    fn from(addr: Address) -> Self {
        Point::new(addr.orbit_id.0, addr.satellite_id.0)
    }
}

impl From<Point> for Address {
    fn from(point: Point) -> Self {
        Address {
            orbit_id: OrbitId(point.y),
            satellite_id: SatelliteId(point.x),
        }
    }
}
