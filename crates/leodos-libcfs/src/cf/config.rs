//! CFDP configuration constants and limits.

use crate::ffi;

/// Number of CF channels.
pub const NUM_CHANNELS: u32 = ffi::CF_NUM_CHANNELS;
/// Maximum NAK segments per PDU.
pub const NAK_MAX_SEGMENTS: u32 = ffi::CF_NAK_MAX_SEGMENTS;
/// Maximum polling directories per channel.
pub const MAX_POLLING_DIR_PER_CHAN: u32 = ffi::CF_MAX_POLLING_DIR_PER_CHAN;
/// Maximum PDU size in bytes.
pub const MAX_PDU_SIZE: u32 = ffi::CF_MAX_PDU_SIZE;
/// Maximum filename (without path).
pub const FILENAME_MAX_NAME: u32 = ffi::CF_FILENAME_MAX_NAME;
/// Maximum filename length.
pub const FILENAME_MAX_LEN: u32 = ffi::CF_FILENAME_MAX_LEN;
/// Extra trailing bytes for PDU encapsulation.
pub const PDU_ENCAPSULATION_EXTRA_TRAILING_BYTES: u32 =
    ffi::CF_PDU_ENCAPSULATION_EXTRA_TRAILING_BYTES;
/// Maximum filename path length.
pub const FILENAME_MAX_PATH: u32 = ffi::CF_FILENAME_MAX_PATH;
/// Software bus pipe depth.
pub const PIPE_DEPTH: u32 = ffi::CF_PIPE_DEPTH;
/// Maximum commanded playback files per channel.
pub const MAX_COMMANDED_PLAYBACK_FILES_PER_CHAN: u32 =
    ffi::CF_MAX_COMMANDED_PLAYBACK_FILES_PER_CHAN;
/// Maximum simultaneous receive transactions.
pub const MAX_SIMULTANEOUS_RX: u32 = ffi::CF_MAX_SIMULTANEOUS_RX;
/// Maximum commanded playback directories per channel.
pub const MAX_COMMANDED_PLAYBACK_DIRECTORIES_PER_CHAN: u32 =
    ffi::CF_MAX_COMMANDED_PLAYBACK_DIRECTORIES_PER_CHAN;
/// Number of history entries per channel.
pub const NUM_HISTORIES_PER_CHANNEL: u32 = ffi::CF_NUM_HISTORIES_PER_CHANNEL;
/// Number of transactions per playback.
pub const NUM_TRANSACTIONS_PER_PLAYBACK: u32 = ffi::CF_NUM_TRANSACTIONS_PER_PLAYBACK;
/// Class 2 CRC chunk size.
pub const R2_CRC_CHUNK_SIZE: u32 = ffi::CF_R2_CRC_CHUNK_SIZE;
/// Receive message timeout.
pub const RCVMSG_TIMEOUT: u32 = ffi::CF_RCVMSG_TIMEOUT;
/// Startup semaphore maximum retries.
pub const STARTUP_SEM_MAX_RETRIES: u32 = ffi::CF_STARTUP_SEM_MAX_RETRIES;
/// Startup semaphore task delay.
pub const STARTUP_SEM_TASK_DELAY: u32 = ffi::CF_STARTUP_SEM_TASK_DELAY;
/// Total number of chunks.
pub const TOTAL_CHUNKS: u32 = ffi::CF_TOTAL_CHUNKS;
/// CF mission revision.
pub const MISSION_REV: u32 = ffi::CF_MISSION_REV;
/// Maximum TLV entries per PDU.
pub const PDU_MAX_TLV: u32 = ffi::CF_PDU_MAX_TLV;
/// Maximum segments per PDU.
pub const PDU_MAX_SEGMENTS: u32 = ffi::CF_PDU_MAX_SEGMENTS;
/// Compound key value.
pub const COMPOUND_KEY: u32 = ffi::CF_COMPOUND_KEY;
/// Value for all channels.
pub const ALL_CHANNELS: u32 = ffi::CF_ALL_CHANNELS;
/// Value for all polling directories.
pub const ALL_POLLDIRS: u32 = ffi::CF_ALL_POLLDIRS;
/// Number of transactions per channel.
pub const NUM_TRANSACTIONS_PER_CHANNEL: u32 = ffi::CF_NUM_TRANSACTIONS_PER_CHANNEL;
/// Total number of transactions.
pub const NUM_TRANSACTIONS: u32 = ffi::CF_NUM_TRANSACTIONS;
/// Total number of history entries.
pub const NUM_HISTORIES: u32 = ffi::CF_NUM_HISTORIES;
/// Number of chunks across all channels.
pub const NUM_CHUNKS_ALL_CHANNELS: u32 = ffi::CF_NUM_CHUNKS_ALL_CHANNELS;

/// Configuration table name.
pub const CONFIG_TABLE_NAME: &[u8; 13] = ffi::CF_CONFIG_TABLE_NAME;
/// Configuration table filename.
pub const CONFIG_TABLE_FILENAME: &[u8; 22] = ffi::CF_CONFIG_TABLE_FILENAME;
/// Command pipe name.
pub const PIPE_NAME: &[u8; 12] = ffi::CF_PIPE_NAME;
/// Channel pipe name prefix.
pub const CHANNEL_PIPE_PREFIX: &[u8; 9] = ffi::CF_CHANNEL_PIPE_PREFIX;
/// Character used to mark truncated filenames.
pub const FILENAME_TRUNCATED: u8 = ffi::CF_FILENAME_TRUNCATED;

/// Generic CF error.
pub const ERROR: i32 = ffi::CF_ERROR;
/// PDU metadata error.
pub const PDU_METADATA_ERROR: i32 = ffi::CF_PDU_METADATA_ERROR;
/// Short PDU error.
pub const SHORT_PDU_ERROR: i32 = ffi::CF_SHORT_PDU_ERROR;
/// Received PDU file size mismatch error.
pub const REC_PDU_FSIZE_MISMATCH_ERROR: i32 = ffi::CF_REC_PDU_FSIZE_MISMATCH_ERROR;
/// Received PDU bad EOF error.
pub const REC_PDU_BAD_EOF_ERROR: i32 = ffi::CF_REC_PDU_BAD_EOF_ERROR;
/// Send PDU no buffer available error.
pub const SEND_PDU_NO_BUF_AVAIL_ERROR: i32 = ffi::CF_SEND_PDU_NO_BUF_AVAIL_ERROR;
/// Send PDU error.
pub const SEND_PDU_ERROR: i32 = ffi::CF_SEND_PDU_ERROR;

/// Configuration table structure.
#[repr(transparent)]
pub struct ConfigTable(pub(crate) ffi::CF_ConfigTable_t);

/// Channel configuration structure.
#[repr(transparent)]
pub struct ChannelConfig(pub(crate) ffi::CF_ChannelConfig_t);

/// Polling directory configuration.
#[repr(transparent)]
pub struct PollDir(pub(crate) ffi::CF_PollDir_t);

impl PollDir {
    /// Returns the polling interval in seconds.
    pub fn interval_sec(&self) -> u32 {
        self.0.interval_sec
    }

    /// Returns the priority for files from this directory.
    pub fn priority(&self) -> u8 {
        self.0.priority
    }

    /// Returns true if polling is enabled for this directory.
    pub fn enabled(&self) -> bool {
        self.0.enabled != 0
    }
}

/// Validates a CF configuration table.
pub fn validate_config_table(tbl_ptr: *mut core::ffi::c_void) -> i32 {
    unsafe { ffi::CF_ValidateConfigTable(tbl_ptr) }
}
