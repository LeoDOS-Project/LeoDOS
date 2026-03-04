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
use crate::transport::cfdp::pdu::tlv::Tlv;
use crate::transport::cfdp::pdu::tlv::TlvIterator;
use crate::transport::cfdp::pdu::tlv::TlvType;
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

/// A zero-copy representation of the **data field** of an End-of-File (EOF) PDU.
///
/// This struct's layout strictly follows the CCSDS specification (Table 5-6).
/// It consists of a fixed-size portion and a `rest` slice. The `rest` slice
/// contains the File-Size Sensitive (FSS) `file_size` field and any optional `Fault Location` TLV.
///
/// ```text
/// +------------------------------------+----------------+--------------------------------------+
/// | Field Name                         | Size           | Notes                                |
/// +------------------------------------+----------------+--------------------------------------+
/// | Condition Code                     | 4 bits         | The reason for the end of file.      |
/// | Reserved for future use            | 4 bits         | Reserved, set to zero.               |
/// |                                    |                | (Both are packed into one octet)     |
/// |                                    |                |                                      |
/// | File Checksum                      | 32 bits        | Checksum of the entire file's data.  |
/// |                                    |                |                                      |
/// | -- Start of `rest` slice --------- | -------------- | ------------------------------------ |
/// | File Size                          | 32 or 64 bits  | FSS field. Size depends on header's  |
/// |                                    | (FSS)          | `Large File Flag`.                   |
/// |                                    |                |                                      |
/// | Fault Location (Optional)          | Variable (TLV) | Present if Condition Code is an error|
/// +------------------------------------+----------------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct EofPdu {
    /// An 8-bit field containing the 4-bit `Condition Code` and 4 `Spare` bits.
    condition_code_and_spare: u8,
    /// The checksum of the entire file.
    file_checksum: U32,
    /// Contains the variable-sized `file_size` and optional `Fault Location` TLV.
    rest: [u8],
}

/// Bit masks for the EOF PDU's packed fields.
mod bitmasks {
    /// Mask for the 4-bit condition code.
    pub const EOF_CC_MASK: u8 = 0b_11110000;
}

use bitmasks::*;

impl EofPdu {
    /// Returns the Condition Code (Table 5-5) from the bit-packed field.
    pub fn condition_code(&self) -> Result<ConditionCode, CfdpError> {
        // Condition Code is in the 4 most significant bits.
        ConditionCode::try_from(get_bits_u8(self.condition_code_and_spare, EOF_CC_MASK))
    }
    /// Sets the condition code field.
    pub fn set_condition_code(&mut self, code: ConditionCode) {
        set_bits_u8(&mut self.condition_code_and_spare, EOF_CC_MASK, code as u8);
    }

    /// Returns the 32-bit file checksum.
    pub fn file_checksum(&self) -> u32 {
        self.file_checksum.get()
    }
    /// Sets the 32-bit file checksum.
    pub fn set_file_checksum(&mut self, checksum: u32) {
        self.file_checksum.set(checksum);
    }

    /// Correctly parses the File-Size Sensitive (FSS) `file_size` field from the `rest` slice.
    ///
    /// # Arguments
    /// * `large_file_flag`: The state of the `Large File Flag` from the PDU header,
    ///   which determines whether to parse 32 or 64 bits.
    pub fn file_size(&self, large_file_flag: bool) -> Result<u64, CfdpError> {
        if large_file_flag {
            U64::ref_from_prefix(&self.rest)
                .map(|(len, _)| len.get())
                .map_err(|_| CfdpError::Custom("Invalid Eof File Size"))
        } else {
            U32::ref_from_prefix(&self.rest)
                .map(|(len, _)| len.get() as u64)
                .map_err(|_| CfdpError::Custom("Invalid Eof File Size"))
        }
    }
    /// Sets the FSS file size field.
    pub fn set_file_size(
        &mut self,
        large_file_flag: bool,
        file_size: u64,
    ) -> Result<(), CfdpError> {
        if large_file_flag {
            let size_field = U64::new(file_size);
            let size_bytes = size_field.as_bytes();
            self.rest
                .get_mut(0..8)
                .ok_or(CfdpError::Custom("Insufficient space for 64-bit file size"))?
                .copy_from_slice(&size_bytes);
            Ok(())
        } else {
            if file_size > u32::MAX as u64 {
                return Err(CfdpError::Custom("File size too large for 32-bit field"));
            }
            let size_field = U32::new(file_size as u32);
            let size_bytes = size_field.as_bytes();
            self.rest
                .get_mut(0..4)
                .ok_or(CfdpError::Custom("Insufficient space for 32-bit file size"))?
                .copy_from_slice(&size_bytes);
            Ok(())
        }
    }

