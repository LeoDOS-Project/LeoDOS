//! CFDP command and telemetry message types.

use crate::ffi;

/// Housekeeping packet payload.
#[repr(transparent)]
pub struct HkPacketPayload(pub(crate) ffi::CF_HkPacket_Payload_t);

/// Housekeeping packet.
#[repr(transparent)]
pub struct HkPacket(pub(crate) ffi::CF_HkPacket_t);

impl HkPacket {
    /// Returns a reference to the payload.
    pub fn payload(&self) -> &HkPacketPayload {
        unsafe { &*(&self.0.Payload as *const _ as *const HkPacketPayload) }
    }
}

/// End of transaction packet payload.
#[repr(transparent)]
pub struct EotPacketPayload(pub(crate) ffi::CF_EotPacket_Payload_t);

impl EotPacketPayload {
    /// Returns the channel number.
    pub fn channel(&self) -> u32 {
        self.0.channel
    }

    /// Returns the direction (0=RX, 1=TX).
    pub fn direction(&self) -> u32 {
        self.0.direction
    }

    /// Returns the transaction state.
    pub fn state(&self) -> u32 {
        self.0.state
    }

    /// Returns the transaction status.
    pub fn txn_stat(&self) -> u32 {
        self.0.txn_stat
    }

    /// Returns the source entity ID.
    pub fn src_eid(&self) -> u32 {
        self.0.src_eid
    }

    /// Returns the peer entity ID.
    pub fn peer_eid(&self) -> u32 {
        self.0.peer_eid
    }

    /// Returns the transaction sequence number.
    pub fn seq_num(&self) -> u32 {
        self.0.seq_num
    }
}

/// End of transaction packet.
#[repr(transparent)]
pub struct EotPacket(pub(crate) ffi::CF_EotPacket_t);

impl EotPacket {
    /// Returns a reference to the payload.
    pub fn payload(&self) -> &EotPacketPayload {
        unsafe { &*(&self.0.Payload as *const _ as *const EotPacketPayload) }
    }
}

/// Housekeeping command counters.
#[repr(transparent)]
pub struct HkCmdCounters(pub(crate) ffi::CF_HkCmdCounters_t);

impl HkCmdCounters {
    /// Returns the command counter.
    pub fn cmd(&self) -> u16 {
        self.0.cmd
    }

    /// Returns the error counter.
    pub fn err(&self) -> u16 {
        self.0.err
    }
}

/// Housekeeping sent counters.
#[repr(transparent)]
pub struct HkSent(pub(crate) ffi::CF_HkSent_t);

impl HkSent {
    /// Returns the file data sent counter.
    pub fn file_data(&self) -> u64 {
        self.0.file_data_bytes
    }

    /// Returns the PDUs sent counter.
    pub fn pdu(&self) -> u32 {
        self.0.pdu
    }

    /// Returns the NAKs sent counter.
    pub fn nak_segment(&self) -> u32 {
        self.0.nak_segment_requests
    }
}

/// Housekeeping receive counters.
#[repr(transparent)]
pub struct HkRecv(pub(crate) ffi::CF_HkRecv_t);

impl HkRecv {
    /// Returns the file data received counter.
    pub fn file_data(&self) -> u64 {
        self.0.file_data_bytes
    }

    /// Returns the PDUs received counter.
    pub fn pdu(&self) -> u32 {
        self.0.pdu
    }

    /// Returns the dropped PDUs counter.
    pub fn dropped(&self) -> u16 {
        self.0.dropped
    }

    /// Returns the error counter.
    pub fn error(&self) -> u32 {
        self.0.error
    }

    /// Returns the spurious counter.
    pub fn spurious(&self) -> u16 {
        self.0.spurious
    }
}

/// Housekeeping fault counters.
#[repr(transparent)]
pub struct HkFault(pub(crate) ffi::CF_HkFault_t);

/// Housekeeping counters aggregate.
#[repr(transparent)]
pub struct HkCounters(pub(crate) ffi::CF_HkCounters_t);

impl HkCounters {
    /// Returns a reference to the sent counters.
    pub fn sent(&self) -> &HkSent {
        unsafe { &*(&self.0.sent as *const _ as *const HkSent) }
    }

    /// Returns a reference to the receive counters.
    pub fn recv(&self) -> &HkRecv {
        unsafe { &*(&self.0.recv as *const _ as *const HkRecv) }
    }

    /// Returns a reference to the fault counters.
    pub fn fault(&self) -> &HkFault {
        unsafe { &*(&self.0.fault as *const _ as *const HkFault) }
    }
}

