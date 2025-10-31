//! CFE-specific packet definitions, views, and builders.
//!
//! This module provides the top-level API for working with CFE-compliant
//! `Telecommand` and `Telemetry` packets.

pub mod tc;
pub mod tc_builder;
pub mod tm;
pub mod tm_builder;

use crate::PacketType;
use crate::SecondaryHeaderFlag;
use crate::SpacePacket;
use crate::SpacePacketData;
use crate::cfe::tc::Telecommand;
use crate::cfe::tm::Telemetry;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;

pub enum CfeError {
    MissingSecondaryHeader,
    PayloadMismatch,
    TypeMismatch,
}

impl core::fmt::Debug for CfeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CfeError::MissingSecondaryHeader => {
                write!(f, "CfePacketError::MissingSecondaryHeader")
            }
            CfeError::PayloadMismatch => write!(f, "CfePacketError::PayloadMismatch"),
            CfeError::TypeMismatch => write!(f, "CfePacketError::TypeMismatch"),
        }
    }
}

impl<'a, P: SpacePacketData> TryFrom<&'a SpacePacket> for &'a Telecommand<P> {
    type Error = CfeError;

    fn try_from(sp: &'a SpacePacket) -> Result<Self, Self::Error> {
        if sp.secondary_header_flag() != SecondaryHeaderFlag::Present {
            return Err(CfeError::MissingSecondaryHeader);
        }

        let bytes = sp.as_bytes();

        match sp.packet_type() {
            PacketType::Telecommand => Telecommand::<P>::ref_from_bytes(bytes)
                .map_err(|_| CfeError::PayloadMismatch),
            PacketType::Telemetry => Err(CfeError::TypeMismatch),
        }
    }
}

impl<'a, P: SpacePacketData> TryFrom<&'a SpacePacket> for &'a Telemetry<P> {
    type Error = CfeError;

    fn try_from(sp: &'a SpacePacket) -> Result<Self, Self::Error> {
        if sp.secondary_header_flag() != SecondaryHeaderFlag::Present {
            return Err(CfeError::MissingSecondaryHeader);
        }

        let bytes = sp.as_bytes();

        match sp.packet_type() {
            PacketType::Telecommand => Err(CfeError::TypeMismatch),
            PacketType::Telemetry => Telemetry::<P>::ref_from_bytes(bytes)
                .map_err(|_| CfeError::PayloadMismatch),
        }
    }
}
