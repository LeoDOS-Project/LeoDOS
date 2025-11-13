//! CFDP Channel types and functions.

use crate::ffi;
use crate::cf::pdu::LogicalPduBuffer;
use crate::cf::types::Direction;

/// CFDP Channel state object.
#[repr(transparent)]
pub struct Channel(pub(crate) ffi::CF_Channel_t);

impl Channel {
    /// Returns the number of commanded transmissions.
    pub fn num_cmd_tx(&self) -> u32 {
        self.0.num_cmd_tx
    }

    /// Returns the outgoing counter.
    pub fn outgoing_counter(&self) -> u32 {
        self.0.outgoing_counter
    }

    /// Returns true if PDU transmission was blocked due to limits.
    pub fn is_tx_blocked(&self) -> bool {
        self.0.tx_blocked
    }

    /// Processes transactions in the channel (tick processing).
    pub fn tick_transactions(&mut self) {
        unsafe { ffi::CF_CFDP_TickTransactions(&mut self.0) }
    }

    /// Processes polling directories for this channel.
    pub fn process_polling_directories(&mut self) {
        unsafe { ffi::CF_CFDP_ProcessPollingDirectories(&mut self.0) }
    }

    /// Receives a PDU on this channel.
    pub fn receive_pdu(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_ReceivePdu(&mut self.0, &mut ph.0) }
    }

    /// Starts the first pending transaction.
    pub fn start_first_pending(&mut self) -> bool {
        unsafe { ffi::CF_CFDP_StartFirstPending(&mut self.0) }
    }

    #[allow(dead_code)]
    pub(crate) fn as_raw_mut_ptr(&mut self) -> *mut ffi::CF_Channel_t {
        &mut self.0
    }
}

/// Playback state for directory playback operations.
#[repr(transparent)]
pub struct Playback(pub(crate) ffi::CF_Playback_t);

impl Playback {
    /// Returns the number of transactions.
    pub fn num_ts(&self) -> u16 {
        self.0.num_ts
    }

    /// Returns the priority.
    pub fn priority(&self) -> u8 {
        self.0.priority
    }

    /// Returns true if the playback is busy.
    pub fn is_busy(&self) -> bool {
        self.0.busy
    }

    /// Returns true if a directory is open.
    pub fn is_dir_open(&self) -> bool {
        self.0.diropen
    }

    /// Returns true if files should be kept after transfer.
    pub fn keep(&self) -> bool {
        self.0.keep
    }
}

impl Channel {
    /// Processes a playback directory.
    pub fn process_playback_directory(&mut self, pb: &mut Playback) {
        unsafe { ffi::CF_CFDP_ProcessPlaybackDirectory(&mut self.0, &mut pb.0) }
    }
}

/// Transaction history entry.
#[repr(transparent)]
pub struct History(pub(crate) ffi::CF_History_t);

impl History {
    /// Returns the direction (RX or TX).
    pub fn direction(&self) -> Direction {
        Direction::try_from(self.0.dir).unwrap_or(Direction::Rx)
    }

    /// Returns the source entity ID.
    pub fn src_eid(&self) -> u32 {
        self.0.src_eid
    }

    /// Returns the peer entity ID.
    pub fn peer_eid(&self) -> u32 {
        self.0.peer_eid
    }

    /// Returns the sequence number.
    pub fn seq_num(&self) -> u32 {
        self.0.seq_num
    }
}

/// Polling directory state.
#[repr(transparent)]
pub struct Poll(pub(crate) ffi::CF_Poll_t);

impl Poll {
    /// Returns true if the timer is set.
    pub fn timer_set(&self) -> bool {
        self.0.timer_set
    }
}

/// CF Engine state structure.
#[repr(transparent)]
pub struct Engine(pub(crate) ffi::CF_Engine_t);

impl Engine {
    /// Returns true if the engine is enabled.
    pub fn enabled(&self) -> bool {
        self.0.enabled
    }

    /// Returns the current sequence number.
    pub fn seq_num(&self) -> u32 {
        self.0.seq_num
    }
}
