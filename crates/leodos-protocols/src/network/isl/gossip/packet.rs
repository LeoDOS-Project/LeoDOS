use bon::bon;
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
/// | Origin Address                  | 2 bytes   |
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
    /// CCSDS SPP primary header.
    pub primary: PrimaryHeader,
    /// CFE command secondary header.
    pub secondary: TelecommandSecondaryHeader,
    /// ISL-specific gossip header.
    pub gossip_header: IslGossipHeader,
    /// Variable-length gossip data payload.
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

/// A gossip epoch number used for duplicate detection.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, IntoBytes, FromBytes, Immutable, KnownLayout, Unaligned,
)]
#[repr(transparent)]
pub struct Epoch(pub U16);

/// The ISL-specific header for a gossip message.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct IslGossipHeader {
    origin: RawAddress,
    predecessor: RawAddress,
    pub(crate) service_area_min: u8,
    pub(crate) service_area_max: u8,
    epoch: Epoch,
}

/// An error that can occur when building or parsing a Gossip message.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum GossipMessageError {
    /// An error from the underlying CFE telecommand layer.
    #[error("CFE Telecommand error: {0}")]
    Cfe(#[from] TelecommandError),
    /// The payload is too small to contain a gossip header.
    #[error("Payload too small to contain a valid gossip header")]
    PayloadTooSmall,
    /// The payload exceeds the maximum allowed size.
    #[error(
        "Payload too large: maximum allowed is {max} bytes, but {provided} bytes were provided"
    )]
    PayloadTooLarge {
        /// Maximum allowed payload size.
        max: usize,
        /// Actual payload size provided.
        provided: usize,
    },
}

impl IslGossipHeader {
    /// Address of the node that originated this gossip.
    pub fn origin(&self) -> Address {
        self.origin.parse()
    }

    /// Sets the origin address in the gossip header.
    pub fn set_origin(&mut self, addr: Address) {
        self.origin = RawAddress::from(addr);
    }

    /// Address of the immediate sender (for routing — don't echo back).
    pub fn predecessor(&self) -> Address {
        self.predecessor.parse()
    }

    /// Sets the from address in the gossip header.
    pub fn set_predecessor(&mut self, addr: Address) {
        self.predecessor = RawAddress::from(addr);
    }

    /// The unique sequence number for this piece of gossip, used for duplicate detection.
    pub fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Sets the epoch number in the gossip header.
    pub fn set_epoch(&mut self, epoch: Epoch) {
        self.epoch = epoch;
    }

    /// The maximum service area (in hops) for this gossip. Nodes with a service area
    /// greater than this value should not forward the gossip further.
    pub fn service_area_min(&self) -> u8 {
        self.service_area_min
    }

    /// Sets the minimum service area in the gossip header.
    pub fn set_service_area_min(&mut self, area: u8) {
        self.service_area_min = area;
    }

    /// The maximum service area (in hops) for this gossip. Nodes with a service area
    /// greater than this value should not forward the gossip further.
    pub fn service_area_max(&self) -> u8 {
        self.service_area_max
    }

    /// Sets the maximum service area in the gossip header.
    pub fn set_service_area_max(&mut self, area: u8) {
        self.service_area_max = area;
    }
}

#[bon]
impl IslGossipTelecommand {
    /// Builder for creating a complete ISL Gossip message.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        function_code: u8,
        origin: Address,
        predecessor: Address,
        service_area_min: u8,
        service_area_max: u8,
        epoch: Epoch,
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

        gossip_tc.gossip_header.set_origin(origin);
        gossip_tc.gossip_header.set_predecessor(predecessor);
        gossip_tc.gossip_header.service_area_min = service_area_min;
        gossip_tc.gossip_header.service_area_max = service_area_max;
        gossip_tc.gossip_header.set_epoch(epoch);

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

    /// Calculates and sets the 8-bit cFE checksum for this gossip packet.
    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    /// Returns `true` if the cFE checksum is valid.
    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    /// Returns the length of the gossip data payload in bytes.
    pub fn data_len(&self) -> usize {
        self.payload.len()
    }
}
