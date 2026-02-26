//! Defines the Inter-Satellite Link (ISL) message structure and builders.
//!
//! This module builds upon the `Telecommand` structure to create a specialized
//! packet type for the ISL protocol, which includes routing and application-level
//! action information.

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
use bon::bon;
use core::mem::size_of;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy view over a complete `IslTelecommand` in a raw byte buffer.
///
/// This represents the highest level of packet specialization in the protocol stack.
/// It is a `Telecommand` where the payload is guaranteed to contain a structured
/// `IslHeader` followed by the variable-length `payload` for the application.
///
/// ```text
/// +----------------------------+-----------+
/// | Field Name                 | Size      |
/// +----------------------------+-----------+
/// + -- cFE Telecommand Hdrs -- | --------- |
/// |                            |           |
/// | Primary Header             | 6 bytes   |
/// | Secondary Header           | 2 bytes   |
/// |                            |           |
/// | -- ISL Header ------------ | --------- |
/// |                            |           |
/// | Message ID                 | 1 byte    |
/// | Target Orbit               | 1 byte    |
/// | Target Satellite           | 1 byte    |
/// | Action Code                | 1 byte    |
/// |                            |           |
/// | -- Payload (Variable) ---- | --------- |
/// |                            |           |
/// | Application Data           | 0-65528 B |
/// +----------------------------+-----------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct IslRoutingTelecommand {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub(crate) isl_header: IslRoutingTelecommandHeader,
    pub payload: [u8],
}

/// The ISL-specific header that contains routing and action information.
/// This structure is placed at the beginning of the `Telecommand`'s payload.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub(crate) struct IslRoutingTelecommandHeader {
    target: RawAddress,
    message_id: u8,
    action_code: u8,
}

/// An error that can occur when building or parsing an ISL message.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum IslMessageError {
    /// An error occurred during the underlying CFE Telecommand construction.
    Cfe(TelecommandError),
    /// A received packet was expected to be an ISL message but its payload was
    /// too small to contain a valid `IslHeader`.
    PayloadTooSmall,
    PayloadTooLarge {
        max: usize,
        provided: usize,
    },
}

impl From<TelecommandError> for IslMessageError {
    fn from(e: TelecommandError) -> Self {
        IslMessageError::Cfe(e)
    }
}

#[bon]
impl IslRoutingTelecommandHeader {
    #[builder]
    pub(crate) fn new(message_id: u8, target: Address, action_code: u8) -> Self {
        Self {
            message_id,
            target: RawAddress::from(target),
            action_code,
        }
    }

    pub(crate) fn target(&self) -> Address {
        self.target.parse()
    }

    pub(crate) fn set_target(&mut self, target: Address) {
        self.target = RawAddress::from(target);
    }

    pub(crate) fn message_id(&self) -> u8 {
        self.message_id
    }

    pub(crate) fn set_message_id(&mut self, message_id: u8) {
        self.message_id = message_id;
    }

    pub(crate) fn action_code(&self) -> u8 {
        self.action_code
    }

    pub(crate) fn set_action_code(&mut self, action_code: u8) {
        self.action_code = action_code;
    }
}

#[bon]
impl IslRoutingTelecommand {
    /// A high-level builder for creating a complete, routable ISL message.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        function_code: u8,
        message_id: u8,
        target: Address,
        action_code: u8,
        payload_len: usize,
    ) -> Result<&'a mut Self, IslMessageError> {
        let tc = Telecommand::builder()
            .buffer(buffer)
            .apid(apid)
            .sequence_count(SequenceCount::new())
            .function_code(function_code)
            .payload_len(size_of::<IslRoutingTelecommandHeader>() + payload_len)
            .build()
            .map_err(IslMessageError::Cfe)?;

        let buffer = tc.as_mut_bytes();
        let provided_len = buffer.len();
        let isl_tc = Self::mut_from_bytes_with_elems(buffer, payload_len).map_err(|_| {
            TelecommandError::BufferTooSmall {
                required_len: size_of::<PrimaryHeader>()
                    + size_of::<TelecommandSecondaryHeader>()
                    + size_of::<IslRoutingTelecommandHeader>()
                    + payload_len,
                provided_len,
            }
        })?;

        isl_tc.isl_header.set_message_id(message_id);
        isl_tc.isl_header.set_target(target);
        isl_tc.isl_header.set_action_code(action_code);

        isl_tc.set_cfe_checksum();

        Ok(isl_tc)
    }
}

impl IslRoutingTelecommand {
    /// Calculates and sets the 8-bit cFE checksum for this command packet.
    ///
    /// The algorithm is a byte-wise XOR sum of the entire packet,
    /// with the checksum field itself treated as zero during calculation.
    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    /// Validates the 8-bit cFE checksum.
    ///
    /// Returns `true` if the checksum is valid, `false` otherwise.
    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    /// Safely parses a generic `Telecommand` as an `IslTelecommand`.
    pub fn from_telecommand(tc: &Telecommand) -> Result<&Self, IslMessageError> {
        if tc.payload().len() < size_of::<IslRoutingTelecommandHeader>() {
            return Err(IslMessageError::PayloadTooSmall);
        }
        // The layouts are compatible, so we can safely cast.
        Ok(Self::ref_from_bytes(tc.as_bytes()).unwrap())
    }

    /// Returns a reference to the underlying `Telecommand` view.
    pub fn as_telecommand(&self) -> &Telecommand {
        Telecommand::ref_from_bytes(self.as_bytes()).unwrap()
    }

    /// Returns a slice containing the application-specific data.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.payload
    }

    pub fn parse<'a>(bytes: &'a [u8]) -> Result<&'a IslRoutingTelecommand, IslMessageError> {
        let tc = Telecommand::parse(bytes).map_err(IslMessageError::Cfe)?;
        Self::from_telecommand(tc)
    }
}

impl crate::utils::Header<PrimaryHeader> for IslRoutingTelecommand {
    fn get(&self) -> &PrimaryHeader {
        &self.primary
    }
    fn get_mut(&mut self) -> &mut PrimaryHeader {
        &mut self.primary
    }
}
