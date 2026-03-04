use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::pdu::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::FileDirectivePdu;
use crate::transport::cfdp::pdu::header::Direction;
use crate::transport::cfdp::pdu::header::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::header::PduType;
use crate::transport::cfdp::pdu::header::TransmissionMode;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// A zero-copy representation of the **data field** of an Acknowledgment (ACK) PDU.
///
/// This struct's layout strictly follows the CCSDS specification (Table 5-8).
/// It consists entirely of fixed-size fields, with several values bit-packed
/// across two octets.
///
/// ```text
/// +------------------------------------+----------+--------------------------------------+
/// | Field Name                         | Size     | Notes                                |
/// +------------------------------------+----------+--------------------------------------+
/// | Directive code of acked PDU        | 4 bits   | e.g., 0x04 for EOF, 0x05 for Fin.    |
/// | Directive subtype code             | 4 bits   | (Both are packed into one octet)     |
/// |                                    |          |                                      |
/// | Condition code                     | 4 bits   | Condition code of the acked PDU.     |
/// | Reserved for future use            | 2 bits   |                                      |
/// | Transaction status                 | 2 bits   | (All three are packed into one octet)|
/// +------------------------------------+----------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Debug, PartialEq, Eq)]
pub struct AckPdu {
    /// An 8-bit field containing the 4-bit `Directive code of acknowledged PDU`
    /// and the 4-bit `Directive subtype code`.
    packed_codes: u8,
    /// An 8-bit field containing the 4-bit `Condition code`, 2 `Spare` bits,
    /// and the 2-bit `Transaction status`.
    packed_status: u8,
}

#[rustfmt::skip]
/// Bit masks for the ACK PDU's packed fields.
mod bitmasks {
    /// Mask for the 4-bit directive code of the acknowledged PDU.
    pub const ACK_DIR_CODE_MASK: u8 =         0b_11110000;
    /// Mask for the 4-bit directive subtype code.
    pub const ACK_DIR_SUBTYPE_CODE_MASK: u8 = 0b_00001111;
    /// Mask for the 4-bit condition code.
    pub const ACK_CC_MASK: u8 =                 0b_11110000;
    /// Mask for the 2-bit reserved field (unused).
    pub const _ACK_RESERVED_MASK: u8 =          0b_00001100;
    /// Mask for the 2-bit transaction status.
    pub const ACK_TRANSACTION_STATUS_MASK: u8 = 0b_00000011;
}

use bitmasks::*;

/// The status of a transaction at a remote entity, as reported in an ACK PDU.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum TransactionStatus {
    /// Transaction status is undefined.
    Undefined = 0b00,
    /// Transaction is currently active.
    Active = 0b01,
    /// Transaction has been terminated.
    Terminated = 0b10,
    /// Transaction is unrecognized by the remote entity.
    Unrecognized = 0b11,
}

impl TryFrom<u8> for TransactionStatus {
    type Error = CfdpError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        let val = match val {
            0b00 => TransactionStatus::Undefined,
            0b01 => TransactionStatus::Active,
            0b10 => TransactionStatus::Terminated,
            0b11 => TransactionStatus::Unrecognized,
            _ => return Err(CfdpError::Custom("Invalid TransactionStatus value")),
        };
        Ok(val)
    }
}

/// Identifies which directive is being acknowledged.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum AckedDirectiveCode {
    /// Acknowledging an EOF PDU.
    Eof,
    /// Acknowledging a Finished PDU.
    Finished,
}

impl TryFrom<u8> for AckedDirectiveCode {
    type Error = CfdpError;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        let val = match val {
            0x04 => AckedDirectiveCode::Eof,
            0x05 => AckedDirectiveCode::Finished,
            _ => return Err(CfdpError::Custom("Invalid AckedPduDirectiveCode value")),
        };
        Ok(val)
    }
}

impl AckPdu {
    /// Returns the directive code of the PDU that is being acknowledged (e.g., EOF, Finished).
    fn directive_code_of_acked_pdu(&self) -> Result<AckedDirectiveCode, CfdpError> {
        AckedDirectiveCode::try_from(get_bits_u8(self.packed_codes, ACK_DIR_CODE_MASK))
    }
    /// Sets the directive code of the PDU being acknowledged.
    fn set_directive_code_of_acked_pdu(&mut self, code: AckedDirectiveCode) {
        set_bits_u8(&mut self.packed_codes, ACK_DIR_CODE_MASK, code as u8);
    }

