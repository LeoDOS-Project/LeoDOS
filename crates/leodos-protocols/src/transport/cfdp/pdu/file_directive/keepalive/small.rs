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

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U32;

/// A zero-copy representation of a Keep Alive PDU for **small files**.
///
/// This struct's layout strictly follows the CCSDS specification (Table 5-13)
/// for a transaction where the `Large File Flag` in the header is `0`.
///
/// ```text
/// +------------------------------------+----------+------------------------------------+
/// | Field Name                         | Size     | Notes                              |
/// +------------------------------------+----------+------------------------------------+
/// | Progress                           | 32 bits  | FSS field, 32-bit version.         |
/// +------------------------------------+----------+------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct KeepAlivePduSmall {
    progress: U32,
}

impl KeepAlivePduSmall {
    /// Get the progress field as a u32.
    pub fn progress(&self) -> u32 {
        self.progress.get()
    }
    pub fn set_progress(&mut self, progress: u32) {
        self.progress.set(progress);
    }
}

#[bon]
impl KeepAlivePduSmall {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        crc_flag: bool,
        progress: u32,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let data_field_len = (DirectiveCode::size() + size_of::<Self>()) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileDirective)
            .direction(Direction::TowardSender)
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(false) // Small version
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
        directive_pdu.set_directive_code(DirectiveCode::KeepAlive);

        let keepalive_pdu = KeepAlivePduSmall::mut_from_bytes(&mut directive_pdu.rest).unwrap();
        keepalive_pdu.set_progress(progress);

        Ok(pdu)
    }
}
