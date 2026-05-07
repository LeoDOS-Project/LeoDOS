//! Wire-format mirror of the cFE EVS event telemetry payload.
//!
//! Used to subscribe to `CFE_EVS_LONG_EVENT_MSG_MID` (0x0808) and
//! decode each broadcast event into structured fields. The layout
//! matches `CFE_EVS_LongEventTlm_Payload_t` from `cfe_evs_msgstruct.h`.

use crate::cfe::sb::msg::MsgId;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// MID of the EVS long-form event broadcast message. Every event
/// fired by `CFE_EVS_SendEvent` is published here.
pub fn long_event_mid() -> MsgId {
    MsgId::local_tlm(8)
}

/// Mirrors `CFE_EVS_PacketID_t`.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[allow(missing_docs)]
pub struct EvsPacketId {
    pub app_name: [u8; 20],
    pub event_id: zerocopy::byteorder::native_endian::U16,
    pub event_type: zerocopy::byteorder::native_endian::U16,
    pub spacecraft_id: zerocopy::byteorder::native_endian::U32,
    pub processor_id: zerocopy::byteorder::native_endian::U32,
}

/// Mirrors `CFE_EVS_LongEventTlm_Payload_t`.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy, FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
#[allow(missing_docs)]
pub struct LongEventPayload {
    pub packet_id: EvsPacketId,
    pub message: [u8; 122],
    pub spare: [u8; 2],
}

impl LongEventPayload {
    /// Returns the app name as a `&str`, trimmed at the first NUL.
    pub fn app_name_str(&self) -> &str {
        let end = self.packet_id.app_name.iter().position(|&b| b == 0).unwrap_or(20);
        core::str::from_utf8(&self.packet_id.app_name[..end]).unwrap_or("")
    }

    /// Returns the message as a `&str`, trimmed at the first NUL.
    pub fn message_str(&self) -> &str {
        let end = self.message.iter().position(|&b| b == 0).unwrap_or(122);
        core::str::from_utf8(&self.message[..end]).unwrap_or("")
    }
}
