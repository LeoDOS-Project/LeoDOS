use crate::transport::cfdp::pdu::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::TransactionSeqNum;
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

/// A zero-copy representation of the data field of a File Data PDU that
/// **does not** have segment metadata.
///
/// It consists of a `rest` slice containing the FSS `Offset` and the `file_data`.
/// ```
/// +------------------------------------+----------------+--------------------------------------+
/// | Field Name                         | Size           | Notes                                |
/// +------------------------------------+----------------+--------------------------------------+
/// | -- Start of PDU Data Field ------- | -------------- | ------------------------------------ |
/// | Offset                             | 32 or 64 bits  | FSS field. Byte offset into the file.|
/// |                                    | (FSS)          | Size depends on PDU Header's         |
/// |                                    |                | `Large File Flag`.                   |
/// | File data                          | Variable       | A chunk of the file's content.       |
/// +------------------------------------+----------------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct FileDataPduWithoutMeta {
    /// Contains the FSS `Offset` followed by the file data.
    rest: [u8],
}

impl FileDataPduWithoutMeta {
    /// Parses the FSS `Offset` from the start of the `rest` slice.
    pub fn offset(&self, large_file_flag: bool) -> Result<u64, CfdpError> {
        if large_file_flag {
            U64::ref_from_prefix(&self.rest)
                .map(|(r, _)| r.get())
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        } else {
            U32::ref_from_prefix(&self.rest)
                .map(|(r, _)| r.get() as u64)
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        }
    }
    pub fn set_offset(&mut self, offset: u64, large_file_flag: bool) -> Result<(), CfdpError> {
        if large_file_flag {
            U64::mut_from_prefix(&mut self.rest)
                .map(|(r, _)| r.set(offset))
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        } else {
            if offset > u32::MAX as u64 {
                return Err(CfdpError::DataTooLarge {
                    field: "offset",
                    max: u32::MAX as usize,
                });
            }
            U32::mut_from_prefix(&mut self.rest)
                .map(|(r, _)| r.set(offset as u32))
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        }
    }

    /// Returns the slice containing the actual file data.
    pub fn file_data(&self, large_file_flag: bool) -> Result<&[u8], CfdpError> {
        let offset_len = if large_file_flag { 8 } else { 4 };
        self.rest
            .get(offset_len..)
            .ok_or_else(|| CfdpError::Custom("Invalid file data slice"))
    }
    pub fn file_data_mut(&mut self, large_file_flag: bool) -> Result<&mut [u8], CfdpError> {
        let offset_len = if large_file_flag { 8 } else { 4 };
        self.rest
            .get_mut(offset_len..)
            .ok_or_else(|| CfdpError::Custom("Invalid file data slice"))
    }

    /// Get the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

#[bon]
impl FileDataPduWithoutMeta {
    /// Builds a complete File Data PDU (without metadata).
    /// The caller is responsible for writing the file data into the buffer after this function returns.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        // Transaction Context
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        large_file_flag: bool,
        crc_flag: bool,
        // FileData Specific
        segmentation_control: bool,
        offset: u64,
        file_data_len: usize,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let offset_len = if large_file_flag { 8 } else { 4 };
        let data_field_len = (offset_len + file_data_len) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileData)
            .direction(Direction::TowardReceiver)
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(large_file_flag)
            .data_field_len(data_field_len)
            .seg_ctrl(segmentation_control)
            .seg_meta_flag(false) // No metadata for this PDU type
            .build()?;

        let pdu = Pdu::builder()
            .buffer(buffer)
            .header_fixed(header)
            .source_entity_id(source_entity_id)
            .destination_entity_id(dest_entity_id)
            .transaction_seq_num(transaction_seq_num)
            .build()?;

        // The header is complete. Now write the offset into the data field.
        let data_field = pdu.data_field_mut()?;
        let actual_data_field_len = data_field.len();
        let fd_pdu = FileDataPduWithoutMeta::mut_from_bytes(data_field).map_err(|_| {
            CfdpError::BufferTooSmall {
                required: data_field_len as usize,
                provided: actual_data_field_len,
            }
        })?;
        fd_pdu.set_offset(offset, large_file_flag)?;

        // The PDU is now fully constructed up to the point where file data should be written.
        // The caller can get `pdu.data_field_mut()` and write into the slice starting at `offset_len`.
        Ok(pdu)
    }
}
