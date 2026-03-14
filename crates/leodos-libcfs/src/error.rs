//! Error types for cFS.

use crate::ffi;
use crate::status::check;
use heapless::CString;

/// A specialized `Result` type for CFE operations.
pub type Result<T> = core::result::Result<T, CfsError>;

// ── Sub-error enums ─────────────────────────────────────────

/// CFE Event Services errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EvsError {
    /// An unknown filter scheme was requested.
    #[error("CFE-EVS: Unknown Filter scheme")]
    UnknownFilter,
    /// The application has not been registered with EVS.
    #[error("CFE-EVS: Application not registered")]
    AppNotRegistered,
    /// An illegal application ID was provided.
    #[error("CFE-EVS: Illegal Application ID")]
    AppIllegalAppId,
    /// The application filter has been overloaded.
    #[error("CFE-EVS: Application filter overload")]
    AppFilterOverload,
    /// Failed to access the reset area pointer.
    #[error("CFE-EVS: Reset Area Pointer Failure")]
    ResetAreaPointer,
    /// The event is not registered for filtering.
    #[error("CFE-EVS: Event not registered for filtering")]
    EvtNotRegistered,
    /// A file write error occurred.
    #[error("CFE-EVS: File write error")]
    FileWriteError,
    /// An invalid parameter was supplied in a command.
    #[error("CFE-EVS: Invalid parameter in command")]
    InvalidParameter,
    /// The event was squelched due to a high event rate.
    #[error("CFE-EVS: Event squelched due to high rate")]
    AppSquelched,
    /// The requested function is not implemented.
    #[error("CFE-EVS: Not Implemented")]
    NotImplemented,
}

/// CFE Executive Services errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EsError {
    /// The resource ID is not valid.
    #[error("CFE-ES: Resource ID is not valid")]
    ResourceIdNotValid,
    /// The resource name was not found.
    #[error("CFE-ES: Resource Name not found")]
    NameNotFound,
    /// Failed to create the application.
    #[error("CFE-ES: Application Create Error")]
    AppCreate,
    /// Failed to create a child task.
    #[error("CFE-ES: Child Task Create Error")]
    ChildTaskCreate,
    /// The system log is full.
    #[error("CFE-ES: System Log Full")]
    SysLogFull,
    /// The memory block size is invalid.
    #[error("CFE-ES: Memory Block Size Error")]
    MemBlockSize,
    /// Failed to load the library.
    #[error("CFE-ES: Load Library Error")]
    LoadLib,
    /// A bad argument was provided.
    #[error("CFE-ES: Bad Argument")]
    BadArgument,
    /// Failed to register a child task.
    #[error("CFE-ES: Child Task Register Error")]
    ChildTaskRegister,
    /// Insufficient memory available in CDS.
    #[error("CFE-ES: CDS Insufficient Memory")]
    CdsInsufficientMemory,
    /// The CDS name is invalid.
    #[error("CFE-ES: CDS Invalid Name")]
    CdsInvalidName,
    /// The CDS size is invalid.
    #[error("CFE-ES: CDS Invalid Size")]
    CdsInvalidSize,
    /// The CDS is invalid.
    #[error("CFE-ES: CDS Invalid")]
    CdsInvalid,
    /// Failed to access the CDS.
    #[error("CFE-ES: CDS Access Error")]
    CdsAccessError,
    /// A file I/O error occurred.
    #[error("CFE-ES: File IO Error")]
    FileIoErr,
    /// Failed to access the reset area.
    #[error("CFE-ES: Reset Area Access Error")]
    RstAccessErr,
    /// Failed to register the application.
    #[error("CFE-ES: Application Register Error")]
    AppRegister,
    /// Failed to delete a child task.
    #[error("CFE-ES: Child Task Delete Error")]
    ChildTaskDelete,
    /// Attempted to delete a main task with the child
    /// task delete API.
    #[error("CFE-ES: Attempted to delete a main task")]
    ChildTaskDeleteMainTask,
    /// The CDS block CRC check failed.
    #[error("CFE-ES: CDS Block CRC Error")]
    CdsBlockCrcErr,
    /// Failed to delete a mutex semaphore.
    #[error("CFE-ES: Mutex Semaphore Delete Error")]
    MutSemDeleteErr,
    /// Failed to delete a binary semaphore.
    #[error("CFE-ES: Binary Semaphore Delete Error")]
    BinSemDeleteErr,
    /// Failed to delete a counting semaphore.
    #[error("CFE-ES: Counting Semaphore Delete Error")]
    CountSemDeleteErr,
    /// Failed to delete a queue.
    #[error("CFE-ES: Queue Delete Error")]
    QueueDeleteErr,
    /// Failed to close a file.
    #[error("CFE-ES: File Close Error")]
    FileCloseErr,
    /// The CDS type does not match the expected type.
    #[error("CFE-ES: CDS Wrong Type Error")]
    CdsWrongTypeErr,
    /// The CDS owner is still active.
    #[error("CFE-ES: CDS Owner Active Error")]
    CdsOwnerActiveErr,
    /// Failed to clean up the application.
    #[error("CFE-ES: Application Cleanup Error")]
    AppCleanupErr,
    /// Failed to delete a timer.
    #[error("CFE-ES: Timer Delete Error")]
    TimerDeleteErr,
    /// The buffer is not in the pool.
    #[error("CFE-ES: Buffer Not In Pool")]
    BufferNotInPool,
    /// Failed to delete a task.
    #[error("CFE-ES: Task Delete Error")]
    TaskDeleteErr,
    /// The operation timed out.
    #[error("CFE-ES: Operation Timed Out")]
    OperationTimedOut,
    /// No resource IDs are available.
    #[error("CFE-ES: No Resource IDs Available")]
    NoResourceIdsAvailable,
    /// The pool block is invalid.
    #[error("CFE-ES: Invalid pool block")]
    PoolBlockInvalid,
    /// A resource with that name already exists.
    #[error("CFE-ES: Duplicate Name Error")]
    DuplicateName,
    /// The requested function is not implemented.
    #[error("CFE-ES: Not Implemented")]
    NotImplemented,
}

