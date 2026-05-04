//! Wire format for the walker-delta ↔ LeoDOS bridge.
//!
//! Walker-delta publishes simulation state (orbital positions, LOS
//! bitmasks, time) over UDP. The LeoDOS-side hwlib backends subscribe
//! to the same byte layout defined here. Mirrors the canonical
//! definition in `walker-delta/src/bridge.rs` byte-for-byte; once
//! the protocol stabilizes, both sides will share a single crate.
//!
//! Encoding rules:
//! - Big-endian (network byte order) for all multi-byte fields.
//! - `#[repr(C)]` + zerocopy for stable layout, no allocation,
//!   no serde overhead.
//! - One `StateHeader` followed by `num_sats × SatState` per UDP
//!   datagram. Receivers MUST validate `magic` and `version`.

use zerocopy::byteorder::network_endian::F64;
use zerocopy::byteorder::network_endian::U16;
use zerocopy::byteorder::network_endian::U32;
use zerocopy::byteorder::network_endian::U64;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// Magic bytes identifying a walker-delta → LeoDOS state packet.
pub const STATE_MAGIC: [u8; 4] = *b"LEOS";

/// Wire format version. Bump on any layout change.
pub const BRIDGE_VERSION: u16 = 1;

/// UDP port LeoDOS hwlib backends listen on for state.
pub const TOPOLOGY_PORT: u16 = 7000;

/// North direction in `los_neighbors` bitmask.
pub const DIR_NORTH: u8 = 0;
/// South direction in `los_neighbors` bitmask.
pub const DIR_SOUTH: u8 = 1;
/// East direction in `los_neighbors` bitmask.
pub const DIR_EAST: u8 = 2;
/// West direction in `los_neighbors` bitmask.
pub const DIR_WEST: u8 = 3;
/// Ground link encoded separately from torus neighbors.
pub const DIR_GROUND: u8 = 4;

/// Header for a state packet. Followed by `num_sats × SatState`.
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct StateHeader {
    /// Identifies the packet kind. Must equal [`STATE_MAGIC`].
    pub magic: [u8; 4],
    /// Wire format version. See [`BRIDGE_VERSION`].
    pub version: U16,
    /// Monotonic sequence number assigned by the publisher.
    pub seq: U32,
    /// Simulated mission time in milliseconds since epoch.
    pub sim_time_ms: U64,
    /// Wall clock time in milliseconds when the packet was published.
    pub real_time_ms: U64,
    /// Number of [`SatState`] entries that follow.
    pub num_sats: U16,
    /// Padding for 8-byte alignment of the body.
    pub _pad: [u8; 4],
}

impl StateHeader {
    /// Construct a fresh header with current magic + version.
    pub fn new(seq: u32, sim_time_ms: u64, real_time_ms: u64, num_sats: u16) -> Self {
        Self {
            magic: STATE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            seq: U32::new(seq),
            sim_time_ms: U64::new(sim_time_ms),
            real_time_ms: U64::new(real_time_ms),
            num_sats: U16::new(num_sats),
            _pad: [0; 4],
        }
    }

    /// Check magic and version against expected constants.
    pub fn validate(&self) -> Result<(), DecodeError> {
        if self.magic != STATE_MAGIC {
            return Err(DecodeError::BadMagic);
        }
        if self.version.get() != BRIDGE_VERSION {
            return Err(DecodeError::VersionMismatch {
                expected: BRIDGE_VERSION,
                got: self.version.get(),
            });
        }
        Ok(())
    }
}

/// Per-satellite state. Position in ECI meters, velocity in m/s,
/// nadir attitude as a body→ECI quaternion (w, x, y, z).
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct SatState {
    /// Spacecraft identifier (matches the `--scid` cFS launches with).
    pub scid: U32,
    /// ECI position in meters.
    pub pos_eci_m: [F64; 3],
    /// ECI velocity in m/s.
    pub vel_eci_m_s: [F64; 3],
    /// Body→ECI quaternion (w, x, y, z) for nadir-pointing attitude.
    pub nadir_quat: [F64; 4],
    /// Bitmask of torus neighbors currently in line of sight. See `DIR_*`.
    pub los_neighbors: u8,
    /// Bitmask of ground stations currently in view (gateway IDs).
    pub los_ground: U16,
    /// Padding for 8-byte alignment.
    pub _pad: [u8; 9],
}

impl SatState {
    /// Construct a sat state with the given orbital and visibility data.
    pub fn new(
        scid: u32,
        pos_eci_m: [f64; 3],
        vel_eci_m_s: [f64; 3],
        nadir_quat: [f64; 4],
        los_neighbors: u8,
        los_ground: u16,
    ) -> Self {
        Self {
            scid: U32::new(scid),
            pos_eci_m: [
                F64::new(pos_eci_m[0]),
                F64::new(pos_eci_m[1]),
                F64::new(pos_eci_m[2]),
            ],
            vel_eci_m_s: [
                F64::new(vel_eci_m_s[0]),
                F64::new(vel_eci_m_s[1]),
                F64::new(vel_eci_m_s[2]),
            ],
            nadir_quat: [
                F64::new(nadir_quat[0]),
                F64::new(nadir_quat[1]),
                F64::new(nadir_quat[2]),
                F64::new(nadir_quat[3]),
            ],
            los_neighbors,
            los_ground: U16::new(los_ground),
            _pad: [0; 9],
        }
    }

