use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;

use crate::transport::cfdp::CfdpError;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;

/// The fixed-size (4-byte) portion at the start of every CFDP PDU header.
/// This struct can be safely read from any PDU to determine the lengths
/// of the variable-sized fields that follow.
#[repr(C)]
#[derive(Copy, Clone, Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct PduHeaderFixedPart {
    /// An 8-bit field containing Version (3), PDU Type (1), Direction (1),
    /// Tx Mode (1), CRC Flag (1), and Large File Flag (1).
    version_and_flags: u8,
    /// The length of the PDU Data Field in octets.
    data_field_len: U16,
    /// An 8-bit field containing Segmentation Control (1), Entity ID Length (3),
    /// Segment Metadata Flag (1), and Txn Sequence Number Length (3).
    lengths_and_metadata_flag: u8,
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

/// Indicates the direction of the PDU relative to the file transfer direction.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Direction {
    /// Toward the entity that receives the file.
    TowardReceiver = 0,
    /// Toward the entity that sends the file.
    TowardSender = 1,
}

/// The reliability mode for the transaction.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum TransmissionMode {
    /// Reliable mode with acknowledgments.
    Acknowledged = 0,
    /// Unreliable, best-effort mode.
    Unacknowledged = 1,
}

// In PduType's file or a common types file
impl From<bool> for PduType {
    fn from(val: bool) -> Self {
        if !val {
            PduType::FileData
        } else {
            PduType::FileDirective
        }
    }
}

// In Direction's file or a common types file
impl From<bool> for Direction {
    fn from(val: bool) -> Self {
        if !val {
            Direction::TowardReceiver
        } else {
            Direction::TowardSender
        }
    }
}

// In TransmissionMode's file or a common types file
impl From<bool> for TransmissionMode {
    fn from(val: bool) -> Self {
        if !val {
            TransmissionMode::Acknowledged
        } else {
            TransmissionMode::Unacknowledged
        }
    }
}

#[rustfmt::skip]
mod bitmasks {
    // Bit masks for the initial flags of PduHeaderFixedPart.
    pub const VERSION_MASK: u8 =         0b_11100000;
    pub const PDU_TYPE_MASK: u8 =        0b_00010000;
    pub const DIRECTION_MASK: u8 =       0b_00001000;
    pub const TX_MODE_MASK: u8 =         0b_00000100;
    pub const CRC_FLAG_MASK: u8 =        0b_00000010;
    pub const LARGE_FILE_FLAG_MASK: u8 = 0b_00000001;

    // Bit masks for the length flags of PduHeaderFixedPart.
    pub const SEG_CTRL_MASK: u8 =                  0b_10000000;
    pub const ENTITY_ID_LEN_MINUS_ONE_MASK: u8 =   0b_01110000;
    pub const SEG_META_FLAG_MASK: u8 =             0b_00001000;
    pub const TXN_SEQ_NUM_LEN_MINUS_ONE_MASK: u8 = 0b_00000111;
}

use bitmasks::*;

#[bon]
impl PduHeaderFixedPart {
    #[builder]
    pub fn new(
        version: u8,
        pdu_type: PduType,
        direction: Direction,
        tx_mode: TransmissionMode,
        crc_flag: bool,
        large_file_flag: bool,
        data_field_len: u16,
        seg_ctrl: bool,
        seg_meta_flag: bool,
    ) -> Result<Self, CfdpError> {
        let mut header = PduHeaderFixedPart {
            version_and_flags: 0,
            data_field_len: U16::new(0),
            lengths_and_metadata_flag: 0,
        };

        header.set_version(version);
        header.set_pdu_type(pdu_type);
        header.set_direction(direction);
        header.set_tx_mode(tx_mode);
        header.set_crc_flag(crc_flag);
        header.set_large_file_flag(large_file_flag);
        header.set_data_field_len(data_field_len);
        header.set_segmentation_control(seg_ctrl);
        header.set_segment_metadata_flag(seg_meta_flag);
        // These fields are automatically set by the PDU builder once the lengths
        // of the entity IDs and seq num are known
        header.set_entity_id_len(1)?;
        header.set_txn_seq_num_len(1)?;

        Ok(header)
    }
}

impl PduHeaderFixedPart {
    // --- Accessors for fields within `version_and_flags` ---
    pub fn version(&self) -> u8 {
        get_bits_u8(self.version_and_flags, VERSION_MASK)
    }
    pub fn set_version(&mut self, version: u8) {
        set_bits_u8(&mut self.version_and_flags, VERSION_MASK, version);
    }

    pub fn pdu_type(&self) -> PduType {
        PduType::from(get_bits_u8(self.version_and_flags, PDU_TYPE_MASK) == 1)
    }
    pub fn set_pdu_type(&mut self, pdu_type: PduType) {
        let val = match pdu_type {
            PduType::FileData => 0,
            PduType::FileDirective => 1,
        };
        set_bits_u8(&mut self.version_and_flags, PDU_TYPE_MASK, val);
    }