/// CFE Software Bus errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SbError {
    /// The receive operation timed out.
    #[error("CFE-SB: Time Out")]
    TimeOut,
    /// No message is available on the pipe.
    #[error("CFE-SB: No Message")]
    NoMessage,
    /// A bad argument was provided.
    #[error("CFE-SB: Bad Argument")]
    BadArgument,
    /// The maximum number of pipes has been reached.
    #[error("CFE-SB: Max Pipes Met")]
    MaxPipesMet,
    /// Failed to create a pipe.
    #[error("CFE-SB: Pipe Create Error")]
    PipeCrErr,
    /// Failed to read from a pipe.
    #[error("CFE-SB: Pipe Read Error")]
    PipeRdErr,
    /// The message exceeds the maximum allowed size.
    #[error("CFE-SB: Message Too Big")]
    MsgTooBig,
    /// The SB message buffer pool has been depleted.
    #[error("CFE-SB: Buffer Allocation Error")]
    BufAllocErr,
    /// The maximum number of messages has been reached.
    #[error("CFE-SB: Max Messages Met")]
    MaxMsgsMet,
    /// The maximum number of destinations has been reached.
    #[error("CFE-SB: Max Destinations Met")]
    MaxDestsMet,
    /// An internal SB error occurred.
    #[error("CFE-SB: CFE-Internal Error")]
    InternalErr,
    /// The message type is incorrect for the operation.
    #[error("CFE-SB: Wrong Message Type")]
    WrongMsgType,
    /// The buffer reference is invalid.
    #[error("CFE-SB: Buffer Invalid")]
    BufferInvalid,
    /// The requested function is not implemented.
    #[error("CFE-SB: Not Implemented")]
    NotImplemented,
}

/// CFE File Services errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FsError {
    /// A bad argument was provided.
    #[error("CFE-FS: Bad Argument")]
    BadArgument,
    /// The file path is invalid.
    #[error("CFE-FS: Invalid Path")]
    InvalidPath,
    /// The filename exceeds the maximum allowed length.
    #[error("CFE-FS: Filename Too Long")]
    FnameTooLong,
    /// The requested function is not implemented.
    #[error("CFE-FS: Not Implemented")]
    NotImplemented,
}

