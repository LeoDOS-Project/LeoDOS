//! Defines the structures and serialization/deserialization logic for
//! CCSDS File Delivery Protocol (CFDP) Protocol Data Units (PDUs).
//!
//! This module provides safe, zero-copy views and builders for CFDP PDUs,
//! following the pattern used by `SpacePacket`. A single PDU enum, `Pdu<'a>`,
//! holds references to concrete PDU types that are views over the underlying network buffer.
//! This allows for efficient, allocation-free parsing of incoming packets.

/// File Data PDU types and builders.
pub mod file_data;
/// File Directive PDU types (EOF, Finished, ACK, Metadata, NAK, Prompt, KeepAlive).
pub mod file_directive;
/// PDU header structures and field accessors.
pub mod header;
/// Type-Length-Value (TLV) record types and iterators.
pub mod tlv;

use core::fmt;
use core::ops::Deref;
use core::ops::DerefMut;

use crate::transport::cfdp::pdu::file_data::with_meta::FileDataPduWithMeta;
use crate::transport::cfdp::pdu::file_data::without_meta::FileDataPduWithoutMeta;
use crate::transport::cfdp::pdu::file_data::FileDataPdu;
use crate::transport::cfdp::pdu::file_directive::ack::AckPdu;
use crate::transport::cfdp::pdu::file_directive::eof::EofPdu;
use crate::transport::cfdp::pdu::file_directive::finished::FinishedPdu;
use crate::transport::cfdp::pdu::file_directive::keepalive::large::KeepAlivePduLarge;
use crate::transport::cfdp::pdu::file_directive::keepalive::small::KeepAlivePduSmall;
use crate::transport::cfdp::pdu::file_directive::keepalive::KeepAlivePdu;
use crate::transport::cfdp::pdu::file_directive::metadata::MetadataPdu;
use crate::transport::cfdp::pdu::file_directive::nak::large::NakPduLarge;
use crate::transport::cfdp::pdu::file_directive::nak::large::NakSegmentLarge;
use crate::transport::cfdp::pdu::file_directive::nak::small::NakPduSmall;
use crate::transport::cfdp::pdu::file_directive::nak::small::NakSegmentSmall;
use crate::transport::cfdp::pdu::file_directive::nak::NakPdu;
use crate::transport::cfdp::pdu::file_directive::prompt::PromptPdu;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::FileDirectivePdu;
use crate::transport::cfdp::pdu::header::PduHeaderFixedPart;
use crate::transport::cfdp::pdu::header::PduType;
use crate::transport::cfdp::CfdpError;
use crate::utils::min_len;
use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy view of a generic CFDP PDU, containing the header and raw bytes.
///
/// +---------------------------------------+---------------+
/// | Field Name                            | Size          |
/// +---------------------------------------+---------------+
/// | -- PDU Header (Variable Length) ----- | ------------- |
/// |                                       |               |
/// | -- Fixed Part (4 bytes) ------------- | ------------- |
/// |                                       |               |
/// | Version Number                        | 3 bits        |
/// | PDU Type                              | 1 bit         |
/// | Direction                             | 1 bit         |
/// | Transmission Mode                     | 1 bit         |
/// | CRC Flag                              | 1 bit         |
/// | Large File Flag                       | 1 bit         |
/// |                                       |               |
/// | PDU Data Field Length                 | 16 bits       |
/// |                                       |               |
/// | Segmentation Control                  | 1 bit         |
/// | Length of Entity IDs                  | 3 bits        |
/// | Segment Metadata Flag                 | 1 bit         |
/// | Length of Transaction Sequence Number | 3 bits        |
/// |                                       |               |
/// | -- Variable Part (3 to 24 bytes) ---- | ------------- |
/// |                                       |               |
/// | Source Entity ID                      | 1 to 8 octets |
/// | Transaction Sequence Number           | 1 to 8 octets |
/// | Destination Entity ID                 | 1 to 8 octets |
/// |                                       |               |
/// +---------------------------------------+---------------+
/// | -- PDU Data Field (Variable Length) - | ------------- |
/// |                                       |               |
/// +---------------------------------------+---------------+
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct Pdu {
    header_fixed: PduHeaderFixedPart,
    rest: [u8],
}

