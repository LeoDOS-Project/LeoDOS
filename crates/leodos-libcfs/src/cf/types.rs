//! Core CFDP types and enums per CCSDS 727.0-B-5.

use crate::ffi;

/// Entity ID type (32-bit).
pub type EntityId = ffi::CF_EntityId_t;
/// Transaction sequence number type (32-bit).
pub type TransactionSeq = ffi::CF_TransactionSeq_t;
/// File size type (32-bit).
pub type FileSize = ffi::CF_FileSize_t;
/// Chunk index type.
pub type ChunkIdx = ffi::CF_ChunkIdx_t;
/// Chunk offset type.
pub type ChunkOffset = ffi::CF_ChunkOffset_t;
/// Chunk size type.
pub type ChunkSize = ffi::CF_ChunkSize_t;
/// Timer ticks type.
pub type TimerTicks = ffi::CF_Timer_Ticks_t;
/// Timer seconds type.
pub type TimerSeconds = ffi::CF_Timer_Seconds_t;

/// CFDP File Directive codes indicating the type of file directive PDU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FileDirective {
    /// End of File directive.
    Eof = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_EOF,
    /// Finished directive.
    Fin = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_FIN,
    /// Acknowledge directive.
    Ack = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_ACK,
    /// Metadata directive.
    Metadata = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_METADATA,
    /// Negative Acknowledge directive.
    Nak = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_NAK,
    /// Prompt directive.
    Prompt = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_PROMPT,
    /// Keep Alive directive.
    KeepAlive = ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_KEEP_ALIVE,
}

impl TryFrom<ffi::CF_CFDP_FileDirective_t> for FileDirective {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_FileDirective_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_EOF => Ok(Self::Eof),
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_FIN => Ok(Self::Fin),
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_ACK => Ok(Self::Ack),
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_METADATA => Ok(Self::Metadata),
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_NAK => Ok(Self::Nak),
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_PROMPT => Ok(Self::Prompt),
            ffi::CF_CFDP_FileDirective_t_CF_CFDP_FileDirective_KEEP_ALIVE => Ok(Self::KeepAlive),
            _ => Err(()),
        }
    }
}

/// CFDP Condition Codes indicating transaction status or fault conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ConditionCode {
    /// No error condition.
    NoError = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_NO_ERROR,
    /// Positive acknowledgment limit reached.
    PosAckLimitReached = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_POS_ACK_LIMIT_REACHED,
    /// Keep alive limit reached.
    KeepAliveLimitReached = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_KEEP_ALIVE_LIMIT_REACHED,
    /// Invalid transmission mode requested.
    InvalidTransmissionMode = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_INVALID_TRANSMISSION_MODE,
    /// Filestore rejection occurred.
    FilestoreRejection = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_FILESTORE_REJECTION,
    /// File checksum failure detected.
    FileChecksumFailure = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_FILE_CHECKSUM_FAILURE,
    /// File size error detected.
    FileSizeError = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_FILE_SIZE_ERROR,
    /// NAK limit reached.
    NakLimitReached = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_NAK_LIMIT_REACHED,
    /// Inactivity detected on transaction.
    InactivityDetected = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_INACTIVITY_DETECTED,
    /// Invalid file structure detected.
    InvalidFileStructure = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_INVALID_FILE_STRUCTURE,
    /// Check limit reached.
    CheckLimitReached = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_CHECK_LIMIT_REACHED,
    /// Unsupported checksum type.
    UnsupportedChecksumType = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_UNSUPPORTED_CHECKSUM_TYPE,
    /// Suspend request received.
    SuspendRequestReceived = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_SUSPEND_REQUEST_RECEIVED,
    /// Cancel request received.
    CancelRequestReceived = ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_CANCEL_REQUEST_RECEIVED,
}

