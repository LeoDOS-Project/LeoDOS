use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::FileDirectivePdu;
use crate::transport::cfdp::pdu::header::Direction;
use crate::transport::cfdp::pdu::header::PduType;
use crate::transport::cfdp::pdu::header::TransmissionMode;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy representation of the data field of a Prompt PDU.
///
/// This struct's layout strictly follows the CCSDS specification (Table 5-12).
/// It has a fixed size of 2 bytes.
///
/// ```text
/// +------------------------------------+----------+------------------------------------+
/// | Field Name                         | Size     | Notes                              |
/// +------------------------------------+----------+------------------------------------+
/// | Response required                  | 1 bit    | 0=NAK, 1=Keep Alive                |
/// | Spare                              | 7 bits   | (Both are packed into one octet)   |
/// +------------------------------------+----------+------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct PromptPdu {
    /// Packed byte containing the response-required bit and spare bits.
    packed_flags: u8,
}

#[rustfmt::skip]
/// Bit masks for the Prompt PDU's packed fields.
mod bitmasks {
    /// Mask for the 1-bit response required flag.
    pub const PROMPT_RESPONSE_REQUIRED_MASK: u8 = 0b_10000000;
    /// Mask for the 7-bit spare field (unused).
    pub const _PROMPT_RESPONSE_RESERVED: u8 =     0b_01111111;
}

use bitmasks::*;

/// The type of response requested by a Prompt PDU.
#[derive(Debug, PartialEq, Eq)]
pub enum PromptResponse {
    /// Request a Keep Alive response.
    KeepAlive,
    /// Request a NAK response.
    Nak,
}

impl PromptPdu {
    /// Returns whether a response is required for this Prompt PDU.
    pub fn prompt_response(&self) -> PromptResponse {
        if get_bits_u8(self.packed_flags, PROMPT_RESPONSE_REQUIRED_MASK) == 1 {
            PromptResponse::KeepAlive
        } else {
            PromptResponse::Nak
        }
    }

    /// Sets the response type for this Prompt PDU.
    pub fn set_prompt_response(&mut self, response: PromptResponse) {
        let response_bit = match response {
            PromptResponse::Nak => 0,
            PromptResponse::KeepAlive => 1,
        };
        set_bits_u8(
            &mut self.packed_flags,
            PROMPT_RESPONSE_REQUIRED_MASK,
            response_bit,
        );
    }
}

#[bon]
impl PromptPdu {
    /// Builds a new Prompt PDU in the given buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        crc_flag: bool,
        response_required: PromptResponse,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let data_field_len = (DirectiveCode::size() + size_of::<Self>()) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileDirective)
            .direction(Direction::TowardReceiver) // Prompt is always sent to the receiver
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(false) // Not relevant
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
        let directive_pdu = FileDirectivePdu::mut_from_bytes(data_field).unwrap();
        directive_pdu.set_directive_code(DirectiveCode::Prompt);

        let prompt_pdu = PromptPdu::mut_from_bytes(&mut directive_pdu.rest).unwrap();
        let response_bit = match response_required {
            PromptResponse::Nak => 0,
            PromptResponse::KeepAlive => 1,
        };
        prompt_pdu.packed_flags = response_bit << 7;

        Ok(pdu)
    }
}
