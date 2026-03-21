//! SpaceCoMP job request.
//!
//! A job specifies a geographic area of interest and the parameters for each
//! of the three processing phases (Collect, Map, Reduce). Ground stations
//! submit jobs to the nearest visible (LOS) satellite for orchestration.
//!
//! `Job` is a 41-byte zero-copy, network-endian wire type that doubles as the
//! `SubmitJob` payload.

use bon::bon;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U32;
use zerocopy::network_endian::U64;

use crate::network::isl::geo::GeoAoi;
use crate::network::isl::geo::LatLon;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// Which assignment algorithm to use for collector-to-mapper matching.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Default)]
pub enum AssignmentSolver {
    /// Kuhn-Munkres (Hungarian) algorithm, O(n^3).
    #[default]
    Hungarian = 0,
    /// Jonker-Volgenant (LAPJV) algorithm, O(n^3) but typically faster.
    JonkerVolgenant = 1,
}

impl TryFrom<u8> for AssignmentSolver {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Hungarian),
            1 => Ok(Self::JonkerVolgenant),
            _ => Err(()),
        }
    }
}

/// A complete SpaceCoMP job request (Section II-A of the paper).
///
/// 41-byte zero-copy wire format, network-endian. Floats are stored
/// as `U32` via `to_bits`/`from_bits`. The `flags` byte packs
/// `ascending_only` (bit 0) and [`AssignmentSolver`] (bits 1-2).
///
/// ```text
/// Offset  Field                      Type   Bytes
/// 0       upper_left_lat             U32    4   (f32 bits)
/// 4       upper_left_lon             U32    4
/// 8       lower_right_lat            U32    4
/// 12      lower_right_lon            U32    4
/// 16      data_volume_bytes          U64    8
/// 24      map_processing_factor      U32    4   (f32 bits)
/// 28      reduce_processing_factor   U32    4
/// 32      map_reduction_factor       U32    4
/// 36      reduce_reduction_factor    U32    4
/// 40      flags                      u8     1
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct Job {
    upper_left_lat: U32,
    upper_left_lon: U32,
    lower_right_lat: U32,
    lower_right_lon: U32,
    data_volume_bytes: U64,
    map_processing_factor: U32,
    reduce_processing_factor: U32,
    map_reduction_factor: U32,
    reduce_reduction_factor: U32,
    flags: u8,
}

#[rustfmt::skip]
mod bitmask {
    pub const ASCENDING_ONLY_MASK: u8 = 0b_0000_0001;
    pub const SOLVER_MASK: u8 =         0b_0000_0110;
}

#[bon]
impl Job {
    #[builder]
    /// Constructs a new job.
    pub fn new(
        geo_aoi: GeoAoi,
        data_volume_bytes: u64,
        #[builder(default = 1.0)] map_processing_factor: f32,
        #[builder(default = 1.0)] reduce_processing_factor: f32,
        #[builder(default = 1.0)] map_reduction_factor: f32,
        #[builder(default = 5.0)] reduce_reduction_factor: f32,
        #[builder(default = false)] ascending_only: bool,
        #[builder(default)] solver: AssignmentSolver,
    ) -> Self {
        let mut flags = 0u8;
        set_bits_u8(&mut flags, bitmask::ASCENDING_ONLY_MASK, ascending_only as u8);
        set_bits_u8(&mut flags, bitmask::SOLVER_MASK, solver as u8);
        Self {
            upper_left_lat: U32::new(geo_aoi.upper_left.lat_deg.to_bits()),
            upper_left_lon: U32::new(geo_aoi.upper_left.lon_deg.to_bits()),
            lower_right_lat: U32::new(geo_aoi.lower_right.lat_deg.to_bits()),
            lower_right_lon: U32::new(geo_aoi.lower_right.lon_deg.to_bits()),
            data_volume_bytes: U64::new(data_volume_bytes),
            map_processing_factor: U32::new(map_processing_factor.to_bits()),
            reduce_processing_factor: U32::new(reduce_processing_factor.to_bits()),
            map_reduction_factor: U32::new(map_reduction_factor.to_bits()),
            reduce_reduction_factor: U32::new(reduce_reduction_factor.to_bits()),
            flags,
        }
    }

    /// Returns the geographic area of interest.
    pub fn geo_aoi(&self) -> GeoAoi {
        GeoAoi::new(
            LatLon::new(
                f32::from_bits(self.upper_left_lat.get()),
                f32::from_bits(self.upper_left_lon.get()),
            ),
            LatLon::new(
                f32::from_bits(self.lower_right_lat.get()),
                f32::from_bits(self.lower_right_lon.get()),
            ),
        )
    }

    /// Returns the data volume per collect task in bytes.
    pub fn data_volume_bytes(&self) -> u64 {
        self.data_volume_bytes.get()
    }

    /// Returns the map processing time factor.
    pub fn map_processing_factor(&self) -> f32 {
        f32::from_bits(self.map_processing_factor.get())
    }

    /// Returns the reduce processing time factor.
    pub fn reduce_processing_factor(&self) -> f32 {
        f32::from_bits(self.reduce_processing_factor.get())
    }

    /// Returns the map output compression ratio.
    pub fn map_reduction_factor(&self) -> f32 {
        f32::from_bits(self.map_reduction_factor.get())
    }

    /// Returns the reduce output compression ratio.
    pub fn reduce_reduction_factor(&self) -> f32 {
        f32::from_bits(self.reduce_reduction_factor.get())
    }

    /// Returns whether only ascending satellites should be used.
    pub fn ascending_only(&self) -> bool {
        get_bits_u8(self.flags, bitmask::ASCENDING_ONLY_MASK) != 0
    }

    /// Returns the assignment solver algorithm.
    pub fn solver(&self) -> AssignmentSolver {
        let bits = get_bits_u8(self.flags, bitmask::SOLVER_MASK);
        AssignmentSolver::try_from(bits).unwrap_or_default()
    }
}