impl TryFrom<ffi::CF_CFDP_ConditionCode_t> for ConditionCode {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_ConditionCode_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_NO_ERROR => Ok(Self::NoError),
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_POS_ACK_LIMIT_REACHED => {
                Ok(Self::PosAckLimitReached)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_KEEP_ALIVE_LIMIT_REACHED => {
                Ok(Self::KeepAliveLimitReached)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_INVALID_TRANSMISSION_MODE => {
                Ok(Self::InvalidTransmissionMode)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_FILESTORE_REJECTION => {
                Ok(Self::FilestoreRejection)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_FILE_CHECKSUM_FAILURE => {
                Ok(Self::FileChecksumFailure)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_FILE_SIZE_ERROR => {
                Ok(Self::FileSizeError)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_NAK_LIMIT_REACHED => {
                Ok(Self::NakLimitReached)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_INACTIVITY_DETECTED => {
                Ok(Self::InactivityDetected)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_INVALID_FILE_STRUCTURE => {
                Ok(Self::InvalidFileStructure)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_CHECK_LIMIT_REACHED => {
                Ok(Self::CheckLimitReached)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_UNSUPPORTED_CHECKSUM_TYPE => {
                Ok(Self::UnsupportedChecksumType)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_SUSPEND_REQUEST_RECEIVED => {
                Ok(Self::SuspendRequestReceived)
            }
            ffi::CF_CFDP_ConditionCode_t_CF_CFDP_ConditionCode_CANCEL_REQUEST_RECEIVED => {
                Ok(Self::CancelRequestReceived)
            }
            _ => Err(()),
        }
    }
}

/// CFDP TLV (Type-Length-Value) types for optional PDU fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TlvType {
    /// Filestore request TLV.
    FilestoreRequest = ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FILESTORE_REQUEST,
    /// Filestore response TLV.
    FilestoreResponse = ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FILESTORE_RESPONSE,
    /// Message to user TLV.
    MessageToUser = ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_MESSAGE_TO_USER,
    /// Fault handler override TLV.
    FaultHandlerOverride = ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FAULT_HANDLER_OVERRIDE,
    /// Flow label TLV.
    FlowLabel = ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FLOW_LABEL,
    /// Entity ID TLV.
    EntityId = ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_ENTITY_ID,
}

impl TryFrom<ffi::CF_CFDP_TlvType_t> for TlvType {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_TlvType_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FILESTORE_REQUEST => Ok(Self::FilestoreRequest),
            ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FILESTORE_RESPONSE => Ok(Self::FilestoreResponse),
            ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_MESSAGE_TO_USER => Ok(Self::MessageToUser),
            ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FAULT_HANDLER_OVERRIDE => {
                Ok(Self::FaultHandlerOverride)
            }
            ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_FLOW_LABEL => Ok(Self::FlowLabel),
            ffi::CF_CFDP_TlvType_t_CF_CFDP_TLV_TYPE_ENTITY_ID => Ok(Self::EntityId),
            _ => Err(()),
        }
    }
}

/// CFDP ACK transaction status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AckTxnStatus {
    /// Transaction status undefined.
    Undefined = ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_UNDEFINED,
    /// Transaction is active.
    Active = ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_ACTIVE,
    /// Transaction is terminated.
    Terminated = ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_TERMINATED,
    /// Transaction is unrecognized.
    Unrecognized = ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_UNRECOGNIZED,
}

impl TryFrom<ffi::CF_CFDP_AckTxnStatus_t> for AckTxnStatus {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_AckTxnStatus_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_UNDEFINED => Ok(Self::Undefined),
            ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_ACTIVE => Ok(Self::Active),
            ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_TERMINATED => Ok(Self::Terminated),
            ffi::CF_CFDP_AckTxnStatus_t_CF_CFDP_AckTxnStatus_UNRECOGNIZED => Ok(Self::Unrecognized),
            _ => Err(()),
        }
    }
}

/// CFDP Finished PDU delivery code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FinDeliveryCode {
    /// File delivery complete.
    Complete = ffi::CF_CFDP_FinDeliveryCode_t_CF_CFDP_FinDeliveryCode_COMPLETE,
    /// File delivery incomplete.
    Incomplete = ffi::CF_CFDP_FinDeliveryCode_t_CF_CFDP_FinDeliveryCode_INCOMPLETE,
}

impl TryFrom<ffi::CF_CFDP_FinDeliveryCode_t> for FinDeliveryCode {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_FinDeliveryCode_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_FinDeliveryCode_t_CF_CFDP_FinDeliveryCode_COMPLETE => Ok(Self::Complete),
            ffi::CF_CFDP_FinDeliveryCode_t_CF_CFDP_FinDeliveryCode_INCOMPLETE => Ok(Self::Incomplete),
            _ => Err(()),
        }
    }
}

