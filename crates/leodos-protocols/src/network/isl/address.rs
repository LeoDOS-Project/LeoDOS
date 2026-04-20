use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U32;

use crate::network::isl::torus::Point;

#[repr(transparent)]
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, Hash, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned,
)]
/// A unique numeric identifier for a spacecraft in the constellation.
pub struct SpacecraftId(pub U32);

impl SpacecraftId {
    /// Creates a new spacecraft ID from a raw `u32` value.
    pub const fn new(id: u32) -> Self {
        Self(U32::new(id))
    }

    /// Returns the underlying `u32` value.
    pub fn get(&self) -> u32 {
        self.0.get()
    }

    /// Encodes an orbital plane and satellite index into a spacecraft ID.
    pub fn encode(orb: u8, sat: u8, num_sats: u8) -> Self {
        Self::new((orb as u32 + 1) * num_sats as u32 + sat as u32)
    }

    /// Decodes this spacecraft ID into an `Address`.
    pub fn to_address(&self, num_sats: u8) -> Address {
        let n = num_sats as u32;
        let orb = self.get() / n;
        let sat = self.get() % n;
        if orb == 0 {
            Address::Ground { station: sat as u8 }
        } else {
            Address::Satellite(Point {
                orb: (orb - 1) as u8,
                sat: sat as u8,
            })
        }
    }
}

#[repr(C)]
#[derive(
    FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Copy, Clone, Debug, PartialEq, Eq, Hash,
)]
/// Wire-format address for zerocopy serialization.
pub struct RawAddress {
    ground_or_orb: u8,
    station_or_sat: u8,
}

/// A logical address in the ISL network.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Address {
    /// A ground station identified by its station number.
    Ground {
        /// Ground station index.
        station: u8,
    },
    /// A specific satellite at a grid position.
    Satellite(Point),
    /// All satellites in a given orbital plane (multicast).
    ServiceArea {
        /// Orbital plane index.
        orb: u8,
    },
}

impl RawAddress {
    /// Converts the wire-format address to an [`Address`].
    pub fn parse(&self) -> Address {
        if self.ground_or_orb == 0 {
            Address::Ground {
                station: self.station_or_sat,
            }
        } else if self.station_or_sat == 0 {
            Address::ServiceArea {
                orb: self.ground_or_orb - 1,
            }
        } else {
            Address::Satellite(Point {
                orb: self.ground_or_orb - 1,
                sat: self.station_or_sat - 1,
            })
        }
    }
}

impl From<Address> for RawAddress {
    fn from(addr: Address) -> Self {
        match addr {
            Address::Ground { station } => Self {
                ground_or_orb: 0,
                station_or_sat: station,
            },
            Address::Satellite(Point { orb, sat }) => Self {
                ground_or_orb: orb + 1,
                station_or_sat: sat + 1,
            },
            Address::ServiceArea { orb } => Self {
                ground_or_orb: orb + 1,
                station_or_sat: 0,
            },
        }
    }
}

impl Address {
    /// Creates a ground station address.
    pub fn ground(station: u8) -> Self {
        Self::Ground { station: station }
    }

    /// Creates a satellite address from orbital plane and satellite indices.
    pub fn satellite(orb: u8, sat: u8) -> Self {
        Self::Satellite(Point { orb, sat })
    }

    /// Creates a service area (multicast) address for an orbital plane.
    pub fn service_area(orb: u8) -> Self {
        Self::ServiceArea { orb }
    }

    /// Returns `true` if this address can be used as a packet source.
    pub fn is_valid_source(&self) -> bool {
        !matches!(self, Address::ServiceArea { .. })
    }

    /// Returns `true` if this satellite falls within the given service area range.
    pub fn is_in_service_area(&self, min_sat: u8, max_sat: u8) -> bool {
        match self {
            Address::Satellite(Point { sat, .. }) => {
                if min_sat <= max_sat {
                    (min_sat..=max_sat).contains(sat)
                } else {
                    (min_sat..).contains(sat) || (..=max_sat).contains(sat)
                }
            }
            _ => false,
        }
    }
}

impl From<Address> for Point {
    fn from(addr: Address) -> Self {
        match addr {
            Address::Satellite(Point { orb, sat }) => Point::new(sat, orb),
            Address::ServiceArea { orb } => Point::new(0, orb),
            Address::Ground { .. } => Point::new(0, 0),
        }
    }
}

impl From<Point> for Address {
    fn from(point: Point) -> Self {
        Address::Satellite(Point {
            orb: point.orb,
            sat: point.sat,
        })
    }
}
