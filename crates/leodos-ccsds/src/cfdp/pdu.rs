//! Defines the structures and serialization/deserialization logic for
//! CCSDS File Delivery Protocol (CFDP) Protocol Data Units (PDUs).
//!
//! This module provides safe, zero-copy views and builders for CFDP PDUs,
//! following the pattern used by `SpacePacket`. A single PDU enum, `Pdu<'a>`,
//! holds references to concrete PDU types that are views over the underlying network buffer.
//! This allows for efficient, allocation-free parsing of incoming packets.

use bon::bon;
use core::mem::size_of;
use zerocopy::byteorder::network_endian::U16;
use zerocopy::byteorder::network_endian::U32;
use zerocopy::byteorder::network_endian::U64;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// The unique identifier for a CFDP entity, represented as a 32-bit integer.
pub type EntityId = U32;

/// The sequence number for a transaction, unique to a given source entity.
pub type TransactionSeqNum = U32;

/// The maximum number of missing segment requests that can be included in a single NAK PDU.
pub const MAX_NAK_SEGMENTS: usize = 32;

// --- Unified PDU Enum ---

/// An enum representing a zero-copy view of a parsed PDU.
///
/// Its lifetime `'a` is tied to the underlying receive buffer from which it was parsed.
#[derive(Debug, PartialEq, Eq)]
pub enum Pdu<'a> {
    /// A view of an End of File (EOF) PDU.
    Eof(&'a EofPdu),
    /// A view of a Finished PDU.
    Finished(&'a FinishedPdu),
    /// A view of an Acknowledgment (ACK) PDU.
    Ack(&'a AckPdu),
    /// A view of a Metadata PDU.
    Metadata(&'a MetadataPdu),
    /// A view of a File Data PDU.
    FileData(&'a FileDataPdu),
    /// A view of a Negative Acknowledgment (NAK) PDU.
    Nak(&'a NakPdu),
}

// --- PDU Header and Flags ---

/// The fixed-length header present at the beginning of every CFDP PDU.
///
/// This struct provides a zero-copy view over the header portion of a PDU packet
/// and includes methods to access its bit-packed fields.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, Debug, KnownLayout, Immutable)]
pub struct PduHeader {
    /// Contains bit-packed flags: 3-bit Version, 1-bit PDU Type, 1-bit Direction, 1-bit Tx Mode, 1-bit CRC Flag, 1-bit Large File Flag.
    version_type_dir_tx_crc_large: u8,
    /// The length of the PDU Data Field that follows this header, in octets.
    pub data_field_len: U16,
    /// The length of the Entity ID and Transaction Sequence Number fields in octets, encoded as `(length - 1)`.
    pub id_and_seq_num_len: u8,
    /// The unique identifier of the source CFDP entity.
    pub source_entity_id: EntityId,
    /// The sequence number for this transaction, unique to the source entity.
    pub transaction_seq_num: TransactionSeqNum,
    /// The unique identifier of the destination CFDP entity.
    pub dest_entity_id: EntityId,
}

#[derive(Debug)]
pub enum BuildError {
    BufferTooSmall { required: usize, provided: usize },
}

fn view<T>(buf: &mut [u8]) -> Result<&mut T, BuildError>
where
    T: FromBytes + IntoBytes + Unaligned + KnownLayout,
{
    let required_size = size_of::<T>();
    if buf.len() < required_size {
        return Err(BuildError::BufferTooSmall {
            required: required_size,
            provided: buf.len(),
        });
    }
    Ok(T::mut_from_bytes(&mut buf[..required_size]).expect("Buffer is large enough for type T"))
}

/// Identifies whether the PDU is a File Data PDU or a File Directive PDU.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq)]
pub enum PduType {
    /// The PDU contains file data.
    FileData = 0,
    /// The PDU contains a directive (e.g., Metadata, EOF, Finished).
    FileDirective = 1,
}

// --- Concrete PDU Structs ---

/// A zero-copy view of an End of File (EOF) PDU, signaling the end of file data transmission.
#[repr(C)]
#[derive(Debug, PartialEq, Eq, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct EofPdu {
    /// The directive code for an EOF PDU, always `0x04`.
    pub directive_code: u8,
    /// The raw byte representing the `ConditionCode`. Use `condition_code()` to access safely.
    condition_code_raw: u8,
    /// The checksum of the entire file.
    pub file_checksum: U32,
    /// The total size of the file in octets.
    pub file_size: U64,
}

#[bon]
impl EofPdu {
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        condition_code: ConditionCode,
        file_checksum: U32,
        file_size: U64,
    ) -> Result<&mut EofPdu, BuildError> {
        let eof_pdu = view::<EofPdu>(buffer)?;
        eof_pdu.directive_code = 0x04;
        eof_pdu.condition_code_raw = condition_code as u8;
        eof_pdu.file_checksum = file_checksum;
        eof_pdu.file_size = file_size;
        Ok(eof_pdu)
    }
}

/// A zero-copy view of a Finished PDU, signaling the completion or cancellation of a transaction.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, Debug, KnownLayout, Immutable, PartialEq, Eq)]
pub struct FinishedPdu {
    /// The directive code for a Finished PDU, always `0x05`.
    pub directive_code: u8,
    /// The raw byte representing the `ConditionCode`. Use `condition_code()` to access safely.
    condition_code_raw: u8,
    /// A flag indicating whether the file was delivered.
    pub delivery_code: u8,
    /// A flag indicating the final status of the file at the receiver.
    pub file_status: u8,
}

#[bon]
impl FinishedPdu {
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        condition_code: ConditionCode,
        delivery_code: u8,
        file_status: u8,
    ) -> Result<&mut FinishedPdu, BuildError> {
        let finished_pdu = view::<FinishedPdu>(buffer)?;
        finished_pdu.directive_code = 0x05;
        finished_pdu.condition_code_raw = condition_code as u8;
        finished_pdu.delivery_code = delivery_code;
        finished_pdu.file_status = file_status;
        Ok(finished_pdu)
    }
}

/// A zero-copy view of an Acknowledgment (ACK) PDU.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, Debug, KnownLayout, Immutable, PartialEq, Eq)]
pub struct AckPdu {
    /// The directive code for an ACK PDU, always `0x06`.
    pub directive_code: u8,
    /// The directive code of the PDU being acknowledged (e.g., `0x04` for EOF).
    pub directive_subtype_code: u8,
    /// The raw byte representing the `ConditionCode`. Use `condition_code()` to access safely.
    condition_code_raw: u8,
    /// The status of the transaction at the time of acknowledgment.
    pub transaction_status: u8,
}

#[bon]
impl AckPdu {
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        directive_subtype_code: u8,
        condition_code: ConditionCode,
        transaction_status: u8,
    ) -> Result<&mut AckPdu, BuildError> {
        let ack_pdu = view::<AckPdu>(buffer)?;
        ack_pdu.directive_code = 0x06;
        ack_pdu.directive_subtype_code = directive_subtype_code;
        ack_pdu.condition_code_raw = condition_code as u8;
        ack_pdu.transaction_status = transaction_status;
        Ok(ack_pdu)
    }
}

// --- Structs with Unsized Fields for Variable-Length Data ---

/// A zero-copy view of a Metadata PDU, which signals the start of a file transfer.
#[repr(C)]
#[derive(Debug, IntoBytes, FromBytes, KnownLayout, Immutable)]
pub struct MetadataPdu {
    /// The directive code for a Metadata PDU, always `0x07`.
    pub directive_code: u8,
    /// Flag indicating whether the file is bounded or unbounded.
    pub segmentation_control: u8,
    /// The total size of the file.
    pub file_size: U64,
    /// The trailing, variable-length portion of the PDU containing LV-encoded filenames.
    rest: [u8],
}

#[bon]
impl MetadataPdu {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        segmentation_control: u8,
        file_size: U64,
        source_file_name: &[u8],
        dest_file_name: &[u8],
    ) -> Result<&'a mut MetadataPdu, BuildError> {
        // --- REPLACE THE todo!() WITH THIS IMPLEMENTATION ---
        let required_len = core::mem::size_of::<u8>() * 2 + // directive_code + segmentation_control
            core::mem::size_of::<U64>() +   // file_size
            1 + source_file_name.len() +    // LV for source name
            1 + dest_file_name.len(); // LV for dest name

        if buffer.len() < required_len {
            return Err(BuildError::BufferTooSmall {
                required: required_len,
                provided: buffer.len(),
            });
        }

        let pdu_buf = &mut buffer[..required_len];
        let pdu = MetadataPdu::mut_from_bytes(pdu_buf).expect("Buffer size is checked");

        pdu.directive_code = 0x07;
        pdu.segmentation_control = segmentation_control;
        pdu.file_size = file_size;

        // Write LV fields into the `rest` slice
        let mut offset = 0;
        let rest_mut = &mut pdu.rest;
        // Source name
        rest_mut[offset] = source_file_name.len() as u8;
        offset += 1;
        rest_mut[offset..offset + source_file_name.len()].copy_from_slice(source_file_name);
        offset += source_file_name.len();
        // Dest name
        rest_mut[offset] = dest_file_name.len() as u8;
        offset += 1;
        rest_mut[offset..offset + dest_file_name.len()].copy_from_slice(dest_file_name);

        Ok(pdu)
    }
}