/// CFDP Finished PDU file status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FinFileStatus {
    /// File was discarded deliberately.
    Discarded = ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_DISCARDED,
    /// File was discarded due to filestore rejection.
    DiscardedFilestore = ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_DISCARDED_FILESTORE,
    /// File was retained successfully.
    Retained = ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_RETAINED,
    /// File status unreported.
    Unreported = ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_UNREPORTED,
}

impl TryFrom<ffi::CF_CFDP_FinFileStatus_t> for FinFileStatus {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_FinFileStatus_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_DISCARDED => Ok(Self::Discarded),
            ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_DISCARDED_FILESTORE => {
                Ok(Self::DiscardedFilestore)
            }
            ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_RETAINED => Ok(Self::Retained),
            ffi::CF_CFDP_FinFileStatus_t_CF_CFDP_FinFileStatus_UNREPORTED => Ok(Self::Unreported),
            _ => Err(()),
        }
    }
}

/// CF transaction state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TxnState {
    /// Undefined state.
    Undef = ffi::CF_TxnState_t_CF_TxnState_UNDEF,
    /// Initial state.
    Init = ffi::CF_TxnState_t_CF_TxnState_INIT,
    /// Class 1 receiver state.
    R1 = ffi::CF_TxnState_t_CF_TxnState_R1,
    /// Class 1 sender state.
    S1 = ffi::CF_TxnState_t_CF_TxnState_S1,
    /// Class 2 receiver state.
    R2 = ffi::CF_TxnState_t_CF_TxnState_R2,
    /// Class 2 sender state.
    S2 = ffi::CF_TxnState_t_CF_TxnState_S2,
    /// Transaction dropped.
    Drop = ffi::CF_TxnState_t_CF_TxnState_DROP,
    /// Transaction on hold.
    Hold = ffi::CF_TxnState_t_CF_TxnState_HOLD,
}

impl TryFrom<ffi::CF_TxnState_t> for TxnState {
    type Error = ();

    fn try_from(val: ffi::CF_TxnState_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_TxnState_t_CF_TxnState_UNDEF => Ok(Self::Undef),
            ffi::CF_TxnState_t_CF_TxnState_INIT => Ok(Self::Init),
            ffi::CF_TxnState_t_CF_TxnState_R1 => Ok(Self::R1),
            ffi::CF_TxnState_t_CF_TxnState_S1 => Ok(Self::S1),
            ffi::CF_TxnState_t_CF_TxnState_R2 => Ok(Self::R2),
            ffi::CF_TxnState_t_CF_TxnState_S2 => Ok(Self::S2),
            ffi::CF_TxnState_t_CF_TxnState_DROP => Ok(Self::Drop),
            ffi::CF_TxnState_t_CF_TxnState_HOLD => Ok(Self::Hold),
            _ => Err(()),
        }
    }
}

/// CF transmit sub-states within the sender state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TxSubState {
    /// Normal data transmission.
    DataNormal = ffi::CF_TxSubState_t_CF_TxSubState_DATA_NORMAL,
    /// EOF has been sent.
    DataEof = ffi::CF_TxSubState_t_CF_TxSubState_DATA_EOF,
    /// Filestore operations in progress.
    Filestore = ffi::CF_TxSubState_t_CF_TxSubState_FILESTORE,
    /// Transaction complete.
    Complete = ffi::CF_TxSubState_t_CF_TxSubState_COMPLETE,
}

impl TryFrom<ffi::CF_TxSubState_t> for TxSubState {
    type Error = ();

    fn try_from(val: ffi::CF_TxSubState_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_TxSubState_t_CF_TxSubState_DATA_NORMAL => Ok(Self::DataNormal),
            ffi::CF_TxSubState_t_CF_TxSubState_DATA_EOF => Ok(Self::DataEof),
            ffi::CF_TxSubState_t_CF_TxSubState_FILESTORE => Ok(Self::Filestore),
            ffi::CF_TxSubState_t_CF_TxSubState_COMPLETE => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

/// CF receive sub-states within the receiver state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RxSubState {
    /// Normal data reception.
    DataNormal = ffi::CF_RxSubState_t_CF_RxSubState_DATA_NORMAL,
    /// EOF has been received.
    DataEof = ffi::CF_RxSubState_t_CF_RxSubState_DATA_EOF,
    /// Validating received file.
    Validate = ffi::CF_RxSubState_t_CF_RxSubState_VALIDATE,
    /// Filestore operations in progress.
    Filestore = ffi::CF_RxSubState_t_CF_RxSubState_FILESTORE,
    /// Waiting for FIN-ACK.
    FinAck = ffi::CF_RxSubState_t_CF_RxSubState_FINACK,
    /// Transaction complete.
    Complete = ffi::CF_RxSubState_t_CF_RxSubState_COMPLETE,
}

impl TryFrom<ffi::CF_RxSubState_t> for RxSubState {
    type Error = ();

