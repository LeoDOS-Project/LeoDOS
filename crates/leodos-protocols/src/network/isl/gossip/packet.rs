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
use crate::network::isl::address::RawAddress;
use crate::network::spp::Apid;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SequenceCount;
use crate::utils::checksum_u8;
use crate::utils::validate_checksum_u8;

/// A zero-copy view over a complete `GossipTelecommand` in a raw byte buffer.
///
/// This is a specialized `Telecommand` where the payload contains a `GossipHeader`
/// that provides information for duplicate detection (`epoch`) and sender
/// identification, followed by the actual data being gossiped.
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
/// | Originator Address              | 2 bytes   |
/// | From Address                    | 2 bytes   |
/// | Service Area Min                | 1 byte    |
/// | Service Area Max                | 1 byte    |
/// | Epoch                           | 2 bytes   |
/// | Action Code                     | 1 byte    |
/// |                                 |           |
/// | -- Gossip Payload (Variable) -- | --------- |
/// |                                 |           |
/// | Data being Gossiped             | 0-65524 B |
/// +---------------------------------+-----------+
/// ```
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct IslGossipTelecommand {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub(crate) gossip_header: IslGossipHeader,
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
pub(crate) struct IslGossipHeader {
    /// Address of the node that originated this gossip.
    pub originator: RawAddress,
    /// Address of the immediate sender (for routing - don't echo back).
    pub from_address: RawAddress,
    /// The minimum satellite ID in the target service area for this gossip.
    pub service_area_min: u8,
    /// The maximum satellite ID in the target service area for this gossip.
    pub service_area_max: u8,
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

impl IslGossipHeader {
    pub(crate) fn originator(&self) -> Address {
        self.originator.parse()
    }

    pub(crate) fn from_address(&self) -> Address {
        self.from_address.parse()
    }
}

impl IslGossipTelecommand {
    /// Builder for creating a complete ISL Gossip message.
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        function_code: u8,
        originator: Address,
        from_address: Address,
        service_area_min: u8,
        service_area_max: u8,
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

        let tc_payload_len = size_of::<IslGossipHeader>() + payload_len;
        let tc = Telecommand::builder()
            .buffer(buffer)
            .apid(apid)
            .sequence_count(SequenceCount::new())
            .function_code(function_code)
            .payload_len(tc_payload_len)
            .build()
            .map_err(GossipMessageError::Cfe)?;

        let buffer = tc.as_mut_bytes();
        let provided_len = buffer.len();
        let gossip_tc = Self::mut_from_bytes_with_elems(buffer, payload_len).map_err(|_| {
            GossipMessageError::Cfe(TelecommandError::BufferTooSmall {
                required_len: size_of::<PrimaryHeader>()
                    + size_of::<TelecommandSecondaryHeader>()
                    + tc_payload_len,
                provided_len,
            })
        })?;

        gossip_tc.gossip_header.originator = RawAddress::from(originator);
        gossip_tc.gossip_header.from_address = RawAddress::from(from_address);
        gossip_tc.gossip_header.service_area_min = service_area_min;
        gossip_tc.gossip_header.service_area_max = service_area_max;
        gossip_tc.gossip_header.epoch = epoch;
        gossip_tc.gossip_header.action_code = action_code;

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
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    pub fn data_len(&self) -> usize {
        self.payload.len()
    }
}