/// An enum representing a zero-copy view of a parsed PDU.
#[derive(Debug)]
pub enum PduVariant<'a> {
    /// A view of an End of File (EOF) PDU.
    Eof(&'a EofPdu),
    /// A view of a Finished PDU.
    Finished(&'a FinishedPdu),
    /// A view of an Acknowledgment (ACK) PDU.
    Ack(&'a AckPdu),
    /// A view of a Metadata PDU.
    Metadata(&'a MetadataPdu),
    /// A view of a File Data PDU.
    FileData(FileDataPdu<'a>),
    /// A view of a Negative Acknowledgment (NAK) PDU.
    Nak(NakPdu<'a>),
    /// A view of a Prompt PDU.
    Prompt(&'a PromptPdu),
    /// A view of a Keep Alive PDU.
    KeepAlive(KeepAlivePdu<'a>),
}

fn write_to_slice(val: u64, slice: &mut [u8]) -> Result<(), CfdpError> {
    let len = slice.len();
    if len == 0 || len > 8 {
        return Err(CfdpError::Custom("Invalid slice length for Entity ID"));
    }
    // Ensure the value can be represented in the given length.
    if len < min_len(val) {
        return Err(CfdpError::Custom(
            "Slice too small to represent Entity ID value",
        ));
    }

    let full_bytes = val.to_be_bytes();
    let relevant_bytes = &full_bytes[8 - len..];
    slice.copy_from_slice(relevant_bytes);
    Ok(())
}

/// The unique identifier for a CFDP entity, stored as an owned `u64`.
///
/// CFDP Entity IDs are variable-length integers up to 8 bytes. This type
/// normalizes them to a `u64` for easy storage, comparison, and hashing.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct EntityId(pub u64);

impl EntityId {
    /// Creates an `EntityId` from a variable-length byte slice.
    /// The slice must be 1 to 8 bytes long.
    pub fn from_bytes(slice: &[u8]) -> Result<Self, CfdpError> {
        if slice.is_empty() || slice.len() > 8 {
            return Err(CfdpError::Custom("Invalid Entity ID length"));
        }
        let mut bytes = [0u8; 8];
        // Right-align the slice into the 8-byte array to handle smaller IDs correctly.
        bytes[8 - slice.len()..].copy_from_slice(slice);
        Ok(EntityId(u64::from_be_bytes(bytes)))
    }

    /// Serializes this entity ID into a variable-length byte slice.
    pub fn write_to_slice(&self, slice: &mut [u8]) -> Result<(), CfdpError> {
        write_to_slice(self.0, slice)
    }

    /// Returns the minimum number of bytes needed to represent this ID.
    pub fn len(&self) -> usize {
        min_len(self.0)
    }
}

// Implement Debug manually for a cleaner hex output
impl fmt::Debug for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EntityId(0x{:X})", self.0)
    }
}

/// A CFDP transaction sequence number, stored as an owned `u64`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
pub struct TransactionSeqNum(pub u64);

impl TransactionSeqNum {
    /// Increments the sequence number by one, wrapping on overflow.
    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }

    /// Creates a `TransactionSeqNum` from a variable-length byte slice (1 to 8 bytes).
    pub fn from_bytes(slice: &[u8]) -> Result<Self, CfdpError> {
        if slice.is_empty() || slice.len() > 8 {
            return Err(CfdpError::Custom(
                "Invalid Transaction Sequence Number length",
            ));
        }
        let mut bytes = [0u8; 8];
        bytes[8 - slice.len()..].copy_from_slice(slice);
        Ok(TransactionSeqNum(u64::from_be_bytes(bytes)))
    }

    /// Serializes this sequence number into a variable-length byte slice.
    pub fn write_to_slice(&self, slice: &mut [u8]) -> Result<(), CfdpError> {
        write_to_slice(self.0, slice)
    }

    /// Returns the minimum number of bytes needed to represent this value.
    pub fn len(&self) -> usize {
        min_len(self.0)
    }
}

impl Pdu {
    // --- Public API ---

    /// Returns the fixed part of the header.
    pub fn header(&self) -> &PduHeaderFixedPart {
        &self.header_fixed
    }

    /// Returns a mutable reference to the fixed part of the header.
    pub fn header_mut(&mut self) -> &mut PduHeaderFixedPart {
        &mut self.header_fixed
    }

    /// Parses the PDU data field into a typed `PduVariant`.
    pub fn variant(&self) -> Result<PduVariant<'_>, CfdpError> {
        let header = self.header();
        let data = self.data_field()?;
        match header.pdu_type() {
            PduType::FileData => {
                let large_file_flag = header.large_file_flag();
                let seg_meta_flag = header.segment_metadata_flag();

                if seg_meta_flag {
                    let file_data_pdu = FileDataPduWithMeta::ref_from_bytes(data)
                        .map_err(|_| CfdpError::Custom("Failed to parse FileDataPduWithMeta"))?;
                    let metadata_len = file_data_pdu.metadata_len();
                    let offset_len = if large_file_flag { 8 } else { 4 };
                    if file_data_pdu.rest().len() < metadata_len + offset_len {
                        return Err(CfdpError::Custom(
                            "Insufficient data for metadata and offset",
                        ));
                    }
                    Ok(PduVariant::FileData(FileDataPdu::WithMeta(file_data_pdu)))
                } else {
                    let file_data_pdu = FileDataPduWithoutMeta::ref_from_bytes(data)
                        .map_err(|_| CfdpError::Custom("Failed to parse FileDataPduWithoutMeta"))?;
                    let offset_len = if large_file_flag { 8 } else { 4 };
                    if file_data_pdu.rest().len() < offset_len {
                        return Err(CfdpError::Custom("Insufficient data for offset"));
                    }
                    Ok(PduVariant::FileData(FileDataPdu::WithoutMeta(
                        file_data_pdu,
                    )))
                }
            }
            PduType::FileDirective => {
                let file_directive = FileDirectivePdu::ref_from_bytes(data)
                    .map_err(|_| CfdpError::Custom("Failed to parse FileDirectivePdu"))?;
                let rest = file_directive.rest();

                match file_directive.directive_code()? {
                    DirectiveCode::Eof => {
                        let pdu = EofPdu::ref_from_bytes(rest)
                            .map_err(|_| CfdpError::Custom("Failed to parse EofPdu"))?;
                        let min_rest_len = if header.large_file_flag() { 8 } else { 4 };
                        if pdu.rest().len() < min_rest_len {
                            return Err(CfdpError::Custom("Insufficient data for EOF file size"));
                        }
                        Ok(PduVariant::Eof(pdu))
                    }
                    DirectiveCode::Finished => {
                        let pdu = FinishedPdu::ref_from_bytes(rest)
                            .map_err(|_| CfdpError::Custom("Failed to parse FinishedPdu"))?;
                        Ok(PduVariant::Finished(pdu))
                    }
                    DirectiveCode::Ack => {
                        let pdu = AckPdu::ref_from_bytes(rest)
                            .map_err(|_| CfdpError::Custom("Failed to parse AckPdu"))?;
                        Ok(PduVariant::Ack(pdu))
                    }
                    DirectiveCode::Metadata => {
                        let pdu = MetadataPdu::ref_from_bytes(rest)
                            .map_err(|_| CfdpError::Custom("Failed to parse MetadataPdu"))?;
                        let min_rest_len = (if header.large_file_flag() { 8 } else { 4 }) + 1 + 1;
                        if pdu.rest().len() < min_rest_len {
                            return Err(CfdpError::Custom("Insufficient data for Metadata PDU"));
                        }
                        Ok(PduVariant::Metadata(pdu))
                    }
                    DirectiveCode::Nak => {
                        if header.large_file_flag() {
                            let pdu = NakPduLarge::ref_from_bytes(rest)
                                .map_err(|_| CfdpError::Custom("Failed to parse NakPduLarge"))?;
                            if pdu.rest().len() % core::mem::size_of::<NakSegmentLarge>() != 0 {
                                return Err(CfdpError::Custom(
                                    "Invalid NAK segment requests length",
                                ));
                            }
                            Ok(PduVariant::Nak(NakPdu::Large(pdu)))
                        } else {
                            let pdu = NakPduSmall::ref_from_bytes(rest)
                                .map_err(|_| CfdpError::Custom("Failed to parse NakPduSmall"))?;
                            if pdu.rest().len() % core::mem::size_of::<NakSegmentSmall>() != 0 {
                                return Err(CfdpError::Custom(
                                    "Invalid NAK segment requests length",
                                ));
                            }
                            Ok(PduVariant::Nak(NakPdu::Small(pdu)))
                        }
                    }
                    DirectiveCode::Prompt => {
                        let pdu = PromptPdu::ref_from_bytes(rest)
                            .map_err(|_| CfdpError::Custom("Failed to parse PromptPdu"))?;
                        Ok(PduVariant::Prompt(pdu))
                    }
                    DirectiveCode::KeepAlive => {
                        if header.large_file_flag() {
                            let pdu = KeepAlivePduLarge::ref_from_bytes(rest).map_err(|_| {
                                CfdpError::Custom("Failed to parse KeepAlivePduLarge")
                            })?;
                            Ok(PduVariant::KeepAlive(KeepAlivePdu::Large(pdu)))
                        } else {
                            let pdu = KeepAlivePduSmall::ref_from_bytes(rest).map_err(|_| {
                                CfdpError::Custom("Failed to parse KeepAlivePduSmall")
                            })?;
                            Ok(PduVariant::KeepAlive(KeepAlivePdu::Small(pdu)))
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn variable_header(&self) -> Result<&[u8], CfdpError> {
        let len = self.header_fixed.variable_header_len();
        self.rest
            .get(0..len)
            .ok_or_else(|| CfdpError::Custom("Invalid variable header slice"))
    }

    pub(crate) fn variable_header_mut(&mut self) -> Result<&mut [u8], CfdpError> {
        let len = self.header_fixed.variable_header_len();
        self.rest
            .get_mut(0..len)
            .ok_or_else(|| CfdpError::Custom("Invalid variable header slice"))
    }

    /// Sets the fixed part of the header.
    pub fn set_header(&mut self, header: PduHeaderFixedPart) {
        self.header_fixed = header;
    }

    /// Returns the raw rest field as a byte slice.
    pub fn rest(&self) -> &[u8] {
        &self.rest
    }

    /// Returns the total length of the PDU header (fixed + variable parts).
    pub fn header_len(&self) -> usize {
        core::mem::size_of::<PduHeaderFixedPart>() + self.header_fixed.variable_header_len()
    }

    /// Parses and returns a slice for the source entity ID.
    pub fn source_entity_id(&self) -> Result<EntityId, CfdpError> {
        let entity_id_len = self.header_fixed.entity_id_len();
        let bytes = self
            .rest
            .get(0..entity_id_len)
            .ok_or_else(|| CfdpError::Custom("Invalid source entity ID slice"))?;
        EntityId::from_bytes(bytes)
    }

    /// Writes the source entity ID into the variable header.
    pub fn set_source_entity_id(&mut self, source_entity_id: EntityId) -> Result<(), CfdpError> {
        let entity_id_len = self.header_fixed.entity_id_len();
        let var_header_slice = self
            .variable_header_mut()?
            .get_mut(0..entity_id_len)
            .ok_or(CfdpError::Custom("Invalid slice for source ID"))?;

        source_entity_id.write_to_slice(var_header_slice)
    }

    /// Parses and returns a slice for the transaction sequence number.
    pub fn transaction_seq_num(&self) -> Result<TransactionSeqNum, CfdpError> {
        let entity_id_len = self.header_fixed.entity_id_len();
        let txn_seq_num_len = self.header_fixed.txn_seq_num_len();
        let offset = entity_id_len;
        let bytes = self
            .variable_header()?
            .get(offset..offset + txn_seq_num_len)
            .ok_or_else(|| CfdpError::Custom("Invalid transaction sequence number slice"))?;
        TransactionSeqNum::from_bytes(bytes)
    }

    /// Writes the transaction sequence number into the variable header.
    pub fn set_transaction_seq_num(
        &mut self,
        txn_seq_num: TransactionSeqNum,
    ) -> Result<(), CfdpError> {
        let entity_id_len = self.header_fixed.entity_id_len();
        let txn_seq_num_len = self.header_fixed.txn_seq_num_len();
        let offset = entity_id_len;

        let var_header_slice = self
            .variable_header_mut()?
            .get_mut(offset..offset + txn_seq_num_len)
            .ok_or(CfdpError::Custom("Invalid slice for seq num"))?;

        txn_seq_num.write_to_slice(var_header_slice)
    }

    /// Parses and returns a slice for the destination entity ID.
    pub fn set_destination_entity_id(&mut self, dest_entity_id: EntityId) -> Result<(), CfdpError> {
        let entity_id_len = self.header_fixed.entity_id_len();
        let txn_seq_num_len = self.header_fixed.txn_seq_num_len();
        let offset = entity_id_len + txn_seq_num_len;

        let var_header_slice = self
            .variable_header_mut()?
            .get_mut(offset..offset + entity_id_len)
            .ok_or(CfdpError::Custom("Invalid slice for dest ID"))?;

        dest_entity_id.write_to_slice(var_header_slice)
    }

    /// Returns a slice representing the PDU's data field.
    pub fn data_field(&self) -> Result<&[u8], CfdpError> {
        let start = self.header_fixed.variable_header_len();
        self.rest
            .get(start..)
            .ok_or_else(|| CfdpError::Custom("Invalid data field slice"))
    }

    /// Returns a mutable slice representing the PDU's data field.
    pub fn data_field_mut(&mut self) -> Result<&mut [u8], CfdpError> {
        let start = self.header_fixed.variable_header_len();
        self.rest
            .get_mut(start..)
            .ok_or_else(|| CfdpError::Custom("Invalid data field slice"))
    }

    /// A convenience method to parse the entire PDU from a byte buffer.
    pub fn from_bytes(buffer: &[u8]) -> Result<&Pdu, CfdpError> {
        let (header_fixed, _rest) = PduHeaderFixedPart::ref_from_prefix(buffer)
            .map_err(|_| CfdpError::Custom("Failed to parse PDU header fixed part"))?;

        let expected_total_len = header_fixed.total_pdu_len();

        if buffer.len() < expected_total_len {
            return Err(CfdpError::Custom("Buffer too small for complete PDU"));
        }

        Pdu::ref_from_bytes(&buffer[..expected_total_len])
            .map_err(|_| CfdpError::Custom("Failed to parse complete PDU"))
    }

    /// A convenience method to parse the entire PDU from a byte buffer.
    pub fn from_bytes_mut(buffer: &mut [u8]) -> Result<&mut Pdu, CfdpError> {
        let buffer_len = buffer.len();

        let (header_fixed, _rest) =
            PduHeaderFixedPart::mut_from_prefix(buffer).map_err(|_| CfdpError::BufferTooSmall {
                required: core::mem::size_of::<PduHeaderFixedPart>(),
                provided: buffer_len,
            })?;

        let expected_total_len = header_fixed.total_pdu_len();

        if buffer_len < expected_total_len {
            return Err(CfdpError::BufferTooSmall {
                required: expected_total_len,
                provided: buffer_len,
            });
        }

        Ok(Pdu::mut_from_bytes(&mut buffer[..expected_total_len])
            .expect("Buffer size already validated"))
    }
}

#[bon]
impl Pdu {
    /// Builds a new PDU in the given buffer with header and entity IDs.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        header_fixed: PduHeaderFixedPart,
        source_entity_id: EntityId,
        destination_entity_id: EntityId,
        transaction_seq_num: TransactionSeqNum,
    ) -> Result<&'a mut Pdu, CfdpError> {
        let pdu = Pdu::from_bytes_mut(buffer)?;
        pdu.set_header(header_fixed);
        pdu.header_fixed.set_entity_id_len(core::cmp::max(
            source_entity_id.len(),
            destination_entity_id.len(),
        ))?;
        pdu.header_fixed
            .set_txn_seq_num_len(transaction_seq_num.len())?;
        pdu.set_source_entity_id(source_entity_id)?;
        pdu.set_transaction_seq_num(transaction_seq_num)?;
        pdu.set_destination_entity_id(destination_entity_id)?;
        Ok(pdu)
    }
}

impl Deref for Pdu {
    type Target = PduHeaderFixedPart;

    fn deref(&self) -> &Self::Target {
        &self.header_fixed
    }
}

impl DerefMut for Pdu {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.header_fixed
    }
}
