use crate::transport::cfdp::pdu::CfdpError;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::FileDirectivePdu;
use crate::transport::cfdp::pdu::header::Direction;
use crate::transport::cfdp::pdu::header::PduType;
use crate::transport::cfdp::pdu::header::TransmissionMode;
use crate::transport::cfdp::pdu::tlv::Tlv;
use crate::transport::cfdp::pdu::tlv::TlvIterator;
use crate::transport::cfdp::pdu::tlv::TlvType;
use crate::transport::cfdp::pdu::tlv::fault_handler_override::FaultHandlerSet;
use crate::transport::cfdp::pdu::tlv::fault_handler_override::TlvFaultHandlerOverride;
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

/// A zero-copy representation of the **data field** of a Metadata PDU.
///
/// This struct's layout strictly follows the CCSDS specification (Table 5-9).
/// It consists of a fixed-size portion and a `rest` slice. The `rest` slice
/// contains the LV-encoded file names and any optional TLVs.
///
/// ```text
/// +------------------------------------+----------------+--------------------------------------+
/// | Field Name                         | Size           | Notes                                |
/// +------------------------------------+----------------+--------------------------------------+
/// | Reserved for future use            | 1 bit          |                                      |
/// | Closure requested                  | 1 bit          |                                      |
/// | Reserved for future use            | 2 bit          |                                      |
/// | Checksum type                      | 4 bits         | (Packed into one octet with spares)  |
/// |                                    |                |                                      |
/// | File Size                          | 32 or 64 bits  | FSS field.                           |
/// |                                    | (FSS)          |                                      |
/// | -- Start of `rest` slice --------- | -------------- | ------------------------------------ |
/// | Source File Name                   | Variable (LV)  | Length-Value encoded.                |
/// | Destination File Name              | Variable (LV)  | Length-Value encoded.                |
/// | Options (Optional)                 | Variable (TLV) | Zero or more TLVs.                   |
/// +------------------------------------+----------------+--------------------------------------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct MetadataPdu {
    /// An 8-bit field containing `Closure requested` (1), and `Checksum type` (4)
    packed_flags: u8,
    /// Contains the FSS file_size, LV file names, and optional TLVs.
    rest: [u8],
}

#[rustfmt::skip]
mod bitmasks {
    pub const _META_RESERVED_MASK_1: u8 =   0b_10000000;
    pub const META_CLOSURE_REQ_MASK: u8 =   0b_01000000;
    pub const _META_RESERVED_MASK_2: u8 =   0b_00110000;
    pub const META_CHECKSUM_TYPE_MASK: u8 = 0b_00001111;
}

use bitmasks::*;

/// Identifies the checksum algorithm used for data integrity (Table 5-10).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumType {
    /// CCSDS Modular Checksum (Sum of 32-bit words).
    Modular = 0,
    /// CCSDS Proximity-1 CRC32.
    Proximity1Crc32 = 1,
    /// CRC-32C (Castagnoli), common in iSCSI.
    Crc32c = 2,
    /// Standard IEEE 802.3 Ethernet CRC32.
    IeeeCrc32 = 3,
    /// No checksum.
    Null = 15,
}

impl TryFrom<u8> for ChecksumType {
    type Error = CfdpError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChecksumType::Modular),
            1 => Ok(ChecksumType::Proximity1Crc32),
            2 => Ok(ChecksumType::Crc32c),
            3 => Ok(ChecksumType::IeeeCrc32),
            15 => Ok(ChecksumType::Null),
            _ => Err(CfdpError::Custom("Unsupported checksum type")),
        }
    }
}

impl MetadataPdu {
    /// Returns the state of the `Closure requested` flag.
    pub fn closure_requested(&self) -> bool {
        get_bits_u8(self.packed_flags, META_CLOSURE_REQ_MASK) == 1
    }
    /// Sets the `Closure requested` flag.
    pub fn set_closure_requested(&mut self, requested: bool) {
        set_bits_u8(
            &mut self.packed_flags,
            META_CLOSURE_REQ_MASK,
            if requested { 1 } else { 0 },
        );
    }

    /// Returns the 4-bit integer identifying the checksum algorithm to be used.
    pub fn checksum_type(&self) -> Result<ChecksumType, CfdpError> {
        get_bits_u8(self.packed_flags, META_CHECKSUM_TYPE_MASK).try_into()
    }
    /// Sets the checksum type field.
    pub fn set_checksum_type(&mut self, checksum_type: ChecksumType) {
        set_bits_u8(
            &mut self.packed_flags,
            META_CHECKSUM_TYPE_MASK,
            checksum_type as u8,
        );
    }

