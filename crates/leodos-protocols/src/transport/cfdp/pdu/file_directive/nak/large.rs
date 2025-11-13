use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::FileDirectivePdu;
use crate::transport::cfdp::pdu::file_directive::nak::NakSegmentRequest;
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
use zerocopy::network_endian::U64;

/// A zero-copy representation of the data field of a NAK PDU for **large files**.
///
/// ```text
/// +------------------------------------+-----------+--------------------------------------+
/// | Field Name                         | Size      | Notes                                |
/// +------------------------------------+-----------+--------------------------------------+
/// | Start of scope                     | 64 bits   | FSS field.                           |
/// | End of scope                       | 64 bits   | FSS field.                           |
/// | -- Start of `rest` slice --------- | --------- | ------------------------------------ |
/// | Segment Requests                   | Variable  | Zero or more `NakSegmentSmall`s.     |
/// +------------------------------------+-----------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct NakPduLarge {
    start_of_scope: U64,
    end_of_scope: U64,
    /// Contains a sequence of zero or more `NakSegmentLarge` structs.
    rest: [u8],
}

impl NakPduLarge {
    /// Parses the `rest` slice into a slice of `NakSegmentLarge`s.
    pub fn segment_requests(&self) -> Result<&[NakSegmentLarge], CfdpError> {
        <[NakSegmentLarge]>::ref_from_bytes(&self.rest)
            .map_err(|_| CfdpError::Custom("Invalid NAK segment requests"))
    }

    /// Get the start_of_scope field as a u64.
    pub fn start_of_scope(&self) -> u64 {
        self.start_of_scope.get()
    }
    pub fn set_start_of_scope(&mut self, scope: u64) {
        self.start_of_scope.set(scope);
    }

    /// Get the end_of_scope field as a u64.
    pub fn end_of_scope(&self) -> u64 {
        self.end_of_scope.get()
    }
    pub fn set_end_of_scope(&mut self, scope: u64) {
        self.end_of_scope.set(scope);
    }

    /// Get the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

/// A `zerocopy`-compatible struct representing a single missing segment in a NAK PDU
/// for a **large file** transaction (64-bit offsets).
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct NakSegmentLarge {
    /// Start offset of the missing data segment.
    start_offset: U64,
    /// End offset of the missing data segment.
    end_offset: U64,
}

impl NakSegmentLarge {
    /// Get the start_offset field as a u64.
    pub fn start_offset(&self) -> u64 {
        self.start_offset.get()
    }
    pub fn set_start_offset(&mut self, offset: u64) {
        self.start_offset.set(offset);
    }

    /// Get the end_offset field as a u64.
    pub fn end_offset(&self) -> u64 {
        self.end_offset.get()
    }
    pub fn set_end_offset(&mut self, offset: u64) {
        self.end_offset.set(offset);
    }
}

#[bon]
impl NakPduLarge {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        crc_flag: bool,
        start_of_scope: u64,
        end_of_scope: u64,
        segment_requests: &'a [NakSegmentRequest],
    ) -> Result<&'a mut Pdu, CfdpError> {
        let fixed_part_len = size_of::<U32>() * 2;
        let segments_len = segment_requests.len() * size_of::<NakSegmentLarge>();
        let specific_data_len = fixed_part_len + segments_len;
        let data_field_len = (1 + specific_data_len) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileDirective)
            .direction(Direction::TowardSender)
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(true)
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

        let data_field = pdu.data_field_mut().or_else(|_| {
            Err(CfdpError::Custom(
                "Failed to get mutable data field for NAK PDU",
            ))
        })?;
        let directive_pdu = FileDirectivePdu::mut_from_bytes(data_field).or_else(|_| {
            Err(CfdpError::Custom(
                "Failed to get mutable directive PDU for NAK PDU",
            ))
        })?;
        directive_pdu.set_directive_code(DirectiveCode::Nak);

        let nak_pdu =
            NakPduLarge::mut_from_bytes_with_elems(&mut directive_pdu.rest, specific_data_len)
                .or_else(|_| Err(CfdpError::Custom("Failed to build NAK PDU")))?;
        nak_pdu.set_start_of_scope(start_of_scope);
        nak_pdu.set_end_of_scope(end_of_scope);

        let segments_slice =
            <[NakSegmentLarge]>::mut_from_bytes(&mut nak_pdu.rest).or_else(|_| {
                Err(CfdpError::Custom(
                    "Failed to get mutable segment requests slice for NAK PDU",
                ))
            })?;
        for (req, seg) in segment_requests.iter().zip(segments_slice.iter_mut()) {
            seg.set_start_offset(req.start_offset);
            seg.set_end_offset(req.end_offset);
        }

        Ok(pdu)
    }
}