    fn try_from(val: ffi::CF_RxSubState_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_RxSubState_t_CF_RxSubState_DATA_NORMAL => Ok(Self::DataNormal),
            ffi::CF_RxSubState_t_CF_RxSubState_DATA_EOF => Ok(Self::DataEof),
            ffi::CF_RxSubState_t_CF_RxSubState_VALIDATE => Ok(Self::Validate),
            ffi::CF_RxSubState_t_CF_RxSubState_FILESTORE => Ok(Self::Filestore),
            ffi::CF_RxSubState_t_CF_RxSubState_FINACK => Ok(Self::FinAck),
            ffi::CF_RxSubState_t_CF_RxSubState_COMPLETE => Ok(Self::Complete),
            _ => Err(()),
        }
    }
}

/// CF transaction direction (receive or transmit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Direction {
    /// Receiving direction.
    Rx = ffi::CF_Direction_t_CF_Direction_RX,
    /// Transmitting direction.
    Tx = ffi::CF_Direction_t_CF_Direction_TX,
}

impl TryFrom<ffi::CF_Direction_t> for Direction {
    type Error = ();

    fn try_from(val: ffi::CF_Direction_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_Direction_t_CF_Direction_RX => Ok(Self::Rx),
            ffi::CF_Direction_t_CF_Direction_TX => Ok(Self::Tx),
            _ => Err(()),
        }
    }
}

/// CFDP Class (transmission mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CfdpClass {
    /// Class 1 - Unacknowledged (unreliable) mode.
    Class1 = ffi::CF_CFDP_Class_t_CF_CFDP_CLASS_1,
    /// Class 2 - Acknowledged (reliable) mode.
    Class2 = ffi::CF_CFDP_Class_t_CF_CFDP_CLASS_2,
}

impl TryFrom<ffi::CF_CFDP_Class_t> for CfdpClass {
    type Error = ();

    fn try_from(val: ffi::CF_CFDP_Class_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_CFDP_Class_t_CF_CFDP_CLASS_1 => Ok(Self::Class1),
            ffi::CF_CFDP_Class_t_CF_CFDP_CLASS_2 => Ok(Self::Class2),
            _ => Err(()),
        }
    }
}

impl From<CfdpClass> for ffi::CF_CFDP_Class_t {
    fn from(val: CfdpClass) -> Self {
        val as ffi::CF_CFDP_Class_t
    }
}