    /// Correctly parses the File-Size Sensitive (FSS) `file_size` field.
    pub fn file_size(&self, large_file_flag: bool) -> Result<u64, CfdpError> {
        if large_file_flag {
            U64::ref_from_prefix(&self.rest)
                .map_err(|_| CfdpError::Custom("Failed to parse 64-bit file size"))
                .map(|(len, _)| len.get())
        } else {
            U32::ref_from_prefix(&self.rest)
                .map_err(|_| CfdpError::Custom("Failed to parse 32-bit file size"))
                .map(|(len, _)| len.get() as u64)
        }
    }
    /// Sets the FSS file size field.
    pub fn set_file_size(
        &mut self,
        large_file_flag: bool,
        file_size: u64,
    ) -> Result<(), CfdpError> {
        if large_file_flag {
            let file_size_field = U64::mut_from_prefix(&mut self.rest)
                .map_err(|_| CfdpError::Custom("Failed to parse 64-bit file size"))?
                .0;
            file_size_field.set(file_size);
            Ok(())
        } else {
            if file_size > u32::MAX as u64 {
                return Err(CfdpError::Custom("File size exceeds 32-bit maximum"));
            }
            let file_size_field = U32::mut_from_prefix(&mut self.rest)
                .map_err(|_| CfdpError::Custom("Failed to parse 32-bit file size"))?
                .0;
            file_size_field.set(file_size as u32);
            Ok(())
        }
    }

    /// A private helper to parse the variable-length fields after the FSS file_size.
    /// This avoids re-parsing the LV fields in each public getter.
    /// Returns (source_name, dest_name, options_slice)
    pub fn variable_fields(
        &self,
        large_file_flag: bool,
    ) -> Result<(&[u8], &[u8], TlvIterator<'_>), CfdpError> {
        let file_size_len = if large_file_flag { 8 } else { 4 };
        let mut remainder = self.rest.get(file_size_len..).ok_or_else(|| {
            CfdpError::Custom("Invalid metadata: insufficient data for file size")
        })?;

        // Parse Source File Name (LV)
        let source_len = *remainder
            .first()
            .ok_or_else(|| CfdpError::Custom("Invalid metadata: missing source name length"))?
            as usize;

        // Advance remainder past the length byte
        remainder = remainder
            .get(1..)
            .ok_or(CfdpError::Custom("Invalid metadata slice"))?;
        if remainder.len() < source_len {
            return Err(CfdpError::Custom(
                "Invalid metadata: insufficient data for source name",
            ));
        }
        let (source_name, mut remainder) = remainder.split_at(source_len);

        // Parse Destination File Name (LV)
        let dest_len = *remainder
            .first()
            .ok_or_else(|| CfdpError::Custom("Invalid metadata: missing destination name length"))?
            as usize;

        // Advance remainder past the length byte
        remainder = remainder
            .get(1..)
            .ok_or(CfdpError::Custom("Invalid metadata slice"))?;
        if remainder.len() < dest_len {
            return Err(CfdpError::Custom(
                "Invalid metadata: insufficient data for destination name",
            ));
        }
        let (dest_name, options_tlvs) = remainder.split_at(dest_len);

        Ok((
            source_name,
            dest_name,
            TlvIterator {
                buffer: options_tlvs,
            },
        ))
    }

    /// Writes the LV-encoded file names and optional TLV options into the PDU.
    pub fn set_variable_fields(
        &mut self,
        large_file_flag: bool,
        source_file_name: &[u8],
        dest_file_name: &[u8],
        options: Option<&[u8]>,
    ) -> Result<(), CfdpError> {
        let file_size_len = if large_file_flag { 8 } else { 4 };
        let mut cursor = file_size_len;

        // Write Source File Name (LV)
        if source_file_name.len() > 255 {
            return Err(CfdpError::Custom("Source file name too long"));
        }
        // Write Destination File Name (LV)
        if dest_file_name.len() > 255 {
            return Err(CfdpError::Custom("Destination file name too long"));
        }

        *self
            .rest
            .get_mut(cursor)
            .ok_or_else(|| CfdpError::Custom("Insufficient space for Source File Name length"))? =
            source_file_name.len() as u8;
        cursor += 1;
        self.rest
            .get_mut(cursor..cursor + source_file_name.len())
            .ok_or_else(|| CfdpError::Custom("Insufficient space for Source File Name"))?
            .copy_from_slice(source_file_name);
        cursor += source_file_name.len();
        *self.rest.get_mut(cursor).ok_or_else(|| {
            CfdpError::Custom("Insufficient space for Destination File Name length")
        })? = dest_file_name.len() as u8;
        cursor += 1;
        self.rest
            .get_mut(cursor..cursor + dest_file_name.len())
            .ok_or_else(|| CfdpError::Custom("Insufficient space for Destination File Name"))?
            .copy_from_slice(dest_file_name);
        cursor += dest_file_name.len();

        // Write Options TLVs
        if let Some(opts) = options {
            self.rest
                .get_mut(cursor..cursor + opts.len())
                .ok_or_else(|| CfdpError::Custom("Insufficient space for Options TLVs"))?;
        }

        Ok(())
    }

