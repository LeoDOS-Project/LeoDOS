use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::pdu::header::TransmissionMode;
use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::FileDirectivePdu;
use crate::transport::cfdp::pdu::header::Direction;
use crate::transport::cfdp::pdu::header::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::header::PduType;
use crate::transport::cfdp::pdu::tlv::TlvIterator;
use crate::transport::cfdp::pdu::tlv::TlvType;
use crate::utils::get_bits_u8;

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy representation of the **data field** of a Finished PDU.
///
/// This struct's layout strictly follows the CCSDS specification (Table 5-7).
/// It consists of a fixed-size portion and a `rest` slice. The `rest` slice
/// contains any optional `Filestore Responses` and `Fault Location` TLVs.
///
/// This struct can be created from a PDU's data field using `FinishedPdu::ref_from_bytes()`.
/// Access to the bit-packed fields and optional TLVs is provided via parser methods.
///
/// ```text
/// +------------------------------------+----------------+--------------------------------------+
/// | Field Name                         | Size           | Notes                                |
/// +------------------------------------+----------------+--------------------------------------+
/// | Condition Code                     | 4 bits         |                                      |
/// | Reserved for future use            | 1 bit          |                                      |
/// | Delivery Code                      | 1 bit          |                                      |
/// | File Status                        | 2 bits         | (All four are packed into one octet) |
/// |                                    |                |                                      |
/// | -- Start of `rest` slice --------- | -------------- | ------------------------------------ |
/// | Filestore Responses (Optional)     | Variable (TLV) | Zero or more Filestore Response TLVs |
/// | Fault Location (Optional)          | Variable (TLV) | Present if Condition Code is an error|
/// +------------------------------------+----------------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct FinishedPdu {
    /// An 8-bit field containing the 4-bit `Condition Code`, 1 `Spare` bit,
    /// 1-bit `Delivery Code`, and 2-bit `File Status`.
    packed_flags: u8,
    /// Contains any optional `Filestore Responses` and `Fault Location` TLVs.
    rest: [u8],
}

#[rustfmt::skip]
mod bitmasks {
    pub const FINISHED_CONDITION_CODE_MASK: u8 = 0b_11110000;
    pub const _FINISHED_RESERVED_MASK: u8 =      0b_00001000;
    pub const FINISHED_DELIVERY_CODE_MASK: u8 =  0b_00000100;
    pub const FINISHED_FILE_STATUS_MASK: u8 =    0b_00000011;
}

use bitmasks::*;

/// Final status of the delivered file at the receiver.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum FileStatus {
    /// File was discarded deliberately.
    DiscardedDeliberately = 0b00,
    /// File was discarded due to a filestore rejection.
    DiscardedFileStoreRejection = 0b01,
    /// File was retained successfully.
    Retained = 0b10,
    /// File status is not reported.
    Unreported = 0b11,
}

impl TryFrom<u8> for FileStatus {
    type Error = ();

    fn try_from(val: u8) -> Result<Self, ()> {
        let res = match val {
            0b00 => FileStatus::DiscardedDeliberately,
            0b01 => FileStatus::DiscardedFileStoreRejection,
            0b10 => FileStatus::Retained,
            0b11 => FileStatus::Unreported,
            _ => return Err(()),
        };
        Ok(res)
    }
}

impl FinishedPdu {
    /// Returns the Condition Code (Table 5-5) from the bit-packed field.
    pub fn condition_code(&self) -> Result<ConditionCode, CfdpError> {
        ConditionCode::try_from(get_bits_u8(self.packed_flags, FINISHED_CONDITION_CODE_MASK))
    }
    pub fn set_condition_code(&mut self, code: ConditionCode) {
        let code_bits = (code as u8) << 4;
        self.packed_flags = (self.packed_flags & !FINISHED_CONDITION_CODE_MASK) | code_bits;
    }