/// CF Transaction Status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum TxnStatus {
    /// Status undefined/not yet set.
    Undefined = ffi::CF_TxnStatus_t_CF_TxnStatus_UNDEFINED,
    /// No error.
    NoError = ffi::CF_TxnStatus_t_CF_TxnStatus_NO_ERROR,
    /// Positive acknowledgment limit reached.
    PosAckLimitReached = ffi::CF_TxnStatus_t_CF_TxnStatus_POS_ACK_LIMIT_REACHED,
    /// Keep alive limit reached.
    KeepAliveLimitReached = ffi::CF_TxnStatus_t_CF_TxnStatus_KEEP_ALIVE_LIMIT_REACHED,
    /// Invalid transmission mode.
    InvalidTransmissionMode = ffi::CF_TxnStatus_t_CF_TxnStatus_INVALID_TRANSMISSION_MODE,
    /// Filestore rejection.
    FilestoreRejection = ffi::CF_TxnStatus_t_CF_TxnStatus_FILESTORE_REJECTION,
    /// File checksum failure.
    FileChecksumFailure = ffi::CF_TxnStatus_t_CF_TxnStatus_FILE_CHECKSUM_FAILURE,
    /// File size error.
    FileSizeError = ffi::CF_TxnStatus_t_CF_TxnStatus_FILE_SIZE_ERROR,
    /// NAK limit reached.
    NakLimitReached = ffi::CF_TxnStatus_t_CF_TxnStatus_NAK_LIMIT_REACHED,
    /// Inactivity detected.
    InactivityDetected = ffi::CF_TxnStatus_t_CF_TxnStatus_INACTIVITY_DETECTED,
    /// Invalid file structure.
    InvalidFileStructure = ffi::CF_TxnStatus_t_CF_TxnStatus_INVALID_FILE_STRUCTURE,
    /// Check limit reached.
    CheckLimitReached = ffi::CF_TxnStatus_t_CF_TxnStatus_CHECK_LIMIT_REACHED,
    /// Unsupported checksum type.
    UnsupportedChecksumType = ffi::CF_TxnStatus_t_CF_TxnStatus_UNSUPPORTED_CHECKSUM_TYPE,
    /// Suspend request received.
    SuspendRequestReceived = ffi::CF_TxnStatus_t_CF_TxnStatus_SUSPEND_REQUEST_RECEIVED,
    /// Cancel request received.
    CancelRequestReceived = ffi::CF_TxnStatus_t_CF_TxnStatus_CANCEL_REQUEST_RECEIVED,
    /// Protocol error.
    ProtocolError = ffi::CF_TxnStatus_t_CF_TxnStatus_PROTOCOL_ERROR,
    /// ACK limit reached without FIN.
    AckLimitNoFin = ffi::CF_TxnStatus_t_CF_TxnStatus_ACK_LIMIT_NO_FIN,
    /// ACK limit reached without EOF.
    AckLimitNoEof = ffi::CF_TxnStatus_t_CF_TxnStatus_ACK_LIMIT_NO_EOF,
    /// NAK response error.
    NakResponseError = ffi::CF_TxnStatus_t_CF_TxnStatus_NAK_RESPONSE_ERROR,
    /// Failed to send EOF.
    SendEofFailure = ffi::CF_TxnStatus_t_CF_TxnStatus_SEND_EOF_FAILURE,
    /// Early FIN received.
    EarlyFin = ffi::CF_TxnStatus_t_CF_TxnStatus_EARLY_FIN,
    /// Read failure.
    ReadFailure = ffi::CF_TxnStatus_t_CF_TxnStatus_READ_FAILURE,
    /// No resource available.
    NoResource = ffi::CF_TxnStatus_t_CF_TxnStatus_NO_RESOURCE,
}

impl TryFrom<ffi::CF_TxnStatus_t> for TxnStatus {
    type Error = ();