/// CFE Table Services errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TblError {
    /// The table handle is invalid.
    #[error("CFE-TBL: Invalid Handle")]
    InvalidHandle,
    /// The table name is invalid.
    #[error("CFE-TBL: Invalid Name")]
    InvalidName,
    /// The table size is invalid.
    #[error("CFE-TBL: Invalid Size")]
    InvalidSize,
    /// The table has never been loaded.
    #[error("CFE-TBL: Never Loaded")]
    NeverLoaded,
    /// The table registry is full.
    #[error("CFE-TBL: Registry Full")]
    RegistryFull,
    /// Access to the table was denied.
    #[error("CFE-TBL: No Access")]
    NoAccess,
    /// The table is not registered.
    #[error("CFE-TBL: Unregistered")]
    Unregistered,
    /// All available table handles are in use.
    #[error("CFE-TBL: Handles Full")]
    HandlesFull,
    /// A duplicate table with a different size was found.
    #[error("CFE-TBL: Duplicate Table With Different Size")]
    DuplicateDiffSize,
    /// A duplicate table was found but is not owned by the
    /// calling application.
    #[error("CFE-TBL: Duplicate Table And Not Owned")]
    DuplicateNotOwned,
    /// No working buffer is available.
    #[error("CFE-TBL: No Buffer Available")]
    NoBufferAvail,
    /// The table is dump-only; load is not permitted.
    #[error("CFE-TBL: Dump Only Error")]
    DumpOnly,
    /// The source type is illegal for this operation.
    #[error("CFE-TBL: Illegal Source Type")]
    IllegalSrcType,
    /// A table load is already in progress.
    #[error("CFE-TBL: Load In Progress")]
    LoadInProgress,
    /// The file is too large for the table.
    #[error("CFE-TBL: File Too Large")]
    FileTooLarge,
    /// The content ID in the file header is invalid.
    #[error("CFE-TBL: Bad Content ID")]
    BadContentId,
    /// The subtype ID in the file header is invalid.
    #[error("CFE-TBL: Bad Subtype ID")]
    BadSubtypeId,
    /// The file size is inconsistent with the table size.
    #[error("CFE-TBL: File Size Inconsistent")]
    FileSizeInconsistent,
    /// The file is missing a standard header.
    #[error("CFE-TBL: No Standard Header")]
    NoStdHeader,
    /// The file is missing a table header.
    #[error("CFE-TBL: No Table Header")]
    NoTblHeader,
    /// The filename exceeds the maximum allowed length.
    #[error("CFE-TBL: Filename Too Long")]
    FilenameTooLong,
    /// The file is intended for a different table.
    #[error("CFE-TBL: File For Wrong Table")]
    FileForWrongTable,
    /// The table load did not complete.
    #[error("CFE-TBL: Load Incomplete")]
    LoadIncomplete,
    /// Only a partial load was performed.
    #[error("CFE-TBL: Partial Load Error")]
    PartialLoad,
    /// The table options are invalid.
    #[error("CFE-TBL: Invalid Options")]
    InvalidOptions,
    /// The spacecraft ID in the file header does not match.
    #[error("CFE-TBL: Bad Spacecraft ID")]
    BadSpacecraftId,
    /// The processor ID in the file header does not match.
    #[error("CFE-TBL: Bad Processor ID")]
    BadProcessorId,
    /// A message error occurred during table operations.
    #[error("CFE-TBL: Message Error")]
    MessageError,
    /// The file is shorter than expected.
    #[error("CFE-TBL: Short File")]
    ShortFile,
    /// A table access error occurred.
    #[error("CFE-TBL: Access error")]
    Access,
    /// A bad argument was provided.
    #[error("CFE-TBL: Bad Argument")]
    BadArgument,
    /// The requested function is not implemented.
    #[error("CFE-TBL: Not Implemented")]
    NotImplemented,
}

/// CFE Time Services errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TimeError {
    /// The requested function is not implemented.
    #[error("CFE-TIME: Not Implemented")]
    NotImplemented,
    /// The time function is for internal use only.
    #[error("CFE-TIME: Internal Only")]
    InternalOnly,
    /// The time value is out of the valid range.
    #[error("CFE-TIME: Out Of Range")]
    OutOfRange,
    /// Too many synchronization callbacks have been
    /// registered.
    #[error("CFE-TIME: Too Many Sync Callbacks")]
    TooManySynchCallbacks,
    /// The callback was not previously registered.
    #[error("CFE-TIME: Callback Not Registered")]
    CallbackNotRegistered,
    /// A bad argument was provided.
    #[error("CFE-TIME: Bad Argument")]
    BadArgument,
}