    /// Returns the directive subtype code.
    fn directive_subtype_code(&self) -> u8 {
        get_bits_u8(self.packed_codes, ACK_DIR_SUBTYPE_CODE_MASK)
    }
    /// Sets the directive subtype code.
    fn set_directive_subtype_code(&mut self, subtype: u8) {
        set_bits_u8(&mut self.packed_codes, ACK_DIR_SUBTYPE_CODE_MASK, subtype);
    }

    /// Interprets the packed codes to determine what specific PDU is being acknowledged.
    pub fn acked_directive_code(&self) -> Result<AckedDirectiveCode, CfdpError> {
        let directive_code = self.directive_code_of_acked_pdu()?;
        let subtype_code = self.directive_subtype_code();
        match (directive_code, subtype_code) {
            (AckedDirectiveCode::Eof, 0) => Ok(AckedDirectiveCode::Eof),
            (AckedDirectiveCode::Finished, 1) => Ok(AckedDirectiveCode::Finished),
            _ => Err(CfdpError::Custom("Unknown acknowledged PDU type")),
        }
    }

    /// Returns the condition code of the PDU that is being acknowledged.
    pub fn condition_code(&self) -> Result<ConditionCode, CfdpError> {
        ConditionCode::try_from(get_bits_u8(self.packed_status, ACK_CC_MASK))
    }
    /// Sets the condition code of the acknowledged PDU.
    pub fn set_condition_code(&mut self, code: ConditionCode) {
        set_bits_u8(&mut self.packed_status, ACK_CC_MASK, code as u8);
    }

    /// Returns the status of the transaction at the acknowledging entity.
    pub fn transaction_status(&self) -> Result<TransactionStatus, CfdpError> {
        TransactionStatus::try_from(get_bits_u8(self.packed_status, ACK_TRANSACTION_STATUS_MASK))
    }
    /// Sets the transaction status field.
    pub fn set_transaction_status(&mut self, status: TransactionStatus) {
        set_bits_u8(
            &mut self.packed_status,
            ACK_TRANSACTION_STATUS_MASK,
            status as u8,
        );
    }
}

#[bon]
impl AckPdu {
    /// Builds a new ACK PDU in the given buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        // Transaction Context
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        crc_flag: bool,
        // ACK Specific
        acked_directive_code: AckedDirectiveCode,
        condition_code: ConditionCode,
        transaction_status: TransactionStatus,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let data_field_len = (DirectiveCode::size() + size_of::<Self>()) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileDirective)
            .direction(Direction::TowardSender)
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(false) // Not relevant for ACK
            .data_field_len(data_field_len)
            .seg_ctrl(false)
            .seg_meta_flag(false)
            .build()?;

        let pdu = Pdu::builder()
            .buffer(buffer)
            .header_fixed(header)
            .source_entity_id(source_entity_id)
            .destination_entity_id(dest_entity_id)
            .transaction_seq_num(transaction_seq_num)
            .build()?;

        let data_field = pdu.data_field_mut().unwrap();
        let directive_pdu = FileDirectivePdu::mut_from_bytes(data_field)
            .map_err(|_| CfdpError::Custom("Failed to create FileDirectivePdu"))?;
        directive_pdu.set_directive_code(DirectiveCode::Ack);

        let ack_pdu = AckPdu::mut_from_bytes(&mut directive_pdu.rest)
            .map_err(|_| CfdpError::Custom("Failed to create AckPdu"))?;
        ack_pdu.set_directive_code_of_acked_pdu(acked_directive_code);
        match acked_directive_code {
            AckedDirectiveCode::Eof => ack_pdu.set_directive_subtype_code(0),
            AckedDirectiveCode::Finished => ack_pdu.set_directive_subtype_code(1),
        }
        ack_pdu.set_condition_code(condition_code);
        ack_pdu.set_transaction_status(transaction_status);

        Ok(pdu)
    }
}