/// A zero-copy view of a File Data PDU, which contains a chunk of the file.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
pub struct FileDataPdu {
    /// The offset within the file where this chunk of data belongs.
    pub offset: U64,
    /// A slice representing the file data itself.
    pub data: [u8],
}

/// A `zerocopy`-compatible struct representing a single missing segment in a NAK PDU.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Debug, PartialEq, Eq)]
pub struct NakSegment {
    /// Start offset of the missing data segment.
    pub offset: U64,
    /// Length of the missing data segment.
    pub length: U64,
}

/// A zero-copy view of a Negative Acknowledgment (NAK) PDU.
#[repr(C)]
#[derive(Debug, IntoBytes, FromBytes, KnownLayout, Immutable)]
pub struct NakPdu {
    /// The directive code for a NAK PDU, always `0x08`.
    pub directive_code: u8,
    /// The start offset of the scope of the NAK.
    pub start_of_scope: U64,
    /// The end offset of the scope of the NAK.
    pub end_of_scope: U64,
    /// The trailing portion containing a series of `NakSegment`s.
    rest: [u8],
}

/// Represents the Condition Code reported in `EOF`, `Finished`, and `ACK` PDUs.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
#[repr(u8)]
pub enum ConditionCode {
    /// No error was detected.
    #[default]
    NoError = 0,
    /// The acknowledgment limit was reached without receiving an expected ACK.
    AckLimitReached = 1,
    /// The keep-alive limit was reached without receiving any PDU for the transaction.
    KeepAliveLimitReached = 2,
    /// An invalid transmission mode was specified.
    InvalidTransmissionMode = 3,
    /// The filestore operation was rejected by the receiver.
    FilestoreRejection = 4,
    /// The file checksum did not match the expected value.
    ChecksumFailure = 5,
    /// The file size did not match the expected value.
    FileSizeError = 6,
    /// The NAK limit was reached without satisfying all missing data requests.
    NakLimitReached = 7,
    /// An inactivity timer expired.
    InactivityDetected = 8,
    /// The check limit was reached.
    CheckLimitReached = 9,
    /// The requested checksum type is not supported.
    UnsupportedChecksumType = 10,
    /// A `SUSPEND` request was received for the transaction.
    SuspendReceived = 14,
    /// A `CANCEL` request was received for the transaction.
    CancelReceived = 15,
}