    fn try_from(val: ffi::CF_TxnStatus_t) -> Result<Self, Self::Error> {
        match val {
            ffi::CF_TxnStatus_t_CF_TxnStatus_UNDEFINED => Ok(Self::Undefined),
            ffi::CF_TxnStatus_t_CF_TxnStatus_NO_ERROR => Ok(Self::NoError),
            ffi::CF_TxnStatus_t_CF_TxnStatus_POS_ACK_LIMIT_REACHED => Ok(Self::PosAckLimitReached),
            ffi::CF_TxnStatus_t_CF_TxnStatus_KEEP_ALIVE_LIMIT_REACHED => {
                Ok(Self::KeepAliveLimitReached)
            }
            ffi::CF_TxnStatus_t_CF_TxnStatus_INVALID_TRANSMISSION_MODE => {
                Ok(Self::InvalidTransmissionMode)
            }
            ffi::CF_TxnStatus_t_CF_TxnStatus_FILESTORE_REJECTION => Ok(Self::FilestoreRejection),
            ffi::CF_TxnStatus_t_CF_TxnStatus_FILE_CHECKSUM_FAILURE => Ok(Self::FileChecksumFailure),
            ffi::CF_TxnStatus_t_CF_TxnStatus_FILE_SIZE_ERROR => Ok(Self::FileSizeError),
            ffi::CF_TxnStatus_t_CF_TxnStatus_NAK_LIMIT_REACHED => Ok(Self::NakLimitReached),
            ffi::CF_TxnStatus_t_CF_TxnStatus_INACTIVITY_DETECTED => Ok(Self::InactivityDetected),
            ffi::CF_TxnStatus_t_CF_TxnStatus_INVALID_FILE_STRUCTURE => {
                Ok(Self::InvalidFileStructure)
            }
            ffi::CF_TxnStatus_t_CF_TxnStatus_CHECK_LIMIT_REACHED => Ok(Self::CheckLimitReached),
            ffi::CF_TxnStatus_t_CF_TxnStatus_UNSUPPORTED_CHECKSUM_TYPE => {
                Ok(Self::UnsupportedChecksumType)
            }
            ffi::CF_TxnStatus_t_CF_TxnStatus_SUSPEND_REQUEST_RECEIVED => {
                Ok(Self::SuspendRequestReceived)
            }
            ffi::CF_TxnStatus_t_CF_TxnStatus_CANCEL_REQUEST_RECEIVED => {
                Ok(Self::CancelRequestReceived)
            }
            ffi::CF_TxnStatus_t_CF_TxnStatus_PROTOCOL_ERROR => Ok(Self::ProtocolError),
            ffi::CF_TxnStatus_t_CF_TxnStatus_ACK_LIMIT_NO_FIN => Ok(Self::AckLimitNoFin),
            ffi::CF_TxnStatus_t_CF_TxnStatus_ACK_LIMIT_NO_EOF => Ok(Self::AckLimitNoEof),
            ffi::CF_TxnStatus_t_CF_TxnStatus_NAK_RESPONSE_ERROR => Ok(Self::NakResponseError),
            ffi::CF_TxnStatus_t_CF_TxnStatus_SEND_EOF_FAILURE => Ok(Self::SendEofFailure),
            ffi::CF_TxnStatus_t_CF_TxnStatus_EARLY_FIN => Ok(Self::EarlyFin),
            ffi::CF_TxnStatus_t_CF_TxnStatus_READ_FAILURE => Ok(Self::ReadFailure),
            ffi::CF_TxnStatus_t_CF_TxnStatus_NO_RESOURCE => Ok(Self::NoResource),
            _ => Err(()),
        }
    }
}

/// Queue index for transaction queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QueueIdx {
    /// Pending transactions.
    Pend = ffi::CF_QueueIdx_t_CF_QueueIdx_PEND,
    /// Active transmit transactions.
    Tx = ffi::CF_QueueIdx_t_CF_QueueIdx_TX,
    /// Active receive transactions.
    Rx = ffi::CF_QueueIdx_t_CF_QueueIdx_RX,
    /// Completed transaction history.
    Hist = ffi::CF_QueueIdx_t_CF_QueueIdx_HIST,
    /// Free history entries.
    HistFree = ffi::CF_QueueIdx_t_CF_QueueIdx_HIST_FREE,
    /// Free transaction entries.
    Free = ffi::CF_QueueIdx_t_CF_QueueIdx_FREE,
}

/// Reset scope for counter reset commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Reset {
    /// Reset all counters.
    All = ffi::CF_Reset_t_CF_Reset_all,
    /// Reset command counters only.
    Command = ffi::CF_Reset_t_CF_Reset_command,
    /// Reset fault counters only.
    Fault = ffi::CF_Reset_t_CF_Reset_fault,
    /// Reset uplink counters only.
    Up = ffi::CF_Reset_t_CF_Reset_up,
    /// Reset downlink counters only.
    Down = ffi::CF_Reset_t_CF_Reset_down,
}

/// Type filter for transaction queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Type {
    /// All transaction types.
    All = ffi::CF_Type_t_CF_Type_all,
    /// Uplink (receive) transactions only.
    Up = ffi::CF_Type_t_CF_Type_up,
    /// Downlink (transmit) transactions only.
    Down = ffi::CF_Type_t_CF_Type_down,
}

/// Queue filter for transaction queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Queue {
    /// Pending queue only.
    Pend = ffi::CF_Queue_t_CF_Queue_pend,
    /// Active queue only.
    Active = ffi::CF_Queue_t_CF_Queue_active,
    /// History queue only.
    History = ffi::CF_Queue_t_CF_Queue_history,
    /// All queues.
    All = ffi::CF_Queue_t_CF_Queue_all,
}