    pub fn direction(&self) -> Direction {
        Direction::from(get_bits_u8(self.version_and_flags, DIRECTION_MASK) == 1)
    }
    pub fn set_direction(&mut self, direction: Direction) {
        set_bits_u8(&mut self.version_and_flags, DIRECTION_MASK, direction as u8);
    }

    pub fn tx_mode(&self) -> TransmissionMode {
        TransmissionMode::from(get_bits_u8(self.version_and_flags, TX_MODE_MASK) == 1)
    }
    pub fn set_tx_mode(&mut self, tx_mode: TransmissionMode) {
        set_bits_u8(&mut self.version_and_flags, TX_MODE_MASK, tx_mode as u8);
    }

    pub fn crc_flag(&self) -> bool {
        get_bits_u8(self.version_and_flags, CRC_FLAG_MASK) == 1
    }
    pub fn set_crc_flag(&mut self, crc_flag: bool) {
        let val = if crc_flag { 1 } else { 0 };
        set_bits_u8(&mut self.version_and_flags, CRC_FLAG_MASK, val);
    }

    pub fn large_file_flag(&self) -> bool {
        get_bits_u8(self.version_and_flags, LARGE_FILE_FLAG_MASK) == 1
    }
    pub fn set_large_file_flag(&mut self, large_file_flag: bool) {
        let val = if large_file_flag { 1 } else { 0 };
        set_bits_u8(&mut self.version_and_flags, LARGE_FILE_FLAG_MASK, val);
    }

    // --- Accessor for `data_field_len` ---
    pub fn data_field_len(&self) -> usize {
        self.data_field_len.get() as usize
    }
    pub fn set_data_field_len(&mut self, len: u16) {
        self.data_field_len.set(len);
    }

    // --- Accessors for fields within `lengths_and_metadata_flag` ---
    pub fn segmentation_control(&self) -> bool {
        get_bits_u8(self.lengths_and_metadata_flag, SEG_CTRL_MASK) == 1
    }
    pub fn set_segmentation_control(&mut self, seg_ctrl: bool) {
        let val = if seg_ctrl { 1 } else { 0 };
        set_bits_u8(&mut self.lengths_and_metadata_flag, SEG_CTRL_MASK, val);
    }

    pub fn entity_id_len(&self) -> usize {
        let val = get_bits_u8(self.lengths_and_metadata_flag, ENTITY_ID_LEN_MINUS_ONE_MASK);
        val as usize + 1
    }
    pub fn set_entity_id_len(&mut self, len: usize) -> Result<(), CfdpError> {
        if len == 0 || len > 8 {
            return Err(CfdpError::Custom(
                "Entity ID length must be between 1 and 8",
            ));
        }
        set_bits_u8(
            &mut self.lengths_and_metadata_flag,
            ENTITY_ID_LEN_MINUS_ONE_MASK,
            len as u8 - 1,
        );
        Ok(())
    }

    pub fn segment_metadata_flag(&self) -> bool {
        get_bits_u8(self.lengths_and_metadata_flag, SEG_META_FLAG_MASK) == 1
    }
    pub fn set_segment_metadata_flag(&mut self, seg_meta_flag: bool) {
        let val = if seg_meta_flag { 1 } else { 0 };
        set_bits_u8(&mut self.lengths_and_metadata_flag, SEG_META_FLAG_MASK, val);
    }

    pub fn txn_seq_num_len(&self) -> usize {
        let val = get_bits_u8(
            self.lengths_and_metadata_flag,
            TXN_SEQ_NUM_LEN_MINUS_ONE_MASK,
        );
        val as usize + 1
    }
    pub fn set_txn_seq_num_len(&mut self, len: usize) -> Result<(), CfdpError> {
        if len == 0 || len > 8 {
            return Err(CfdpError::Custom(
                "Transaction Sequence Number length must be between 1 and 8",
            ));
        }
        set_bits_u8(
            &mut self.lengths_and_metadata_flag,
            TXN_SEQ_NUM_LEN_MINUS_ONE_MASK,
            len as u8 - 1,
        );
        Ok(())
    }

    pub fn fixed_header_len(&self) -> usize {
        core::mem::size_of::<PduHeaderFixedPart>()
    }

    /// Calculates the length of the variable-sized portion of the PDU header
    pub fn variable_header_len(&self) -> usize {
        let entity_id_len = self.entity_id_len();
        let txn_seq_num_len = self.txn_seq_num_len();
        entity_id_len * 2 + txn_seq_num_len
    }

    pub fn total_header_len(&self) -> usize {
        self.fixed_header_len() + self.variable_header_len()
    }

    pub fn total_pdu_len(&self) -> usize {
        self.total_header_len() + self.data_field_len()
    }
}