    /// Returns the Delivery Code from the bit-packed field. `true` means Data Complete.
    pub fn delivery_code(&self) -> bool {
        // Delivery code is '0' for Data Complete, '1' for Incomplete.
        // We'll return a bool where `true` means complete.
        get_bits_u8(self.packed_flags, FINISHED_DELIVERY_CODE_MASK) == 0
    }
    pub fn set_delivery_code(&mut self, complete: bool) {
        let delivery_bit = if complete { 0 } else { 1 } << 2;
        self.packed_flags = (self.packed_flags & !FINISHED_DELIVERY_CODE_MASK) | delivery_bit;
    }

    /// Returns the File Status (Table 5-7) from the bit-packed field.
    pub fn file_status(&self) -> Option<FileStatus> {
        FileStatus::try_from(get_bits_u8(self.packed_flags, FINISHED_FILE_STATUS_MASK)).ok()
    }
    pub fn set_file_status(&mut self, status: FileStatus) {
        let status_bits = status as u8;
        self.packed_flags = (self.packed_flags & !FINISHED_FILE_STATUS_MASK) | status_bits;
    }

    /// Returns an iterator over all TLVs in the data field.
    pub fn tlvs(&self) -> TlvIterator<'_> {
        TlvIterator { buffer: &self.rest }
    }

    /// Returns an iterator that filters for only Filestore Response TLVs.
    /// The value of each yielded TLV is a complete Filestore Response record.
    pub fn filestore_responses(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.tlvs()
            .filter(|tlv| tlv.tlv_type() == Ok(TlvType::FilestoreResponse))
            .map(|tlv| tlv.value())
    }

    /// Finds and returns the Fault Location TLV's value, which is an Entity ID.
    pub fn fault_location(&self) -> Option<&[u8]> {
        self.tlvs()
            .find(|tlv| tlv.tlv_type() == Ok(TlvType::EntityId))
            .map(|tlv| tlv.value())
    }

    pub fn set_tlvs(
        &mut self,
        filestore_responses: Option<&[u8]>,
        fault_location: Option<&[u8]>,
    ) -> Result<(), CfdpError> {
        let mut cursor = 0;
        if let Some(responses) = filestore_responses {
            let len = responses.len();
            self.rest
                .get_mut(cursor..cursor + len)
                .ok_or_else(|| CfdpError::Custom("Insufficient space for Filestore Responses"))?
                .copy_from_slice(responses);
            cursor += len;
        }
        if let Some(location) = fault_location {
            let len = location.len();
            self.rest
                .get_mut(cursor..cursor + len)
                .ok_or_else(|| CfdpError::Custom("Insufficient space for Fault Location"))?
                .copy_from_slice(location);
        }
        Ok(())
    }

    /// Get the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

#[bon]
impl FinishedPdu {
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
        delivery_code_complete: bool,
        file_status: FileStatus,
        filestore_responses: Option<&'a [u8]>,
        fault_location: Option<&'a [u8]>,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let fixed_part_len = size_of::<u8>();
        let fs_responses_len = filestore_responses.map_or(0, |r| r.len());
        let fault_loc_len = fault_location.map_or(0, |loc| loc.len());
        let specific_data_len = fixed_part_len + fs_responses_len + fault_loc_len;
        let data_field_len = (1 + specific_data_len) as u16;

        let header = PduHeaderFixedPart::builder()
            .version(1)
            .pdu_type(PduType::FileDirective)
            .direction(Direction::TowardSender)
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

        let data_field = pdu.data_field_mut()?;
        let directive_pdu = FileDirectivePdu::mut_from_bytes(data_field)
            .map_err(|_| CfdpError::Custom("Failed to parse File Directive PDU from data field"))?;
        directive_pdu.set_directive_code(DirectiveCode::Finished);
        let remaining_len = directive_pdu.rest.len();

        let rest_len = fs_responses_len + fault_loc_len;
        let (fin_pdu, _rest) =
            FinishedPdu::mut_from_prefix_with_elems(&mut directive_pdu.rest, rest_len)
                .map_err(|_| CfdpError::BufferTooSmall {
                    provided: remaining_len,
                    required: specific_data_len,
                })?;
        fin_pdu.set_condition_code(condition_code);
        fin_pdu.set_delivery_code(delivery_code_complete);
        fin_pdu.set_file_status(file_status);
        fin_pdu.set_tlvs(filestore_responses, fault_location)?;

        Ok(pdu)
    }
}