/// Housekeeping channel data.
#[repr(transparent)]
pub struct HkChannelData(pub(crate) ffi::CF_HkChannel_Data_t);

impl HkChannelData {
    /// Returns the frozen channel indicator.
    pub fn frozen(&self) -> u8 {
        self.0.frozen
    }

    /// Returns a reference to the counters.
    pub fn counters(&self) -> &HkCounters {
        unsafe { &*(&self.0.counters as *const _ as *const HkCounters) }
    }
}

/// Transaction filenames structure.
#[repr(transparent)]
pub struct TxnFilenames(pub(crate) ffi::CF_TxnFilenames_t);

/// No-op command.
#[repr(transparent)]
pub struct NoopCmd(pub(crate) ffi::CF_NoopCmd_t);

/// Enable engine command.
#[repr(transparent)]
pub struct EnableEngineCmd(pub(crate) ffi::CF_EnableEngineCmd_t);

/// Disable engine command.
#[repr(transparent)]
pub struct DisableEngineCmd(pub(crate) ffi::CF_DisableEngineCmd_t);

/// Reset counters command.
#[repr(transparent)]
pub struct ResetCountersCmd(pub(crate) ffi::CF_ResetCountersCmd_t);

/// Freeze command.
#[repr(transparent)]
pub struct FreezeCmd(pub(crate) ffi::CF_FreezeCmd_t);

/// Thaw command.
#[repr(transparent)]
pub struct ThawCmd(pub(crate) ffi::CF_ThawCmd_t);

/// Enable dequeue command.
#[repr(transparent)]
pub struct EnableDequeueCmd(pub(crate) ffi::CF_EnableDequeueCmd_t);

/// Disable dequeue command.
#[repr(transparent)]
pub struct DisableDequeueCmd(pub(crate) ffi::CF_DisableDequeueCmd_t);

/// Enable directory polling command.
#[repr(transparent)]
pub struct EnableDirPollingCmd(pub(crate) ffi::CF_EnableDirPollingCmd_t);

/// Disable directory polling command.
#[repr(transparent)]
pub struct DisableDirPollingCmd(pub(crate) ffi::CF_DisableDirPollingCmd_t);

/// Purge queue command.
#[repr(transparent)]
pub struct PurgeQueueCmd(pub(crate) ffi::CF_PurgeQueueCmd_t);

/// Get parameter command.
#[repr(transparent)]
pub struct GetParamCmd(pub(crate) ffi::CF_GetParamCmd_t);

/// Set parameter command.
#[repr(transparent)]
pub struct SetParamCmd(pub(crate) ffi::CF_SetParamCmd_t);

/// Transmit file command.
#[repr(transparent)]
pub struct TxFileCmd(pub(crate) ffi::CF_TxFileCmd_t);

/// Write queue command.
#[repr(transparent)]
pub struct WriteQueueCmd(pub(crate) ffi::CF_WriteQueueCmd_t);

/// Playback directory command.
#[repr(transparent)]
pub struct PlaybackDirCmd(pub(crate) ffi::CF_PlaybackDirCmd_t);

/// Suspend command.
#[repr(transparent)]
pub struct SuspendCmd(pub(crate) ffi::CF_SuspendCmd_t);

/// Resume command.
#[repr(transparent)]
pub struct ResumeCmd(pub(crate) ffi::CF_ResumeCmd_t);

/// Cancel command.
#[repr(transparent)]
pub struct CancelCmd(pub(crate) ffi::CF_CancelCmd_t);

/// Abandon command.
#[repr(transparent)]
pub struct AbandonCmd(pub(crate) ffi::CF_AbandonCmd_t);

/// Send housekeeping command.
#[repr(transparent)]
pub struct SendHkCmd(pub(crate) ffi::CF_SendHkCmd_t);

/// Wakeup command.
#[repr(transparent)]
pub struct WakeupCmd(pub(crate) ffi::CF_WakeupCmd_t);

/// Get parameter payload.
#[repr(transparent)]
pub struct GetParamPayload(pub(crate) ffi::CF_GetParam_Payload_t);

/// Set parameter payload.
#[repr(transparent)]
pub struct SetParamPayload(pub(crate) ffi::CF_SetParam_Payload_t);

/// Transmit file payload.
#[repr(transparent)]
pub struct TxFilePayload(pub(crate) ffi::CF_TxFile_Payload_t);

/// Write queue payload.
#[repr(transparent)]
pub struct WriteQueuePayload(pub(crate) ffi::CF_WriteQueue_Payload_t);

/// Transaction payload.
#[repr(transparent)]
pub struct TransactionPayload(pub(crate) ffi::CF_Transaction_Payload_t);