/// OSAL errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum OsalError {
    /// A generic OSAL error occurred.
    #[error("OSAL: Generic error")]
    Error,
    /// An invalid pointer was provided.
    #[error("OSAL: Invalid pointer")]
    InvalidPointer,
    /// The address is not properly aligned.
    #[error("OSAL: Address misalignment")]
    AddressMisaligned,
    /// The operation timed out.
    #[error("OSAL: Timeout")]
    Timeout,
    /// The interrupt number is invalid.
    #[error("OSAL: Invalid Interrupt number")]
    InvalidIntNum,
    /// A semaphore operation failed.
    #[error("OSAL: Semaphore failure")]
    SemFailure,
    /// A semaphore operation timed out.
    #[error("OSAL: Semaphore timeout")]
    SemTimeout,
    /// The queue is empty.
    #[error("OSAL: Queue empty")]
    QueueEmpty,
    /// The queue is full.
    #[error("OSAL: Queue full")]
    QueueFull,
    /// A queue operation timed out.
    #[error("OSAL: Queue timeout")]
    QueueTimeout,
    /// The queue size is invalid.
    #[error("OSAL: Queue invalid size")]
    QueueInvalidSize,
    /// The queue ID is invalid.
    #[error("OSAL: Queue ID error")]
    QueueIdError,
    /// The name exceeds the maximum allowed length.
    #[error("OSAL: Name length too long")]
    NameTooLong,
    /// No free IDs are available.
    #[error("OSAL: No free IDs")]
    NoFreeIds,
    /// The requested name is already taken.
    #[error("OSAL: Name taken")]
    NameTaken,
    /// The object ID is invalid.
    #[error("OSAL: Invalid ID")]
    InvalidId,
    /// The name was not found.
    #[error("OSAL: Name not found")]
    NameNotFound,
    /// The semaphore is not full.
    #[error("OSAL: Semaphore not full")]
    SemNotFull,
    /// The priority value is invalid.
    #[error("OSAL: Invalid priority")]
    InvalidPriority,
    /// The semaphore value is invalid.
    #[error("OSAL: Invalid semaphore value")]
    InvalidSemValue,
    /// A file operation error occurred.
    #[error("OSAL: File error")]
    File,
    /// The requested function is not implemented.
    #[error("OSAL: Not implemented")]
    NotImplemented,
    /// Invalid arguments were passed to a timer function.
    #[error("OSAL: Timer invalid arguments")]
    TimerInvalidArgs,
    /// The timer ID is invalid.
    #[error("OSAL: Timer ID error")]
    TimerIdError,
    /// The timer is unavailable.
    #[error("OSAL: Timer unavailable")]
    TimerUnavailable,
    /// An internal timer error occurred.
    #[error("OSAL: Timer internal error")]
    TimerInternal,
    /// The object is currently in use.
    #[error("OSAL: Object in use")]
    ObjectInUse,
    /// The address is invalid.
    #[error("OSAL: Bad address")]
    BadAddress,
    /// The object is in an incorrect state for the
    /// requested operation.
    #[error("OSAL: Incorrect object state")]
    IncorrectObjState,
    /// The object type is incorrect for the requested
    /// operation.
    #[error("OSAL: Incorrect object type")]
    IncorrectObjType,
    /// The stream has been disconnected.
    #[error("OSAL: Stream disconnected")]
    StreamDisconnected,
    /// The requested operation is not supported on the
    /// supplied objects.
    #[error("OSAL: Requested operation not supported on supplied object(s)")]
    OperationNotSupported,
    /// The size is invalid.
    #[error("OSAL: Invalid Size")]
    InvalidSize,
    /// The output size exceeds the limit.
    #[error("OSAL: Size of output exceeds limit")]
    OutputTooLarge,
    /// The argument value is invalid.
    #[error("OSAL: Invalid argument value")]
    InvalidArgument,
    /// The filesystem path exceeds the maximum length.
    #[error("OSAL: FS path too long")]
    FsPathTooLong,
    /// The filesystem name exceeds the maximum length.
    #[error("OSAL: FS name too long")]
    FsNameTooLong,
    /// The filesystem drive was not created.
    #[error("OSAL: FS drive not created")]
    FsDriveNotCreated,
    /// The filesystem device is not free.
    #[error("OSAL: FS device not free")]
    FsDeviceNotFree,
    /// The filesystem path is invalid.
    #[error("OSAL: FS path invalid")]
    FsPathInvalid,
}

// ── Top-level Error ─────────────────────────────────────────

