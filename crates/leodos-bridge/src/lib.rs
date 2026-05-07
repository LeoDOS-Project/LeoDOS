//! Wire format for the leo-viz ↔ LeoDOS bridge over TCP.
//!
//! Shared by all bridge endpoints — leo-viz (`walker-delta`),
//! `sim_client` (re-exported via `leodos-libcfs::bridge`), and the
//! ground station daemon (`leodos-ground` in bridge mode).
//!
//! Encoding rules:
//! - Big-endian (network byte order) for all multi-byte fields.
//! - `#[repr(C)]` + zerocopy for stable layout, no allocation,
//!   no serde overhead.
//! - All message types are fixed size; no length prefix needed.
//!   Receivers MUST validate `magic` and `version` on every frame
//!   to recover from a stream gone out of sync.

#![no_std]
#![deny(missing_docs)]

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

/// Endpoint kind: this connection represents a satellite running cFS.
pub const ENDPOINT_SATELLITE: u8 = 0;
/// Endpoint kind: this connection represents a ground station daemon.
pub const ENDPOINT_GROUND: u8 = 1;

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
    /// One of [`ENDPOINT_SATELLITE`] or [`ENDPOINT_GROUND`].
    pub endpoint_kind: u8,
    /// Padding.
    pub _pad0: u8,
    /// For Satellite endpoints: the spacecraft id.
    /// For Ground endpoints: the ground station id.
    pub scid: U32,
    /// Trailing padding to a multiple of 8 bytes.
    pub _pad1: [u8; 4],
}

impl Hello {
    /// Construct a Satellite hello with current magic + version.
    pub fn new(scid: u32) -> Self {
        Self::satellite(scid)
    }

