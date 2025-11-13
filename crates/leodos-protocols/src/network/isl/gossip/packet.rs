use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;

use crate::network::cfe::tc::Telecommand;
use crate::network::cfe::tc::TelecommandError;
use crate::network::cfe::tc::TelecommandSecondaryHeader;
use crate::network::isl::address::Address;
use crate::network::isl::address::OrbitId;
use crate::network::isl::address::SatelliteId;
use crate::network::isl::address::SpacecraftId;
use crate::network::spp::Apid;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SequenceCount;
use crate::utils::checksum_u8;
use crate::utils::validate_checksum_u8;

/// A zero-copy view over a complete `GossipTelecommand` in a raw byte buffer.
///
/// This is a specialized `Telecommand` where the payload contains a `GossipHeader`
/// that provides information for duplicate detection (`epoch`) and sender
/// identification (`from_orb`/`from_sat`), followed by the actual data being gossiped.
///
/// ```text
/// +---------------------------------+-----------+
/// | Field Name                      | Size      |
/// +---------------------------------+-----------+
/// + -- cFE Telecommand Hdrs ------- | --------- |
/// |                                 |           |
/// | Primary Header                  | 6 bytes   |
/// | Secondary Header                | 2 bytes   |
/// |                                 |           |
/// + -- Gossip Header -------------- | --------- |
/// |                                 |           |
/// | Spacecraft ID                   | 1 byte    |
/// | From OrbitID                    | 1 byte    |
/// | From SatelliteID                | 1 byte    |
/// | Service Area Min                | 1 byte    |
/// | Service Area Max                | 1 byte    |
/// | Epoch                           | 2 bytes   |
/// | Action Code                     | 1 byte    |
/// |                                 |           |
/// | -- Gossip Payload (Variable) -- | --------- |
/// |                                 |           |
/// | Data being Gossiped             | 0-65525 B |
/// +---------------------------------+-----------+
/// ```
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct IslGossipTelecommand {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub gossip_header: IslGossipHeader,
    pub payload: [u8],
}

impl core::fmt::Debug for IslGossipTelecommand {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GossipTelecommand")
            .field("primary", &self.primary)
            .field("secondary", &self.secondary)
            .field("gossip_header", &self.gossip_header)
            .field("payload_len", &self.payload.len())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable)]
#[repr(transparent)]
pub struct Epoch(pub U16);

/// The ISL-specific header for a gossip message.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct IslGossipHeader {
    /// ID of the spacecraft that originated the gossip packet.
    pub spacecraft_id: SpacecraftId,
    /// Address of the immediate sender (for routing).
    pub from_address: Address,
    /// The minimum satellite ID in the target service area for this gossip.
    pub service_area_min: SatelliteId,
    /// The maximum satellite ID in the target service area for this gossip.
    pub service_area_max: SatelliteId,
    /// The unique sequence number for this piece of gossip, used for duplicate detection.
    pub epoch: Epoch,
    /// The application-specific action code for the gossip message.
    pub action_code: u8,
}

/// An error that can occur when building or parsing a Gossip message.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum GossipMessageError {
    Cfe(TelecommandError),
    PayloadTooSmall,
    PayloadTooLarge { max: usize, provided: usize },
}

impl From<TelecommandError> for GossipMessageError {
    fn from(e: TelecommandError) -> Self {
        GossipMessageError::Cfe(e)
    }
}

impl IslGossipTelecommand {
    /// Builder for creating a complete ISL Gossip message.
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        function_code: u8,
        spacecraft_id: SpacecraftId,
        from_orb: OrbitId,
        from_sat: SatelliteId,
        service_area_min: SatelliteId,
        service_area_max: SatelliteId,
        epoch: Epoch,
        action_code: u8,
        payload_len: usize,
    ) -> Result<&'a mut Self, GossipMessageError> {
        if payload_len > u16::MAX as usize {
            return Err(GossipMessageError::PayloadTooLarge {
                max: u16::MAX as usize,
                provided: payload_len,
            });
        }

        let payload_len = size_of::<IslGossipHeader>() + payload_len;
        let tc = Telecommand::builder()
            .buffer(buffer)
            .apid(apid)
            .sequence_count(SequenceCount::new())
            .function_code(function_code)
            .payload_len(payload_len)
            .build()
            .map_err(GossipMessageError::Cfe)?;

        let required_len =
            size_of::<PrimaryHeader>() + size_of::<TelecommandSecondaryHeader>() + payload_len;

        let buffer = tc.as_mut_bytes();
        let provided_len = buffer.len();
        let gossip_tc = Self::mut_from_bytes_with_elems(buffer, required_len).map_err(|_| {
            GossipMessageError::Cfe(TelecommandError::BufferTooSmall {
                required_len,
                provided_len,
            })
        })?;

        // Copy the provided header and payload into place.
        gossip_tc.gossip_header.spacecraft_id = spacecraft_id;
        gossip_tc.gossip_header.from_address = Address::new(from_orb, from_sat);
        gossip_tc.gossip_header.service_area_min = service_area_min;
        gossip_tc.gossip_header.service_area_max = service_area_max;
        gossip_tc.gossip_header.epoch = epoch;
        gossip_tc.gossip_header.action_code = action_code;

        // Calculate the final checksum.
        gossip_tc.set_cfe_checksum();

        Ok(gossip_tc)
    }

    /// Safely parses a generic `Telecommand` as a `GossipTelecommand`.
    pub fn from_telecommand(tc: &Telecommand) -> Result<&Self, GossipMessageError> {
        if tc.payload().len() < size_of::<IslGossipHeader>() {
            return Err(GossipMessageError::PayloadTooSmall);
        }
        Ok(Self::ref_from_bytes(tc.as_bytes()).unwrap())
    }

    pub fn set_cfe_checksum(&mut self) {
        self.secondary.checksum = 0;
        self.secondary.checksum = checksum_u8(self.as_bytes());
    }

    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    pub fn data_len(&self) -> usize {
        self.payload.len()
    }
}