    /// Returns `true` if `dir` (one of `DIR_*`) is in the neighbor mask.
    pub fn los_has(&self, dir: u8) -> bool {
        (self.los_neighbors >> dir) & 1 == 1
    }
}

/// Errors decoding an incoming state packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Buffer was shorter than [`StateHeader`].
    HeaderTooShort,
    /// Header magic did not match [`STATE_MAGIC`].
    BadMagic,
    /// Wire format version did not match [`BRIDGE_VERSION`].
    VersionMismatch {
        /// Version this build understands.
        expected: u16,
        /// Version observed in the packet.
        got: u16,
    },
    /// Buffer was shorter than `num_sats × size_of::<SatState>`.
    BodyTooShort {
        /// Bytes needed to decode the declared sat count.
        expected: usize,
        /// Bytes actually available after the header.
        got: usize,
    },
}

/// Encode a state packet into `out`. Returns the number of bytes
/// written. `out` must be at least
/// `size_of::<StateHeader>() + sats.len() * size_of::<SatState>()`.
pub fn encode_state(out: &mut [u8], header: &StateHeader, sats: &[SatState]) -> usize {
    let header_len = core::mem::size_of::<StateHeader>();
    let sat_len = core::mem::size_of::<SatState>();
    let total = header_len + sats.len() * sat_len;
    assert!(out.len() >= total, "buffer too small for state packet");
    out[..header_len].copy_from_slice(header.as_bytes());
    let mut off = header_len;
    for sat in sats {
        out[off..off + sat_len].copy_from_slice(sat.as_bytes());
        off += sat_len;
    }
    total
}

/// Decode a state packet. Returns header + view of the sat array.
pub fn decode_state(buf: &[u8]) -> Result<(StateHeader, &[SatState]), DecodeError> {
    let header_len = core::mem::size_of::<StateHeader>();
    if buf.len() < header_len {
        return Err(DecodeError::HeaderTooShort);
    }
    let header = StateHeader::read_from_bytes(&buf[..header_len])
        .map_err(|_| DecodeError::HeaderTooShort)?;
    header.validate()?;
    let n = header.num_sats.get() as usize;
    let body = &buf[header_len..];
    let expected = n * core::mem::size_of::<SatState>();
    if body.len() < expected {
        return Err(DecodeError::BodyTooShort {
            expected,
            got: body.len(),
        });
    }
    let sats = <[SatState]>::ref_from_bytes(&body[..expected])
        .map_err(|_| DecodeError::BodyTooShort {
            expected,
            got: body.len(),
        })?;
    Ok((header, sats))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_size_is_stable() {
        assert_eq!(core::mem::size_of::<StateHeader>(), 32);
    }

    #[test]
    fn sat_state_size_is_stable() {
        assert_eq!(core::mem::size_of::<SatState>(), 96);
    }

    #[test]
    fn round_trip_three_sats() {
        let sats = [
            SatState::new(1, [7000e3, 0.0, 0.0], [0.0, 7.5e3, 0.0], [1.0, 0.0, 0.0, 0.0], 0b0011, 0),
            SatState::new(2, [0.0, 7000e3, 0.0], [-7.5e3, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], 0b1100, 0b0001),
            SatState::new(3, [0.0, 0.0, 7000e3], [0.0, 0.0, 7.5e3], [1.0, 0.0, 0.0, 0.0], 0b1111, 0b0011),
        ];
        let h = StateHeader::new(42, 1_000_000, 2_000_000, sats.len() as u16);
        let mut buf = [0u8; 32 + 3 * 96];
        let n = encode_state(&mut buf, &h, &sats);
        assert_eq!(n, buf.len());

        let (h2, decoded) = decode_state(&buf[..n]).unwrap();
        assert_eq!(h2.seq.get(), 42);
        assert_eq!(h2.num_sats.get(), 3);
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].scid.get(), 1);
        assert_eq!(decoded[0].pos_eci_m[0].get(), 7000e3);
        assert_eq!(decoded[2].los_neighbors, 0b1111);
        assert_eq!(decoded[2].los_ground.get(), 0b0011);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut buf = [0u8; 32];
        let h = StateHeader::new(0, 0, 0, 0);
        encode_state(&mut buf, &h, &[]);
        buf[0] = b'X';
        assert!(matches!(decode_state(&buf), Err(DecodeError::BadMagic)));
    }

    #[test]
    fn los_has_decodes_bitmask() {
        let s = SatState::new(0, [0.0; 3], [0.0; 3], [1.0, 0.0, 0.0, 0.0], 0b1010, 0);
        assert!(!s.los_has(DIR_NORTH));
        assert!(s.los_has(DIR_SOUTH));
        assert!(!s.los_has(DIR_EAST));
        assert!(s.los_has(DIR_WEST));
    }
}
