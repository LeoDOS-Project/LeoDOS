use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::TransactionSeqNum;
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
use zerocopy::network_endian::U32;
use zerocopy::network_endian::U64;

/// A zero-copy representation of the data field of a File Data PDU that
/// **does** have segment metadata.
///
/// It consists of the administrative octet and a `rest` slice containing the
/// `segment_metadata`, the FSS `Offset`, and the `file_data`.
/// ```text
/// +------------------------------------+----------------+--------------------------------------+
/// | Field Name                         | Size           | Notes                                |
/// +------------------------------------+----------------+--------------------------------------+
/// | -- Start of PDU Data Field ------- | -------------- | ------------------------------------ |
/// | Admin Octet                        |                |                                      |
/// |   - Record continuation state      | 2 bits         | Indicates alignment with records.    |
/// |   - Segment metadata length        | 6 bits         | Length of the metadata field (0-63). |
/// | Segment metadata                   | 0-63 octets    | Application-specific metadata.       |
/// |                                    |                | Present only if length > 0.          |
/// | Offset                             | 32 or 64 bits  | FSS field. Byte offset into the file.|
/// |                                    | (FSS)          | Size depends on PDU Header's         |
/// |                                    |                | `Large File Flag`.                   |
/// | File data                          | Variable       | A chunk of the file's content.       |
/// +------------------------------------+----------------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct FileDataPduWithMeta {
    /// The administrative octet containing `Record continuation state` and `Segment metadata length`.
    admin_octet: u8,
    /// Contains the metadata, FSS `Offset`, and file data.
    rest: [u8],
}

#[rustfmt::skip]
/// Bit masks for the File Data PDU's administrative octet.
mod bitmasks {
    /// Mask for the 2-bit record continuation state.
    pub const FILE_DATA_REC_CONT_STATE_MASK: u8 = 0b_11000000;
    /// Mask for the 6-bit segment metadata length.
    pub const FILE_DATA_SEG_META_LEN_MASK: u8 =   0b_00111111;
}

use bitmasks::*;

impl FileDataPduWithMeta {
    /// Returns the 2-bit record continuation state.
    pub fn record_continuation_state(&self) -> u8 {
        get_bits_u8(self.admin_octet, FILE_DATA_REC_CONT_STATE_MASK)
    }
    /// Sets the 2-bit record continuation state.
    pub fn set_record_continuation_state(&mut self, state: u8) -> Result<(), CfdpError> {
        if state > 0b11 {
            return Err(CfdpError::DataTooLarge {
                field: "record_continuation_state",
                max: 3,
            });
        }
        set_bits_u8(&mut self.admin_octet, FILE_DATA_REC_CONT_STATE_MASK, state);
        Ok(())
    }

    /// Returns the length of the segment metadata in bytes (0 to 63).
    pub fn metadata_len(&self) -> usize {
        get_bits_u8(self.admin_octet, FILE_DATA_SEG_META_LEN_MASK) as usize
    }
    /// Sets the segment metadata length field.
    pub fn set_metadata_len(&mut self, len: u8) -> Result<(), CfdpError> {
        if len > 63 {
            return Err(CfdpError::DataTooLarge {
                field: "segment_metadata_length",
                max: 63,
            });
        }
        set_bits_u8(&mut self.admin_octet, FILE_DATA_SEG_META_LEN_MASK, len);
        Ok(())
    }

    /// Returns the segment metadata as a slice.
    pub fn segment_metadata(&self) -> Result<&[u8], CfdpError> {
        self.rest
            .get(0..self.metadata_len())
            .ok_or_else(|| CfdpError::Custom("Invalid segment metadata slice"))
    }
    /// Writes segment metadata bytes into the PDU.
    pub fn set_segment_metadata(&mut self, metadata: &[u8]) -> Result<(), CfdpError> {
        let len = metadata.len();
        if len > 63 {
            return Err(CfdpError::DataTooLarge {
                field: "segment_metadata",
                max: 63,
            });
        }
        let dest_slice = self
            .rest
            .get_mut(0..len)
            .ok_or_else(|| CfdpError::Custom("Invalid segment metadata slice"))?;
        dest_slice.copy_from_slice(metadata);
        Ok(())
    }

