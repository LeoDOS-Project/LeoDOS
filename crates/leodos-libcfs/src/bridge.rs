//! Wire format for the leo-viz ↔ LeoDOS bridge over TCP.
//!
//! Mirrors the canonical definition in `leo-viz/src/bridge.rs`
//! byte-for-byte. Once the protocol stabilizes both sides will
//! share a single crate.
//!
//! The cFS-side `sim_client` opens a TCP connection at boot, writes
//! one [`Hello`] frame to identify its spacecraft id, and then reads
//! a stream of [`StateFrame`]s — one per simulator tick — each
//! carrying the satellite's current ECI position/velocity, attitude,
//! and link visibility.
//!
//! Encoding rules:
//! - Big-endian (network byte order) for all multi-byte fields.
//! - `#[repr(C)]` + zerocopy for stable layout, no allocation,
//!   no serde overhead.
//! - Both message types are fixed size — no length prefix needed.
//!   Receivers MUST validate `magic` and `version` on every frame
//!   to recover from a stream gone out of sync.

use zerocopy::byteorder::network_endian::F64;
use zerocopy::byteorder::network_endian::U16;
use zerocopy::byteorder::network_endian::U32;
use zerocopy::byteorder::network_endian::U64;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// Magic bytes identifying any frame in the leo-viz ↔ LeoDOS protocol.
pub const BRIDGE_MAGIC: [u8; 4] = *b"LEOS";

/// Wire format version. Bump on any layout change.
pub const BRIDGE_VERSION: u16 = 2;

/// Default loopback port. Production launches pass an explicit
/// `host:port` via the `LEODOS_BRIDGE_ADDR` env var; this constant
/// is only a fallback for standalone tests.
pub const DEFAULT_BRIDGE_PORT: u16 = 7000;

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

/// Client → server: identifies which satellite this connection is.
/// Sent once immediately after the TCP handshake, before any
/// [`StateFrame`]s are read.
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct Hello {
    /// Identifies the packet kind. Must equal [`BRIDGE_MAGIC`].
    pub magic: [u8; 4],
    /// Wire format version. See [`BRIDGE_VERSION`].
    pub version: U16,
    /// Padding to align the next field.
    pub _pad0: [u8; 2],
    /// Spacecraft id this connection represents.
    pub scid: U32,
    /// Trailing padding to a multiple of 8 bytes.
    pub _pad1: [u8; 4],
}

impl Hello {
    /// Construct a hello with current magic + version for `scid`.
    pub fn new(scid: u32) -> Self {
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            _pad0: [0; 2],
            scid: U32::new(scid),
            _pad1: [0; 4],
        }
    }

    /// Check magic and version against expected constants.
    pub fn validate(&self) -> Result<(), DecodeError> {
        if self.magic != BRIDGE_MAGIC {
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

/// Server → client: per-tick snapshot of one satellite's state.
/// Position in ECI meters, velocity in m/s, nadir attitude as a
/// body→ECI quaternion (w, x, y, z).
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct StateFrame {
    /// Identifies the packet kind. Must equal [`BRIDGE_MAGIC`].
    pub magic: [u8; 4],
    /// Wire format version. See [`BRIDGE_VERSION`].
    pub version: U16,
    /// Padding to align the next field.
    pub _pad0: [u8; 2],
    /// Monotonic sequence number assigned by leo-viz.
    pub seq: U32,
    /// Simulated mission time in milliseconds since epoch.
    pub sim_time_ms: U64,
    /// Wall clock time in milliseconds when the frame was published.
    pub real_time_ms: U64,
    /// Spacecraft id this frame is addressed to (matches `Hello.scid`).
    pub scid: U32,
    /// Padding to align the next field.
    pub _pad1: [u8; 4],
    /// ECI position in meters.
    pub pos_eci_m: [F64; 3],
    /// ECI velocity in m/s.
    pub vel_eci_m_s: [F64; 3],
    /// Body→ECI quaternion (w, x, y, z) for nadir-pointing attitude.
    pub nadir_quat: [F64; 4],
    /// Bitmask of torus neighbors currently in line of sight.
    pub los_neighbors: u8,
    /// Padding.
    pub _pad2: [u8; 1],
    /// Bitmask of ground stations currently in view.
    pub los_ground: U16,
    /// Trailing padding to a multiple of 4 bytes.
    pub _pad3: [u8; 4],
}

impl StateFrame {
    /// Check magic and version against expected constants.
    pub fn validate(&self) -> Result<(), DecodeError> {
        if self.magic != BRIDGE_MAGIC {
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

    /// Returns `true` if `dir` (one of `DIR_*`) is in the neighbor mask.
    pub fn los_has(&self, dir: u8) -> bool {
        (self.los_neighbors >> dir) & 1 == 1
    }
}

/// Errors decoding an incoming frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// Buffer was too short for the expected frame size.
    Truncated {
        /// Bytes needed to decode the frame.
        expected: usize,
        /// Bytes actually available.
        got: usize,
    },
    /// Frame magic did not match [`BRIDGE_MAGIC`].
    BadMagic,
    /// Wire format version did not match [`BRIDGE_VERSION`].
    VersionMismatch {
        /// Version this build understands.
        expected: u16,
        /// Version observed in the frame.
        got: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_size_is_stable() {
        assert_eq!(core::mem::size_of::<Hello>(), 16);
    }

    #[test]
    fn state_frame_size_is_stable() {
        assert_eq!(core::mem::size_of::<StateFrame>(), 124);
    }

    #[test]
    fn hello_round_trip() {
        let h = Hello::new(42);
        let bytes = h.as_bytes();
        let decoded = Hello::read_from_bytes(bytes).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded.scid.get(), 42);
    }
}
