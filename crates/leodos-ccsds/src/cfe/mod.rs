//! CFE-specific packet definitions, views, and builders.
//!
//! This module provides the top-level API for working with CFE-compliant
//! `Telecommand` and `Telemetry` packets.

pub mod tc;
pub mod tm;

use crate::spp::PacketType;
use crate::spp::SecondaryHeaderFlag;
use crate::spp::SpacePacket;
use crate::spp::SpacePacketData;
use crate::cfe::tc::Telecommand;
use crate::cfe::tm::Telemetry;

use zerocopy::FromBytes;
use zerocopy::IntoBytes;

#[derive(Debug)]
pub enum CfeError {
    MissingSecondaryHeader,
    PayloadMismatch,
    TypeMismatch,
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