    /// Parses the FSS `Offset` from the `rest` slice (after the metadata).
    pub fn offset(&self, large_file_flag: bool) -> Result<u64, CfdpError> {
        let offset_slice = self
            .rest
            .get(self.metadata_len()..)
            .ok_or_else(|| CfdpError::Custom("Invalid FSS Offset slice"))?;
        if large_file_flag {
            U64::ref_from_prefix(offset_slice)
                .map(|(len, _)| len.get())
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        } else {
            U32::ref_from_prefix(offset_slice)
                .map(|(len, _)| len.get() as u64)
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        }
    }
    /// Sets the FSS byte offset into the file.
    pub fn set_offset(&mut self, offset: u64, large_file_flag: bool) -> Result<(), CfdpError> {
        let offset_slice = self
            .rest
            .get_mut(self.metadata_len()..)
            .ok_or_else(|| CfdpError::Custom("Invalid FSS Offset slice"))?;
        if large_file_flag {
            U64::mut_from_prefix(offset_slice)
                .map(|(len, _)| len.set(offset))
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        } else {
            if offset > u32::MAX as u64 {
                return Err(CfdpError::DataTooLarge {
                    field: "offset",
                    max: u32::MAX as usize,
                });
            }
            U32::mut_from_prefix(offset_slice)
                .map(|(len, _)| len.set(offset as u32))
                .map_err(|_| CfdpError::Custom("Invalid FSS Offset"))
        }
    }

    /// Returns the slice containing the actual file data.
    pub fn file_data<'a>(&'a self, large_file_flag: bool) -> Result<&'a [u8], CfdpError> {
        let offset_len = if large_file_flag { 8 } else { 4 };
        let start = self.metadata_len() + offset_len;
        self.rest
            .get(start..)
            .ok_or_else(|| CfdpError::Custom("Invalid file data slice"))
    }
    /// Returns a mutable slice containing the actual file data.
    pub fn file_data_mut(&mut self, large_file_flag: bool) -> Result<&mut [u8], CfdpError> {
        let offset_len = if large_file_flag { 8 } else { 4 };
        let start = self.metadata_len() + offset_len;
        self.rest
            .get_mut(start..)
            .ok_or_else(|| CfdpError::Custom("Invalid file data slice"))
    }

    /// Returns the raw administrative octet.
    pub fn admin_octet(&self) -> u8 {
        self.admin_octet
    }

    /// Returns the raw rest slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

#[bon]
impl FileDataPduWithMeta {
    /// Builds a complete File Data PDU with segment metadata.
    /// The caller is responsible for writing the file data into the buffer after this function returns.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        // Transaction Context
        source_entity_id: EntityId,
        destination_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        large_file_flag: bool,
        crc_flag: bool,
        // FileData Specific
        segmentation_control: bool,
        record_continuation_state: u8,
        segment_metadata: &'a [u8],
        offset: u64,
        file_data_len: usize,
    ) -> Result<&'a mut Pdu, CfdpError> {
        if record_continuation_state > 0b11 {
            return Err(CfdpError::DataTooLarge {
                field: "record_continuation_state",
                max: 3,
            });
        }
        if segment_metadata.len() > 63 {
            return Err(CfdpError::DataTooLarge {
                field: "segment_metadata",
                max: 63,
            });
        }

        let fixed_part_len = size_of::<u8>();
        let metadata_len = segment_metadata.len();
        let offset_len = if large_file_flag { 8 } else { 4 };
        let data_field_len = (fixed_part_len + metadata_len + offset_len + file_data_len) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileData)
            .direction(Direction::TowardReceiver)
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(large_file_flag)
            .data_field_len(data_field_len)
            .seg_ctrl(segmentation_control)
            .seg_meta_flag(true) // Crucial: this flag MUST be set
            .build()?;

        let pdu = Pdu::builder()
            .buffer(buffer)
            .header_fixed(header)
            .source_entity_id(source_entity_id)
            .destination_entity_id(destination_entity_id)
            .transaction_seq_num(transaction_seq_num)
            .build()?;

        let data_field = pdu.data_field_mut().unwrap();
        let fd_pdu = FileDataPduWithMeta::mut_from_bytes(data_field).unwrap();

        fd_pdu.set_record_continuation_state(record_continuation_state)?;
        fd_pdu.set_metadata_len(segment_metadata.len() as u8)?;
        fd_pdu.set_segment_metadata(segment_metadata)?;
        fd_pdu.set_offset(offset, large_file_flag)?;
        Ok(pdu)
    }
}