    /// Construct a Satellite hello.
    pub fn satellite(scid: u32) -> Self {
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            endpoint_kind: ENDPOINT_SATELLITE,
            _pad0: 0,
            scid: U32::new(scid),
            _pad1: [0; 4],
        }
    }

    /// Construct a Ground-station hello.
    pub fn ground(station_id: u32) -> Self {
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            endpoint_kind: ENDPOINT_GROUND,
            _pad0: 0,
            scid: U32::new(station_id),
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
    /// Construct a state frame with current magic + version.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        seq: u32,
        sim_time_ms: u64,
        real_time_ms: u64,
        scid: u32,
        pos_eci_m: [f64; 3],
        vel_eci_m_s: [f64; 3],
        nadir_quat: [f64; 4],
        los_neighbors: u8,
        los_ground: u16,
    ) -> Self {
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            _pad0: [0; 2],
            seq: U32::new(seq),
            sim_time_ms: U64::new(sim_time_ms),
            real_time_ms: U64::new(real_time_ms),
            scid: U32::new(scid),
            _pad1: [0; 4],
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
            _pad2: [0; 1],
            los_ground: U16::new(los_ground),
            _pad3: [0; 4],
        }
    }

    /// Returns `true` if direction `dir` (one of `DIR_*`) is set in
    /// `los_neighbors`.
    pub fn los_has(&self, dir: u8) -> bool {
        (self.los_neighbors >> dir) & 1 == 1
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

/// Client → server: one EVS event observed on this spacecraft's
/// software bus. The cFS `sim_client` subscribes to
/// `CFE_EVS_LONG_EVENT_MSG_MID` and forwards each event verbatim
/// (only the fields a visualizer cares about) to leo-viz.
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct EventFrame {
    /// Identifies the packet kind. Must equal [`BRIDGE_MAGIC`].
    pub magic: [u8; 4],
    /// Wire format version. See [`BRIDGE_VERSION`].
    pub version: U16,
    /// Padding to align the next field.
    pub _pad0: [u8; 2],
    /// Monotonic sequence number assigned by the emitting sim_client.
    pub seq: U32,
    /// Padding.
    pub _pad1: [u8; 4],
    /// Mission time in milliseconds when the event was emitted.
    pub sim_time_ms: U64,
    /// Spacecraft id that fired the event.
    pub scid: U32,
    /// EVS event id.
    pub event_id: U16,
    /// EVS event type: 1=DEBUG, 2=INFO, 3=ERROR, 4=CRITICAL.
    pub event_type: u8,
    /// Padding.
    pub _pad2: u8,
    /// Source app name, null-padded.
    pub app_name: [u8; 20],
    /// Event message, null-padded.
    pub message: [u8; 96],
}

impl EventFrame {
    /// Construct an event frame with current magic + version.
    pub fn new(
        seq: u32,
        sim_time_ms: u64,
        scid: u32,
        event_id: u16,
        event_type: u8,
        app_name: &[u8],
        message: &[u8],
    ) -> Self {
        let mut app_buf = [0u8; 20];
        let mut msg_buf = [0u8; 96];
        let app_len = app_name.len().min(20);
        let msg_len = message.len().min(96);
        app_buf[..app_len].copy_from_slice(&app_name[..app_len]);
        msg_buf[..msg_len].copy_from_slice(&message[..msg_len]);
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            _pad0: [0; 2],
            seq: U32::new(seq),
            _pad1: [0; 4],
            sim_time_ms: U64::new(sim_time_ms),
            scid: U32::new(scid),
            event_id: U16::new(event_id),
            event_type,
            _pad2: 0,
            app_name: app_buf,
            message: msg_buf,
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

    /// App name decoded as UTF-8, trimmed at the first NUL.
    pub fn app_name_str(&self) -> &str {
        let end = self.app_name.iter().position(|&b| b == 0).unwrap_or(20);
        core::str::from_utf8(&self.app_name[..end]).unwrap_or("")
    }

    /// Message decoded as UTF-8, trimmed at the first NUL.
    pub fn message_str(&self) -> &str {
        let end = self.message.iter().position(|&b| b == 0).unwrap_or(96);
        core::str::from_utf8(&self.message[..end]).unwrap_or("")
    }
}

/// Server → ground daemon: ask the daemon to send a ping to the
/// satellite at `(target_orb, target_sat)` and report the result.
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct PingRequestFrame {
    /// Identifies the packet kind. Must equal [`BRIDGE_MAGIC`].
    pub magic: [u8; 4],
    /// Wire format version.
    pub version: U16,
    /// Padding.
    pub _pad0: [u8; 2],
    /// Caller-assigned id, echoed back in the result EventFrame so
    /// the UI can correlate request and response.
    pub request_id: U32,
    /// Target orbit index.
    pub target_orb: u8,
    /// Target sat index within plane.
    pub target_sat: u8,
    /// Sats per plane (constellation parameter the daemon doesn't
    /// know on its own).
    pub num_sats_per_plane: u8,
    /// Padding.
    pub _pad1: u8,
    /// SRSPP retransmit timeout (ms).
    pub rto_ms: U32,
    /// Overall ping timeout (ms).
    pub timeout_ms: U32,
    /// Trailing padding.
    pub _pad2: [u8; 8],
}

impl PingRequestFrame {
    /// Construct a request frame with current magic + version.
    pub fn new(
        request_id: u32,
        target_orb: u8,
        target_sat: u8,
        num_sats_per_plane: u8,
        rto_ms: u32,
        timeout_ms: u32,
    ) -> Self {
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            _pad0: [0; 2],
            request_id: U32::new(request_id),
            target_orb,
            target_sat,
            num_sats_per_plane,
            _pad1: 0,
            rto_ms: U32::new(rto_ms),
            timeout_ms: U32::new(timeout_ms),
            _pad2: [0; 8],
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

/// One-byte tag prefixed to frames the bridge server pushes to a
/// connected ground daemon. Lets the daemon discriminate between
/// fixed-size frame types arriving on the same TCP stream.
/// Sat-side and daemon-to-server traffic does not use this tag —
/// it's a single direction with mixed frame types.
pub const KIND_PING_REQUEST: u8 = 1;
/// See [`KIND_PING_REQUEST`].
pub const KIND_GROUND_STATE: u8 = 2;

/// Maximum number of LOS-visible satellites a [`GroundStateFrame`]
/// can carry. Excess sats are truncated; entries past `visible_count`
/// are zero-padded.
pub const GROUND_STATE_MAX_VISIBLE: usize = 32;

/// One satellite currently in line-of-sight from a ground station,
/// as carried by [`GroundStateFrame`].
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct VisibleSat {
    /// Orbit (plane) index.
    pub orb: u8,
    /// Satellite index within plane.
    pub sat: u8,
}

/// Server → ground daemon: per-tick snapshot of which satellites are
/// currently visible from this ground station. Daemon uses
/// `visible[0]` as its gateway (entries are ordered by elevation,
/// highest first). When `visible_count == 0` the station has no
/// link and outbound requests should fail fast.
#[repr(C)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, Immutable, KnownLayout, Unaligned)]
pub struct GroundStateFrame {
    /// Identifies the packet kind. Must equal [`BRIDGE_MAGIC`].
    pub magic: [u8; 4],
    /// Wire format version. See [`BRIDGE_VERSION`].
    pub version: U16,
    /// Padding.
    pub _pad0: [u8; 2],
    /// Monotonic sequence number assigned by leo-viz.
    pub seq: U32,
    /// Simulated mission time in milliseconds since epoch.
    pub sim_time_ms: U64,
    /// Ground station id this frame is addressed to (matches `Hello.scid`).
    pub station_id: U32,
    /// Number of valid entries in [`Self::visible`].
    pub visible_count: u8,
    /// Padding.
    pub _pad1: [u8; 3],
    /// Visible sats, ordered by elevation (highest first). Entries
    /// past `visible_count` are zero.
    pub visible: [VisibleSat; GROUND_STATE_MAX_VISIBLE],
}

impl GroundStateFrame {
    /// Construct a ground state frame with current magic + version.
    /// `visible` is truncated to at most [`GROUND_STATE_MAX_VISIBLE`].
    pub fn new(seq: u32, sim_time_ms: u64, station_id: u32, visible: &[VisibleSat]) -> Self {
        let mut buf = [VisibleSat::default(); GROUND_STATE_MAX_VISIBLE];
        let n = visible.len().min(GROUND_STATE_MAX_VISIBLE);
        buf[..n].copy_from_slice(&visible[..n]);
        Self {
            magic: BRIDGE_MAGIC,
            version: U16::new(BRIDGE_VERSION),
            _pad0: [0; 2],
            seq: U32::new(seq),
            sim_time_ms: U64::new(sim_time_ms),
            station_id: U32::new(station_id),
            visible_count: n as u8,
            _pad1: [0; 3],
            visible: buf,
        }
    }

    /// Returns the slice of valid visible-sat entries.
    pub fn visible_slice(&self) -> &[VisibleSat] {
        let n = (self.visible_count as usize).min(GROUND_STATE_MAX_VISIBLE);
        &self.visible[..n]
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
    fn event_frame_size_is_stable() {
        assert_eq!(core::mem::size_of::<EventFrame>(), 148);
    }

    #[test]
    fn ping_request_frame_size_is_stable() {
        assert_eq!(core::mem::size_of::<PingRequestFrame>(), 32);
    }

    #[test]
    fn ground_state_frame_size_is_stable() {
        assert_eq!(core::mem::size_of::<GroundStateFrame>(), 92);
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