    /// Returns an iterator over any trailing TLVs (e.g., Fault Location).
    pub fn tlvs(&self, large_file_flag: bool) -> Result<TlvIterator<'_>, CfdpError> {
        let file_size_len = if large_file_flag { 8 } else { 4 };

        let tlv_buffer = self
            .rest
            .get(file_size_len..)
            .ok_or_else(|| CfdpError::Custom("Invalid Eof PDU: insufficient data for TLVs"))?;

        Ok(TlvIterator { buffer: tlv_buffer })
    }

    /// Finds and returns the Fault Location TLV's value from the data field.
    ///
    /// The Fault Location is an Entity ID, as defined by the standard.
    /// This method will search for the first TLV with the correct type.
    pub fn fault_location(&self, large_file_flag: bool) -> Result<Option<&Tlv>, CfdpError> {
        let fault_loc_tlv = self
            .tlvs(large_file_flag)?
            .find(|tlv| tlv.tlv_type() == Ok(TlvType::EntityId));

        Ok(fault_loc_tlv)
    }

    /// Get the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

#[bon]
impl EofPdu {
    /// Builds a new EOF PDU in the given buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        large_file_flag: bool,
        crc_flag: bool,
        condition_code: ConditionCode,
        file_checksum: u32,
        file_size: u64,
        fault_location: Option<&'a [u8]>,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let fixed_part_len = size_of::<u8>() + size_of::<U32>();
        let file_size_len = if large_file_flag { 8 } else { 4 };
        let fault_loc_len = fault_location.map_or(0, |loc| loc.len());

        let specific_data_len = fixed_part_len + file_size_len + fault_loc_len;
        let data_field_len = (DirectiveCode::size() + specific_data_len) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileDirective)
            .direction(Direction::TowardReceiver)
            .tx_mode(transmission_mode)
            .crc_flag(crc_flag)
            .large_file_flag(large_file_flag)
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

        let data_field = pdu
            .data_field_mut()
            .or_else(|_| Err(CfdpError::Custom("Failed to get data field")))?;
        let actual_data_field_len = data_field.len();
        let directive_pdu = FileDirectivePdu::mut_from_bytes(data_field).map_err(|_| {
            CfdpError::BufferTooSmall {
                required: data_field_len as usize,
                provided: actual_data_field_len,
            }
        })?;
        directive_pdu.set_directive_code(DirectiveCode::Eof);

        let remaining_len = directive_pdu.rest.len();
        let rest_len = file_size_len + fault_loc_len;
        let eof_pdu = EofPdu::mut_from_bytes_with_elems(&mut directive_pdu.rest, rest_len)
            .map_err(|_| CfdpError::BufferTooSmall {
                required: specific_data_len,
                provided: remaining_len,
            })?;
        eof_pdu.set_condition_code(condition_code);
        eof_pdu.set_file_checksum(file_checksum);
        eof_pdu
            .set_file_size(large_file_flag, file_size)
            .map_err(|_| CfdpError::Custom("Failed to set file size"))?;

        // Write TLV
        if let Some(location) = fault_location {
            let tlv_slice = &mut eof_pdu.rest[file_size_len..];
            let tlv_slice_len = tlv_slice.len();
            let (tlv, _rest) =
                Tlv::mut_from_prefix(tlv_slice).map_err(|_| CfdpError::BufferTooSmall {
                    required: fault_loc_len,
                    provided: tlv_slice_len,
                })?;
            tlv.set_type(TlvType::EntityId);
            tlv.set_length(location.len())
                .map_err(|_| CfdpError::Custom("Failed to set TLV length"))?;
            tlv.set_value(location)
                .map_err(|_| CfdpError::Custom("Failed to set TLV value"))?;
        }

        Ok(pdu)
    }
}
