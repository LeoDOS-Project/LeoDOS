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
/// A satellite's identifier in the constellation.
///
/// Values in `[0, num_planes * sats_per_plane)` map to satellites in
/// plane-major order; values outside that range are not satellites
/// and decode to `None`. Ground stations are identified separately
/// via [`GroundStationId`] and never share this namespace.
pub struct SpacecraftId(pub U32);

impl SpacecraftId {
    /// Wraps a raw `u32` cFE spacecraft ID.
    pub const fn new(id: u32) -> Self {
        Self(U32::new(id))
    }

    /// Returns the underlying `u32` value.
    pub fn get(&self) -> u32 {
        self.0.get()
    }

    /// Encodes a `(orb, sat)` pair into a satellite ID.
    pub fn encode(orb: u8, sat: u8, sats_per_plane: u8) -> Self {
        Self::new(orb as u32 * sats_per_plane as u32 + sat as u32)
    }

    /// Decodes this satellite ID into a satellite [`Address`], or
    /// `None` if the ID falls outside `[0, num_planes * sats_per_plane)`.
    pub fn to_address(&self, num_planes: u8, sats_per_plane: u8) -> Option<Address> {
        let n = num_planes as u32 * sats_per_plane as u32;
        let id = self.get();
        (id < n).then(|| {
            Address::Satellite(Point {
                orb: (id / sats_per_plane as u32) as u8,
                sat: (id % sats_per_plane as u32) as u8,
            })
        })
    }
}

/// A ground station's identifier. Disjoint from [`SpacecraftId`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroundStationId(pub u8);

impl GroundStationId {
    /// Decodes this ground station ID into an [`Address`].
    pub fn to_address(&self) -> Address {
        Address::Ground { station: self.0 }
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
}

impl RawAddress {
    /// Converts the wire-format address to an [`Address`].
    pub fn parse(&self) -> Address {
        if self.ground_or_orb == 0 {
            Address::Ground {
                station: self.station_or_sat,
            }
        } else {
            Address::Satellite(Point {
                orb: self.ground_or_orb - 1,
                sat: self.station_or_sat,
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
                station_or_sat: sat,
            },
        }
    }
}

impl Address {
    /// Creates a ground station address.
    pub fn ground(station: u8) -> Self {
        Self::Ground { station }
    }

    /// Creates a satellite address from orbital plane and satellite indices.
    pub fn satellite(orb: u8, sat: u8) -> Self {
        Self::Satellite(Point { orb, sat })
    }
}

impl From<Address> for Point {
    fn from(addr: Address) -> Self {
        match addr {
            Address::Satellite(p) => p,
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