    /// Returns a slice representing the source file name.
    pub fn source_file_name(&self, large_file_flag: bool) -> Result<&[u8], CfdpError> {
        self.variable_fields(large_file_flag).map(|(src, _, _)| src)
    }

    /// Returns a slice representing the destination file name.
    pub fn dest_file_name(&self, large_file_flag: bool) -> Result<&[u8], CfdpError> {
        self.variable_fields(large_file_flag).map(|(_, dst, _)| dst)
    }

    /// Returns an iterator over the TLV options in the metadata.
    pub fn options(&self, large_file_flag: bool) -> Result<TlvIterator<'_>, CfdpError> {
        self.variable_fields(large_file_flag)
            .map(|(_, _, tlv_iter)| tlv_iter)
    }

    /// Parses the Fault Handler Override TLVs and returns a `FaultHandlerSet`.
    /// The set is initialized with default handlers, which are then updated
    /// by any overrides present in the PDU's options.
    pub fn fault_handler_overrides(
        &self,
        large_file_flag: bool,
    ) -> Result<FaultHandlerSet, CfdpError> {
        // Start with a set of default handlers.
        let mut handlers = FaultHandlerSet::default();
        let tlv_iter = self.options(large_file_flag)?;

        for tlv in tlv_iter {
            if tlv.tlv_type()? == TlvType::FaultHandlerOverride {
                // The value of a FaultHandlerOverride TLV is exactly one byte.
                if tlv.value().len() != 1 {
                    // Malformed TLV, maybe log this and continue.
                    continue;
                }
                let fho_tlv = TlvFaultHandlerOverride::ref_from_bytes(tlv.value())
                    .map_err(|_| CfdpError::Custom("Failed to parse Fault Handler Override TLV"))?;

                // Use the set_handler method to update the bit-packed u32.
                handlers.set_handler(fho_tlv.condition_code()?, fho_tlv.handler_code()?);
            }
        }
        Ok(handlers)
    }

    /// Returns an iterator over the Filestore Request TLVs in the metadata.
    pub fn filestore_requests(
        &self,
        large_file_flag: bool,
    ) -> Result<impl Iterator<Item = &Tlv>, CfdpError> {
        Ok(self
            .options(large_file_flag)?
            .filter(|tlv| tlv.tlv_type() == Ok(TlvType::FilestoreRequest)))
    }

    /// Get the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }
}

#[bon]
impl MetadataPdu {
    /// Builds a new Metadata PDU in the given buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_entity_id: EntityId,
        dest_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
        transmission_mode: TransmissionMode,
        large_file_flag: bool,
        crc_flag: bool,
        closure_requested: bool,
        checksum_type: ChecksumType,
        file_size: u64,
        source_file_name: &'a [u8],
        dest_file_name: &'a [u8],
        options: Option<&'a [u8]>,
    ) -> Result<&'a mut Pdu, CfdpError> {
        // --- Correct Size Calculation ---
        let fixed_part_len = size_of::<u8>(); // packed_flags
        let file_size_len = if large_file_flag { 8 } else { 4 };
        let src_name_lv_len = 1 + source_file_name.len(); // +1 for Length octet
        let dst_name_lv_len = 1 + dest_file_name.len(); // +1 for Length octet
        let options_len = options.map_or(0, |o| o.len());

        let specific_data_len =
            fixed_part_len + file_size_len + src_name_lv_len + dst_name_lv_len + options_len;
        let data_field_len = (1 + specific_data_len) as u16;

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
            .map_err(|_| CfdpError::Custom("Failed to get data field"))?;
        let provided_len = data_field.len();
        let directive_pdu = FileDirectivePdu::mut_from_bytes(data_field).map_err(|_| {
            CfdpError::BufferTooSmall {
                required: data_field_len as usize,
                provided: provided_len,
            }
        })?;
        directive_pdu.set_directive_code(DirectiveCode::Metadata);

        let provided_len = directive_pdu.rest.len();
        let rest_len = file_size_len + src_name_lv_len + dst_name_lv_len + options_len;
        let meta_pdu =
            MetadataPdu::mut_from_bytes_with_elems(&mut directive_pdu.rest, rest_len)
                .map_err(|_| CfdpError::BufferTooSmall {
                    required: specific_data_len,
                    provided: provided_len,
                })?;
        meta_pdu.set_closure_requested(closure_requested);
        meta_pdu.set_checksum_type(checksum_type);

        meta_pdu
            .set_file_size(large_file_flag, file_size)
            .map_err(|_| CfdpError::Custom("Failed to set file size in Metadata PDU"))?;
        meta_pdu
            .set_variable_fields(large_file_flag, source_file_name, dest_file_name, options)
            .map_err(|_| CfdpError::Custom("Failed to set variable fields in Metadata PDU"))?;

        Ok(pdu)
    }
}
