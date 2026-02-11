use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;

use crate::network::isl::torus::Point;

#[repr(transparent)]
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned,
)]
pub struct SpacecraftId(pub U16);

impl SpacecraftId {
    pub const fn new(id: u16) -> Self {
        Self(U16::new(id))
    }

    pub fn get(&self) -> u16 {
        self.0.get()
    }

    pub fn orbit(&self) -> u16 {
        self.get() / 1000
    }

    pub fn sat(&self) -> u16 {
        self.get() % 1000
    }
}

impl From<SpacecraftId> for Address {
    fn from(scid: SpacecraftId) -> Self {
        let orbit = scid.orbit();
        let sat = scid.sat() as u8;
        if orbit == 0 {
            Address::Ground { station_id: sat }
        } else {
            Address::Satellite {
                orbit_id: (orbit - 1) as u8,
                satellite_id: sat,
            }
        }
    }
}

impl TryFrom<Address> for SpacecraftId {
    type Error = ();

    fn try_from(addr: Address) -> Result<Self, Self::Error> {
        match addr {
            Address::Ground { station_id } => Ok(SpacecraftId::new(station_id as u16)),
            Address::Satellite {
                orbit_id,
                satellite_id,
            } => Ok(SpacecraftId::new(
                ((orbit_id + 1) as u16) * 1000 + satellite_id as u16,
            )),
            Address::ServiceArea { .. } => Err(()),
        }
    }
}

#[repr(C)]
#[derive(
    FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Copy, Clone, Debug, PartialEq, Eq, Hash,
)]
pub struct RawAddress {
    ground_or_orbit: u8,
    station_or_sat: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Address {
    Ground { station_id: u8 },
    Satellite { orbit_id: u8, satellite_id: u8 },
    ServiceArea { orbit_id: u8 },
}

impl RawAddress {
    pub fn parse(&self) -> Address {
        if self.ground_or_orbit == 0 {
            Address::Ground {
                station_id: self.station_or_sat,
            }
        } else if self.station_or_sat == 0 {
            Address::ServiceArea {
                orbit_id: self.ground_or_orbit - 1,
            }
        } else {
            Address::Satellite {
                orbit_id: self.ground_or_orbit - 1,
                satellite_id: self.station_or_sat,
            }
        }
    }
}

impl From<Address> for RawAddress {
    fn from(addr: Address) -> Self {
        match addr {
            Address::Ground { station_id } => Self {
                ground_or_orbit: 0,
                station_or_sat: station_id,
            },
            Address::Satellite {
                orbit_id,
                satellite_id,
            } => Self {
                ground_or_orbit: orbit_id + 1,
                station_or_sat: satellite_id,
            },
            Address::ServiceArea { orbit_id } => Self {
                ground_or_orbit: orbit_id + 1,
                station_or_sat: 0,
            },
        }
    }
}

impl Address {
    pub fn ground(station_id: u8) -> Self {
        Self::Ground { station_id }
    }

    pub fn satellite(orbit_id: u8, satellite_id: u8) -> Self {
        Self::Satellite {
            orbit_id,
            satellite_id,
        }
    }

    pub fn service_area(orbit_id: u8) -> Self {
        Self::ServiceArea { orbit_id }
    }

    pub fn is_valid_source(&self) -> bool {
        !matches!(self, Address::ServiceArea { .. })
    }

    pub fn is_in_service_area(&self, min_sat_id: u8, max_sat_id: u8) -> bool {
        match self {
            Address::Satellite { satellite_id, .. } => {
                if min_sat_id <= max_sat_id {
                    (min_sat_id..=max_sat_id).contains(satellite_id)
                } else {
                    (min_sat_id..).contains(satellite_id) || (..=max_sat_id).contains(satellite_id)
                }
            }
            _ => false,
        }
    }
}

impl From<Address> for Point {
    fn from(addr: Address) -> Self {
        match addr {
            Address::Satellite {
                orbit_id,
                satellite_id,
            } => Point::new(satellite_id, orbit_id),
            Address::ServiceArea { orbit_id } => Point::new(0, orbit_id),
            Address::Ground { .. } => Point::new(0, 0),
        }
    }
}

impl From<Point> for Address {
    fn from(point: Point) -> Self {
        Address::Satellite {
            orbit_id: point.y,
            satellite_id: point.x,
        }
    }
}