// --- Safe Accessors and Methods ---

impl EofPdu {
    /// Safely reads the raw condition code byte and converts it to the `ConditionCode` enum.
    pub fn condition_code(&self) -> ConditionCode {
        match self.condition_code_raw {
            1 => ConditionCode::AckLimitReached,
            4 => ConditionCode::FilestoreRejection,
            5 => ConditionCode::ChecksumFailure,
            6 => ConditionCode::FileSizeError,
            _ => ConditionCode::NoError,
        }
    }

    pub fn set_condition_code(&mut self, code: ConditionCode) {
        self.condition_code_raw = code as u8;
    }
}
impl FinishedPdu {
    /// Safely reads the raw condition code byte and converts it to the `ConditionCode` enum.
    pub fn condition_code(&self) -> ConditionCode {
        match self.condition_code_raw {
            1 => ConditionCode::AckLimitReached,
            4 => ConditionCode::FilestoreRejection,
            _ => ConditionCode::NoError,
        }
    }
}

impl AckPdu {
    /// Safely reads the raw condition code byte and converts it to the `ConditionCode` enum.
    pub fn condition_code(&self) -> ConditionCode {
        match self.condition_code_raw {
            1 => ConditionCode::AckLimitReached,
            _ => ConditionCode::NoError,
        }
    }

    pub fn set_condition_code(&mut self, code: ConditionCode) {
        self.condition_code_raw = code as u8;
    }
}

/// Helper to read an LV field and return the slice and remainder.
fn read_lv(bytes: &[u8]) -> Option<(&[u8], &[u8])> {
    let len = *bytes.first()? as usize;
    let data = bytes.get(1..1 + len)?;
    let remainder = bytes.get(1 + len..)?;
    Some((data, remainder))
}

impl MetadataPdu {
    /// Parses the LV-encoded source file name from the `rest` field.
    pub fn source_file_name(&self) -> Option<&[u8]> {
        read_lv(&self.rest).map(|(name, _)| name)
    }
    /// Parses the LV-encoded destination file name from the `rest` field.
    pub fn dest_file_name(&self) -> Option<&[u8]> {
        read_lv(&self.rest).and_then(|(_, remainder)| read_lv(remainder).map(|(name, _)| name))
    }
}
impl NakPdu {
    /// Parses the `rest` field into a slice of `NakSegment`s.
    pub fn segment_requests(&self) -> Option<&[NakSegment]> {
        <[NakSegment]>::ref_from_bytes(&self.rest).ok()
    }
}