/// Parameter IDs for get/set parameter commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GetSetValueId {
    /// Ticks per second.
    TicksPerSecond = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_ticks_per_second,
    /// RX CRC calculation bytes per wakeup.
    RxCrcCalcBytesPerWakeup = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_rx_crc_calc_bytes_per_wakeup,
    /// ACK timer in seconds.
    AckTimerS = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_ack_timer_s,
    /// NAK timer in seconds.
    NakTimerS = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_nak_timer_s,
    /// Inactivity timer in seconds.
    InactivityTimerS = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_inactivity_timer_s,
    /// Outgoing file chunk size.
    OutgoingFileChunkSize = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_outgoing_file_chunk_size,
    /// ACK limit.
    AckLimit = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_ack_limit,
    /// NAK limit.
    NakLimit = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_nak_limit,
    /// Local entity ID.
    LocalEid = ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_local_eid,
    /// Maximum outgoing messages per wakeup per channel.
    ChanMaxOutgoingMessagesPerWakeup =
        ffi::CF_GetSet_ValueID_t_CF_GetSet_ValueID_chan_max_outgoing_messages_per_wakeup,
}

/// Tick state for tick processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TickState {
    /// Initial state.
    Init = ffi::CF_TickState_t_CF_TickState_INIT,
    /// RX state processing.
    RxState = ffi::CF_TickState_t_CF_TickState_RX_STATE,
    /// TX state processing.
    TxState = ffi::CF_TickState_t_CF_TickState_TX_STATE,
    /// TX NAK processing.
    TxNak = ffi::CF_TickState_t_CF_TickState_TX_NAK,
    /// TX file data processing.
    TxFileData = ffi::CF_TickState_t_CF_TickState_TX_FILEDATA,
    /// TX pending processing.
    TxPend = ffi::CF_TickState_t_CF_TickState_TX_PEND,
    /// Complete state.
    Complete = ffi::CF_TickState_t_CF_TickState_COMPLETE,
}

/// CList traverse status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CListTraverseStatus {
    /// Continue traversing.
    Continue = ffi::CF_CListTraverse_Status_t_CF_CListTraverse_Status_CONTINUE,
    /// Exit traversal.
    Exit = ffi::CF_CListTraverse_Status_t_CF_CListTraverse_Status_EXIT,
}

/// CF command codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CommandCode {
    /// No-op command.
    Noop = ffi::CF_CMDS_CF_NOOP_CC,
    /// Reset counters command.
    Reset = ffi::CF_CMDS_CF_RESET_CC,
    /// Transmit file command.
    TxFile = ffi::CF_CMDS_CF_TX_FILE_CC,
    /// Playback directory command.
    PlaybackDir = ffi::CF_CMDS_CF_PLAYBACK_DIR_CC,
    /// Freeze command.
    Freeze = ffi::CF_CMDS_CF_FREEZE_CC,
    /// Thaw command.
    Thaw = ffi::CF_CMDS_CF_THAW_CC,
    /// Suspend command.
    Suspend = ffi::CF_CMDS_CF_SUSPEND_CC,
    /// Resume command.
    Resume = ffi::CF_CMDS_CF_RESUME_CC,
    /// Cancel command.
    Cancel = ffi::CF_CMDS_CF_CANCEL_CC,
    /// Abandon command.
    Abandon = ffi::CF_CMDS_CF_ABANDON_CC,
    /// Set parameter command.
    SetParam = ffi::CF_CMDS_CF_SET_PARAM_CC,
    /// Get parameter command.
    GetParam = ffi::CF_CMDS_CF_GET_PARAM_CC,
    /// Write queue command.
    WriteQueue = ffi::CF_CMDS_CF_WRITE_QUEUE_CC,
    /// Enable dequeue command.
    EnableDequeue = ffi::CF_CMDS_CF_ENABLE_DEQUEUE_CC,
    /// Disable dequeue command.
    DisableDequeue = ffi::CF_CMDS_CF_DISABLE_DEQUEUE_CC,
    /// Enable directory polling command.
    EnableDirPolling = ffi::CF_CMDS_CF_ENABLE_DIR_POLLING_CC,
    /// Disable directory polling command.
    DisableDirPolling = ffi::CF_CMDS_CF_DISABLE_DIR_POLLING_CC,
    /// Purge queue command.
    PurgeQueue = ffi::CF_CMDS_CF_PURGE_QUEUE_CC,
    /// Enable engine command.
    EnableEngine = ffi::CF_CMDS_CF_ENABLE_ENGINE_CC,
    /// Disable engine command.
    DisableEngine = ffi::CF_CMDS_CF_DISABLE_ENGINE_CC,
}