/// Represents all possible errors and status codes from the
/// CFE and OSAL APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CfsError {
    /// An EVS (Event Services) error.
    #[error(transparent)]
    Evs(#[from] EvsError),
    /// An ES (Executive Services) error.
    #[error(transparent)]
    Es(#[from] EsError),
    /// An SB (Software Bus) error.
    #[error(transparent)]
    Sb(#[from] SbError),
    /// An FS (File Services) error.
    #[error(transparent)]
    Fs(#[from] FsError),
    /// A TBL (Table Services) error.
    #[error(transparent)]
    Tbl(#[from] TblError),
    /// A TIME (Time Services) error.
    #[error(transparent)]
    Time(#[from] TimeError),
    /// An OSAL error.
    #[error(transparent)]
    Osal(#[from] OsalError),

    // --- Generic CFE Status ---
    /// The message length is incorrect.
    #[error("CFE: Wrong Message Length")]
    WrongMsgLength,
    /// The message ID is unknown.
    #[error("CFE: Unknown Message ID")]
    UnknownMsgId,
    /// The command code is invalid.
    #[error("CFE: Bad Command Code")]
    BadCommandCode,
    /// An external resource failure occurred.
    #[error("CFE: External resource failure")]
    ExternalResourceFail,
    /// The request is already pending.
    #[error("CFE: Request already pending")]
    RequestAlreadyPending,
    /// Validation of the input failed.
    #[error("CFE: Validation Failure")]
    ValidationFailure,
    /// The input value is out of the valid range.
    #[error("CFE: Input value out of range")]
    RangeError,
    /// The request cannot be processed in the current state.
    #[error("CFE: Cannot process request in current state")]
    IncorrectState,
    /// The requested function is not implemented.
    #[error("CFE: Not Implemented")]
    NotImplemented,

    // --- Other Errors ---
    /// The string contains an interior null character.
    #[error("Invalid string: contains interior null character")]
    InvalidString,
    /// The task pool is full and cannot accept new tasks.
    #[error("The task pool is full")]
    TaskPoolFull,
    /// The task's stack size exceeds the maximum allowed.
    #[error("The task's stack size exceeds the maximum allowed size")]
    TaskTooLarge,
    /// The task's alignment exceeds the maximum allowed.
    #[error("The task's alignment requirement exceeds the maximum allowed alignment")]
    TaskAlignmentTooLarge,
    /// A type conversion error occurred.
    #[error("Type conversion error")]
    ConversionError(&'static str),
    /// An unhandled error code was returned by the C API.
    #[error("Unhandled error code: {0}")]
    Unhandled(i32),
}

// ── From<CFE_Status_t> ─────────────────────────────────────

impl From<ffi::CFE_Status_t> for CfsError {
    fn from(status: ffi::CFE_Status_t) -> Self {
        match status {
            // --- Generic CFE Status ---
            ffi::CFE_STATUS_WRONG_MSG_LENGTH => CfsError::WrongMsgLength,
            ffi::CFE_STATUS_UNKNOWN_MSG_ID => CfsError::UnknownMsgId,
            ffi::CFE_STATUS_BAD_COMMAND_CODE => CfsError::BadCommandCode,
            ffi::CFE_STATUS_EXTERNAL_RESOURCE_FAIL => CfsError::ExternalResourceFail,
            ffi::CFE_STATUS_REQUEST_ALREADY_PENDING => CfsError::RequestAlreadyPending,
            ffi::CFE_STATUS_VALIDATION_FAILURE => CfsError::ValidationFailure,
            ffi::CFE_STATUS_RANGE_ERROR => CfsError::RangeError,
            ffi::CFE_STATUS_INCORRECT_STATE => CfsError::IncorrectState,
            ffi::CFE_STATUS_NOT_IMPLEMENTED => CfsError::NotImplemented,

            // --- CFE EVS (Event Services) ---
            ffi::CFE_EVS_UNKNOWN_FILTER => EvsError::UnknownFilter.into(),
            ffi::CFE_EVS_APP_NOT_REGISTERED => EvsError::AppNotRegistered.into(),
            ffi::CFE_EVS_APP_ILLEGAL_APP_ID => EvsError::AppIllegalAppId.into(),
            ffi::CFE_EVS_APP_FILTER_OVERLOAD => EvsError::AppFilterOverload.into(),
            ffi::CFE_EVS_RESET_AREA_POINTER => EvsError::ResetAreaPointer.into(),
            ffi::CFE_EVS_EVT_NOT_REGISTERED => EvsError::EvtNotRegistered.into(),
            ffi::CFE_EVS_FILE_WRITE_ERROR => EvsError::FileWriteError.into(),
            ffi::CFE_EVS_INVALID_PARAMETER => EvsError::InvalidParameter.into(),
            ffi::CFE_EVS_APP_SQUELCHED => EvsError::AppSquelched.into(),
            ffi::CFE_EVS_NOT_IMPLEMENTED => EvsError::NotImplemented.into(),

            // --- CFE ES (Executive Services) ---
            ffi::CFE_ES_ERR_RESOURCEID_NOT_VALID => EsError::ResourceIdNotValid.into(),
            ffi::CFE_ES_ERR_NAME_NOT_FOUND => EsError::NameNotFound.into(),
            ffi::CFE_ES_ERR_APP_CREATE => EsError::AppCreate.into(),
            ffi::CFE_ES_ERR_CHILD_TASK_CREATE => EsError::ChildTaskCreate.into(),
            ffi::CFE_ES_ERR_SYS_LOG_FULL => EsError::SysLogFull.into(),
            ffi::CFE_ES_ERR_MEM_BLOCK_SIZE => EsError::MemBlockSize.into(),
            ffi::CFE_ES_ERR_LOAD_LIB => EsError::LoadLib.into(),
            ffi::CFE_ES_BAD_ARGUMENT => EsError::BadArgument.into(),
            ffi::CFE_ES_ERR_CHILD_TASK_REGISTER => EsError::ChildTaskRegister.into(),
            ffi::CFE_ES_CDS_INSUFFICIENT_MEMORY => EsError::CdsInsufficientMemory.into(),
            ffi::CFE_ES_CDS_INVALID_NAME => EsError::CdsInvalidName.into(),
            ffi::CFE_ES_CDS_INVALID_SIZE => EsError::CdsInvalidSize.into(),
            ffi::CFE_ES_CDS_INVALID => EsError::CdsInvalid.into(),
            ffi::CFE_ES_CDS_ACCESS_ERROR => EsError::CdsAccessError.into(),
            ffi::CFE_ES_FILE_IO_ERR => EsError::FileIoErr.into(),
            ffi::CFE_ES_RST_ACCESS_ERR => EsError::RstAccessErr.into(),
            ffi::CFE_ES_ERR_APP_REGISTER => EsError::AppRegister.into(),
            ffi::CFE_ES_ERR_CHILD_TASK_DELETE => EsError::ChildTaskDelete.into(),
            ffi::CFE_ES_ERR_CHILD_TASK_DELETE_MAIN_TASK => EsError::ChildTaskDeleteMainTask.into(),
            ffi::CFE_ES_CDS_BLOCK_CRC_ERR => EsError::CdsBlockCrcErr.into(),
            ffi::CFE_ES_MUT_SEM_DELETE_ERR => EsError::MutSemDeleteErr.into(),
            ffi::CFE_ES_BIN_SEM_DELETE_ERR => EsError::BinSemDeleteErr.into(),
            ffi::CFE_ES_COUNT_SEM_DELETE_ERR => EsError::CountSemDeleteErr.into(),
            ffi::CFE_ES_QUEUE_DELETE_ERR => EsError::QueueDeleteErr.into(),
            ffi::CFE_ES_FILE_CLOSE_ERR => EsError::FileCloseErr.into(),
            ffi::CFE_ES_CDS_WRONG_TYPE_ERR => EsError::CdsWrongTypeErr.into(),
            ffi::CFE_ES_CDS_OWNER_ACTIVE_ERR => EsError::CdsOwnerActiveErr.into(),
            ffi::CFE_ES_APP_CLEANUP_ERR => EsError::AppCleanupErr.into(),
            ffi::CFE_ES_TIMER_DELETE_ERR => EsError::TimerDeleteErr.into(),
            ffi::CFE_ES_BUFFER_NOT_IN_POOL => EsError::BufferNotInPool.into(),
            ffi::CFE_ES_TASK_DELETE_ERR => EsError::TaskDeleteErr.into(),
            ffi::CFE_ES_OPERATION_TIMED_OUT => EsError::OperationTimedOut.into(),
            ffi::CFE_ES_NO_RESOURCE_IDS_AVAILABLE => EsError::NoResourceIdsAvailable.into(),
            ffi::CFE_ES_POOL_BLOCK_INVALID => EsError::PoolBlockInvalid.into(),
            ffi::CFE_ES_ERR_DUPLICATE_NAME => EsError::DuplicateName.into(),
            ffi::CFE_ES_NOT_IMPLEMENTED => EsError::NotImplemented.into(),

            // --- CFE FS (File Services) ---
            ffi::CFE_FS_BAD_ARGUMENT => FsError::BadArgument.into(),
            ffi::CFE_FS_INVALID_PATH => FsError::InvalidPath.into(),
            ffi::CFE_FS_FNAME_TOO_LONG => FsError::FnameTooLong.into(),
            ffi::CFE_FS_NOT_IMPLEMENTED => FsError::NotImplemented.into(),

            // --- CFE SB (Software Bus) ---
            ffi::CFE_SB_TIME_OUT => SbError::TimeOut.into(),
            ffi::CFE_SB_NO_MESSAGE => SbError::NoMessage.into(),
            ffi::CFE_SB_BAD_ARGUMENT => SbError::BadArgument.into(),
            ffi::CFE_SB_MAX_PIPES_MET => SbError::MaxPipesMet.into(),
            ffi::CFE_SB_PIPE_CR_ERR => SbError::PipeCrErr.into(),
            ffi::CFE_SB_PIPE_RD_ERR => SbError::PipeRdErr.into(),
            ffi::CFE_SB_MSG_TOO_BIG => SbError::MsgTooBig.into(),
            ffi::CFE_SB_BUF_ALOC_ERR => SbError::BufAllocErr.into(),
            ffi::CFE_SB_MAX_MSGS_MET => SbError::MaxMsgsMet.into(),
            ffi::CFE_SB_MAX_DESTS_MET => SbError::MaxDestsMet.into(),
            ffi::CFE_SB_INTERNAL_ERR => SbError::InternalErr.into(),
            ffi::CFE_SB_WRONG_MSG_TYPE => SbError::WrongMsgType.into(),
            ffi::CFE_SB_BUFFER_INVALID => SbError::BufferInvalid.into(),
            ffi::CFE_SB_NOT_IMPLEMENTED => SbError::NotImplemented.into(),

            // --- CFE TBL (Table Services) ---
            ffi::CFE_TBL_ERR_INVALID_HANDLE => TblError::InvalidHandle.into(),
            ffi::CFE_TBL_ERR_INVALID_NAME => TblError::InvalidName.into(),
            ffi::CFE_TBL_ERR_INVALID_SIZE => TblError::InvalidSize.into(),
            ffi::CFE_TBL_ERR_NEVER_LOADED => TblError::NeverLoaded.into(),
            ffi::CFE_TBL_ERR_REGISTRY_FULL => TblError::RegistryFull.into(),
            ffi::CFE_TBL_ERR_NO_ACCESS => TblError::NoAccess.into(),
            ffi::CFE_TBL_ERR_UNREGISTERED => TblError::Unregistered.into(),
            ffi::CFE_TBL_ERR_HANDLES_FULL => TblError::HandlesFull.into(),
            ffi::CFE_TBL_ERR_DUPLICATE_DIFF_SIZE => TblError::DuplicateDiffSize.into(),
            ffi::CFE_TBL_ERR_DUPLICATE_NOT_OWNED => TblError::DuplicateNotOwned.into(),
            ffi::CFE_TBL_ERR_NO_BUFFER_AVAIL => TblError::NoBufferAvail.into(),
            ffi::CFE_TBL_ERR_DUMP_ONLY => TblError::DumpOnly.into(),
            ffi::CFE_TBL_ERR_ILLEGAL_SRC_TYPE => TblError::IllegalSrcType.into(),
            ffi::CFE_TBL_ERR_LOAD_IN_PROGRESS => TblError::LoadInProgress.into(),
            ffi::CFE_TBL_ERR_FILE_TOO_LARGE => TblError::FileTooLarge.into(),
            ffi::CFE_TBL_ERR_BAD_CONTENT_ID => TblError::BadContentId.into(),
            ffi::CFE_TBL_ERR_BAD_SUBTYPE_ID => TblError::BadSubtypeId.into(),
            ffi::CFE_TBL_ERR_FILE_SIZE_INCONSISTENT => TblError::FileSizeInconsistent.into(),
            ffi::CFE_TBL_ERR_NO_STD_HEADER => TblError::NoStdHeader.into(),
            ffi::CFE_TBL_ERR_NO_TBL_HEADER => TblError::NoTblHeader.into(),
            ffi::CFE_TBL_ERR_FILENAME_TOO_LONG => TblError::FilenameTooLong.into(),
            ffi::CFE_TBL_ERR_FILE_FOR_WRONG_TABLE => TblError::FileForWrongTable.into(),
            ffi::CFE_TBL_ERR_LOAD_INCOMPLETE => TblError::LoadIncomplete.into(),
            ffi::CFE_TBL_ERR_PARTIAL_LOAD => TblError::PartialLoad.into(),
            ffi::CFE_TBL_ERR_INVALID_OPTIONS => TblError::InvalidOptions.into(),
            ffi::CFE_TBL_ERR_BAD_SPACECRAFT_ID => TblError::BadSpacecraftId.into(),
            ffi::CFE_TBL_ERR_BAD_PROCESSOR_ID => TblError::BadProcessorId.into(),
            ffi::CFE_TBL_MESSAGE_ERROR => TblError::MessageError.into(),
            ffi::CFE_TBL_ERR_SHORT_FILE => TblError::ShortFile.into(),
            ffi::CFE_TBL_ERR_ACCESS => TblError::Access.into(),
            ffi::CFE_TBL_BAD_ARGUMENT => TblError::BadArgument.into(),
            ffi::CFE_TBL_NOT_IMPLEMENTED => TblError::NotImplemented.into(),

            // --- CFE TIME (Time Services) ---
            ffi::CFE_TIME_NOT_IMPLEMENTED => TimeError::NotImplemented.into(),
            ffi::CFE_TIME_INTERNAL_ONLY => TimeError::InternalOnly.into(),
            ffi::CFE_TIME_OUT_OF_RANGE => TimeError::OutOfRange.into(),
            ffi::CFE_TIME_TOO_MANY_SYNCH_CALLBACKS => TimeError::TooManySynchCallbacks.into(),
            ffi::CFE_TIME_CALLBACK_NOT_REGISTERED => TimeError::CallbackNotRegistered.into(),
            ffi::CFE_TIME_BAD_ARGUMENT => TimeError::BadArgument.into(),

            // --- OSAL Status Codes ---
            ffi::OS_ERROR => OsalError::Error.into(),
            ffi::OS_INVALID_POINTER => OsalError::InvalidPointer.into(),
            ffi::OS_ERROR_ADDRESS_MISALIGNED => OsalError::AddressMisaligned.into(),
            ffi::OS_ERROR_TIMEOUT => OsalError::Timeout.into(),
            ffi::OS_INVALID_INT_NUM => OsalError::InvalidIntNum.into(),
            ffi::OS_SEM_FAILURE => OsalError::SemFailure.into(),
            ffi::OS_SEM_TIMEOUT => OsalError::SemTimeout.into(),
            ffi::OS_QUEUE_EMPTY => OsalError::QueueEmpty.into(),
            ffi::OS_QUEUE_FULL => OsalError::QueueFull.into(),
            ffi::OS_QUEUE_TIMEOUT => OsalError::QueueTimeout.into(),
            ffi::OS_QUEUE_INVALID_SIZE => OsalError::QueueInvalidSize.into(),
            ffi::OS_QUEUE_ID_ERROR => OsalError::QueueIdError.into(),
            ffi::OS_ERR_NAME_TOO_LONG => OsalError::NameTooLong.into(),
            ffi::OS_ERR_NO_FREE_IDS => OsalError::NoFreeIds.into(),
            ffi::OS_ERR_NAME_TAKEN => OsalError::NameTaken.into(),
            ffi::OS_ERR_INVALID_ID => OsalError::InvalidId.into(),
            ffi::OS_ERR_NAME_NOT_FOUND => OsalError::NameNotFound.into(),
            ffi::OS_ERR_SEM_NOT_FULL => OsalError::SemNotFull.into(),
            ffi::OS_ERR_INVALID_PRIORITY => OsalError::InvalidPriority.into(),
            ffi::OS_INVALID_SEM_VALUE => OsalError::InvalidSemValue.into(),
            ffi::OS_ERR_FILE => OsalError::File.into(),
            ffi::OS_ERR_NOT_IMPLEMENTED => OsalError::NotImplemented.into(),
            ffi::OS_TIMER_ERR_INVALID_ARGS => OsalError::TimerInvalidArgs.into(),
            ffi::OS_TIMER_ERR_TIMER_ID => OsalError::TimerIdError.into(),
            ffi::OS_TIMER_ERR_UNAVAILABLE => OsalError::TimerUnavailable.into(),
            ffi::OS_TIMER_ERR_INTERNAL => OsalError::TimerInternal.into(),
            ffi::OS_ERR_OBJECT_IN_USE => OsalError::ObjectInUse.into(),
            ffi::OS_ERR_BAD_ADDRESS => OsalError::BadAddress.into(),
            ffi::OS_ERR_INCORRECT_OBJ_STATE => OsalError::IncorrectObjState.into(),
            ffi::OS_ERR_INCORRECT_OBJ_TYPE => OsalError::IncorrectObjType.into(),
            ffi::OS_ERR_STREAM_DISCONNECTED => OsalError::StreamDisconnected.into(),
            ffi::OS_ERR_OPERATION_NOT_SUPPORTED => OsalError::OperationNotSupported.into(),
            ffi::OS_ERR_INVALID_SIZE => OsalError::InvalidSize.into(),
            ffi::OS_ERR_OUTPUT_TOO_LARGE => OsalError::OutputTooLarge.into(),
            ffi::OS_ERR_INVALID_ARGUMENT => OsalError::InvalidArgument.into(),
            ffi::OS_FS_ERR_PATH_TOO_LONG => OsalError::FsPathTooLong.into(),
            ffi::OS_FS_ERR_NAME_TOO_LONG => OsalError::FsNameTooLong.into(),
            ffi::OS_FS_ERR_DRIVE_NOT_CREATED => OsalError::FsDriveNotCreated.into(),
            ffi::OS_FS_ERR_DEVICE_NOT_FREE => OsalError::FsDeviceNotFree.into(),
            ffi::OS_FS_ERR_PATH_INVALID => OsalError::FsPathInvalid.into(),

            other => CfsError::Unhandled(other),
        }
    }
}

// ── Helper functions ────────────────────────────────────────

impl CfsError {
    /// Retrieves the symbolic name for an OSAL status code.
    pub fn name(error: i32) -> Result<CString<{ ffi::OS_ERROR_NAME_LENGTH as usize }>> {
        const SIZE: usize = ffi::OS_ERROR_NAME_LENGTH as usize;
        let mut name_buf = [0u8; SIZE];
        check(unsafe {
            ffi::OS_GetErrorName(error, &mut name_buf as *mut _ as *mut [libc::c_char; SIZE])
        })?;

        let mut s = CString::new();
        s.extend_from_bytes(&name_buf)
            .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        Ok(s)
    }
}

/// Retrieves the symbolic name for a CFE status code.
pub fn get_cfe_status_name(
    status: i32,
) -> Result<CString<{ ffi::CFE_STATUS_STRING_LENGTH as usize }>> {
    const SIZE: usize = ffi::CFE_STATUS_STRING_LENGTH as usize;
    let mut name_buf = [0u8; SIZE];
    unsafe {
        ffi::CFE_ES_StatusToString(status, &mut name_buf as *mut _ as *mut [libc::c_char; SIZE])
    };

    let mut s = CString::new();
    s.extend_from_bytes(&name_buf)
        .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
    Ok(s)
}

/// Converts an OSAL status code to its decimal or hex string
/// representation.
pub fn osal_status_to_string(
    status: i32,
) -> Result<CString<{ ffi::OS_STATUS_STRING_LENGTH as usize }>> {
    const SIZE: usize = ffi::OS_STATUS_STRING_LENGTH as usize;
    let mut name_buf = [0u8; SIZE];
    unsafe { ffi::OS_StatusToString(status, &mut name_buf as *mut _ as *mut [libc::c_char; SIZE]) };

    let mut s = CString::new();
    s.extend_from_bytes(&name_buf)
        .map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
    Ok(s)
}