#[bon]
impl NakPdu {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        start_of_scope: U64,
        end_of_scope: U64,
        segment_requests: &[(U64, U64)],
    ) -> Result<&'a mut NakPdu, BuildError> {
        let segments_len = segment_requests.len() * core::mem::size_of::<(U64, U64)>();
        let required_len = core::mem::size_of::<u8>() + // directive_code
                           core::mem::size_of::<U64>() * 2 + // scope start/end
                           segments_len;

        if buffer.len() < required_len {
            return Err(BuildError::BufferTooSmall {
                required: required_len,
                provided: buffer.len(),
            });
        }

        let pdu_buf = &mut buffer[..required_len];
        let pdu = NakPdu::mut_from_bytes(pdu_buf).expect("Buffer size is checked");

        pdu.directive_code = 0x08;
        pdu.start_of_scope = start_of_scope;
        pdu.end_of_scope = end_of_scope;

        // Use zerocopy's helpers for slices of structs
        let mut_segments =
            <[NakSegment]>::mut_from_bytes_with_elems(&mut pdu.rest, segment_requests.len())
                .expect("Invalid layout for NakSegment slice");

        // Copy segment requests manually
        for (i, req) in segment_requests.iter().enumerate() {
            mut_segments[i] = NakSegment {
                offset: req.0,
                length: req.1,
            };
        }

        Ok(pdu)
    }
}

// Manual implementations of PartialEq and Eq are required for unsized structs.
impl PartialEq for MetadataPdu {
    fn eq(&self, other: &Self) -> bool {
        self.rest == other.rest
    }
}

impl Eq for MetadataPdu {}

impl PartialEq for FileDataPdu {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
    }
}

impl Eq for FileDataPdu {}

impl PartialEq for NakPdu {
    fn eq(&self, other: &Self) -> bool {
        self.rest == other.rest
    }
}

impl Eq for NakPdu {}

// --- Parsing ---

impl PduHeader {
    /// Returns the `PduType` (FileData or FileDirective) from the header flags.
    pub fn pdu_type(&self) -> PduType {
        if (self.version_type_dir_tx_crc_large & 0b0001_0000) == 0 {
            PduType::FileData
        } else {
            PduType::FileDirective
        }
    }

    /// Sets the `PduType` (FileData or FileDirective) in the header flags.
    pub fn set_pdu_type(&mut self, pdu_type: PduType) {
        let current = self.version_type_dir_tx_crc_large;
        let new_val = if pdu_type == PduType::FileDirective {
            current | 0b0001_0000
        } else {
            current & !0b0001_0000
        };
        self.version_type_dir_tx_crc_large = new_val;
    }
}

/// Parses a raw byte slice into a zero-copy `PduHeader` and `Pdu` view enum.
///
/// This is the main entry point for handling incoming CFDP packets. It is fully
/// zero-copy and performs no allocations.
///
/// # Arguments
/// * `bytes`: A slice representing the complete CFDP packet (header + data field).
///
/// # Returns
/// `Some((header, pdu_view))` on success, or `None` if the slice is too short,
/// the length field is inconsistent, or an unknown directive code is found.
pub fn parse_pdu(bytes: &[u8]) -> Option<(&PduHeader, Pdu<'_>)> {
    if bytes.len() < size_of::<PduHeader>() {
        return None;
    }
    let (header_bytes, data_bytes) = bytes.split_at(size_of::<PduHeader>());
    let header = PduHeader::ref_from_bytes(header_bytes).ok()?;

    if header.data_field_len.get() as usize != data_bytes.len() {
        return None;
    }

    let pdu = match header.pdu_type() {
        PduType::FileData => FileDataPdu::ref_from_bytes(data_bytes)
            .map(Pdu::FileData)
            .ok(),
        PduType::FileDirective => {
            let directive_code = *data_bytes.first()?;
            match directive_code {
                0x04 => EofPdu::ref_from_bytes(data_bytes).map(Pdu::Eof).ok(),
                0x05 => FinishedPdu::ref_from_bytes(data_bytes)
                    .map(Pdu::Finished)
                    .ok(),
                0x06 => AckPdu::ref_from_bytes(data_bytes).map(Pdu::Ack).ok(),
                0x07 => MetadataPdu::ref_from_bytes(data_bytes)
                    .map(Pdu::Metadata)
                    .ok(),
                0x08 => NakPdu::ref_from_bytes(data_bytes).map(Pdu::Nak).ok(),
                _ => None,
            }
        }
    };
    pdu.map(|p| (header, p))
}
