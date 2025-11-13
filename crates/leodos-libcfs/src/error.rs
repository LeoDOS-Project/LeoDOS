//! Error types for cFS.

use crate::ffi;
use crate::status::check;
use heapless::CString;

/// A specialized `Result` type for CFE operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Represents all possible errors and status codes from the CFE and OSAL APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    // --- Generic CFE Status ---
    /// Wrong Message Length.
    #[error("CFE: Wrong Message Length")]
    CfeStatusWrongMsgLength,
    /// Unknown Message ID.
    #[error("CFE: Unknown Message ID")]
    CfeStatusUnknownMsgId,
    /// Bad Command Code.
    #[error("CFE: Bad Command Code")]
    CfeStatusBadCommandCode,
    /// External resource failure.
    #[error("CFE: External resource failure")]
    CfeStatusExternalResourceFail,
    /// Request already pending.
    #[error("CFE: Request already pending")]
    CfeStatusRequestAlreadyPending,
    /// Request or input value failed basic structural validation.
    #[error("CFE: Validation Failure")]
    CfeStatusValidationFailure,
    /// Request or input value is out of range.
    #[error("CFE: Input value out of range")]
    CfeStatusRangeError,
    /// Cannot process request at this time.
    #[error("CFE: Cannot process request in current state")]
    CfeStatusIncorrectState,
    /// Not Implemented.
    #[error("CFE: Not Implemented")]
    CfeStatusNotImplemented,

    // --- CFE EVS (Event Services) ---
    /// Unknown Filter scheme.
    #[error("CFE-EVS: Unknown Filter scheme")]
    CfeEvsUnknownFilter,
    /// Application not registered with EVS.
    #[error("CFE-EVS: Application not registered")]
    CfeEvsAppNotRegistered,
    /// Illegal Application ID.
    #[error("CFE-EVS: Illegal Application ID")]
    CfeEvsAppIllegalAppId,
    /// Application event filter overload.
    #[error("CFE-EVS: Application filter overload")]
    CfeEvsAppFilterOverload,
    /// Could not get pointer to the ES Reset area.
    #[error("CFE-EVS: Reset Area Pointer Failure")]
    CfeEvsResetAreaPointer,
    /// EventID argument was not found in any registered event filter.
    #[error("CFE-EVS: Event not registered for filtering")]
    CfeEvsEvtNotRegistered,
    /// A file write error occurred while processing an EVS command.
    #[error("CFE-EVS: File write error")]
    CfeEvsFileWriteError,
    /// Invalid parameter supplied to EVS command.
    #[error("CFE-EVS: Invalid parameter in command")]
    CfeEvsInvalidParameter,
    /// Event squelched due to being sent at too high a rate.
    #[error("CFE-EVS: Event squelched due to high rate")]
    CfeEvsAppSquelched,
    /// EVS feature is not implemented.
    #[error("CFE-EVS: Not Implemented")]
    CfeEvsNotImplemented,

    // --- CFE ES (Executive Services) ---
    /// Resource ID is not valid.
    #[error("CFE-ES: Resource ID is not valid")]
    CfeEsErrResourceIdNotValid,
    /// Resource Name not found.
    #[error("CFE-ES: Resource Name not found")]
    CfeEsErrNameNotFound,
    /// Error loading or creating an App.
    #[error("CFE-ES: Application Create Error")]
    CfeEsErrAppCreate,
    /// Error creating a child task.
    #[error("CFE-ES: Child Task Create Error")]
    CfeEsErrChildTaskCreate,
    /// The cFE system Log is full.
    #[error("CFE-ES: System Log Full")]
    CfeEsErrSysLogFull,
    /// The memory block size requested is invalid.
    #[error("CFE-ES: Memory Block Size Error")]
    CfeEsErrMemBlockSize,
    /// Could not load the shared library.
    #[error("CFE-ES: Load Library Error")]
    CfeEsErrLoadLib,
    /// Bad parameter passed into an ES API.
    #[error("CFE-ES: Bad Argument")]
    CfeEsBadArgument,
    /// Errors occurred when trying to register a child task.
    #[error("CFE-ES: Child Task Register Error")]
    CfeEsErrChildTaskRegister,
    /// CDS is larger than the remaining CDS memory.
    #[error("CFE-ES: CDS Insufficient Memory")]
    CfeEsCdsInsufficientMemory,
    /// CDS name is too long or empty.
    #[error("CFE-ES: CDS Invalid Name")]
    CfeEsCdsInvalidName,
    /// CDS size is beyond the applicable limits.
    #[error("CFE-ES: CDS Invalid Size")]
    CfeEsCdsInvalidSize,
    /// The CDS contents are invalid.
    #[error("CFE-ES: CDS Invalid")]
    CfeEsCdsInvalid,
    /// The CDS was inaccessible.
    #[error("CFE-ES: CDS Access Error")]
    CfeEsCdsAccessError,
    /// A file operation failed.
    #[error("CFE-ES: File IO Error")]
    CfeEsFileIoErr,
    /// BSP failed to return the reset area address.
    #[error("CFE-ES: Reset Area Access Error")]
    CfeEsRstAccessErr,
    /// A task cannot be registered in ES global tables.
    #[error("CFE-ES: Application Register Error")]
    CfeEsErrAppRegister,
    /// Error deleting a child task.
    #[error("CFE-ES: Child Task Delete Error")]
    CfeEsErrChildTaskDelete,
    /// Attempted to delete a cFE App Main Task with DeleteChildTask API.
    #[error("CFE-ES: Attempted to delete a main task")]
    CfeEsErrChildTaskDeleteMainTask,
    /// CDS Data Block CRC does not match stored CRC.
    #[error("CFE-ES: CDS Block CRC Error")]
    CfeEsCdsBlockCrcErr,
    /// Error deleting a Mutex Semaphore during task cleanup.
    #[error("CFE-ES: Mutex Semaphore Delete Error")]
    CfeEsMutSemDeleteErr,
    /// Error deleting a Binary Semaphore during task cleanup.
    #[error("CFE-ES: Binary Semaphore Delete Error")]
    CfeEsBinSemDeleteErr,
    /// Error deleting a Counting Semaphore during task cleanup.
    #[error("CFE-ES: Counting Semaphore Delete Error")]
    CfeEsCountSemDeleteErr,
    /// Error deleting a Queue during task cleanup.
    #[error("CFE-ES: Queue Delete Error")]
    CfeEsQueueDeleteErr,
    /// Error closing a file during task cleanup.
    #[error("CFE-ES: File Close Error")]
    CfeEsFileCloseErr,
    /// CDS is not the correct type for the operation.
    #[error("CFE-ES: CDS Wrong Type Error")]
    CfeEsCdsWrongTypeErr,
    /// Attempted to delete a CDS while its owner application is still active.
    #[error("CFE-ES: CDS Owner Active Error")]
    CfeEsCdsOwnerActiveErr,
    /// An error occurred during application cleanup.
    #[error("CFE-ES: Application Cleanup Error")]
    CfeEsAppCleanupErr,
    /// Error deleting a Timer during task cleanup.
    #[error("CFE-ES: Timer Delete Error")]
    CfeEsTimerDeleteErr,
    /// The specified address is not in the memory pool.
    #[error("CFE-ES: Buffer Not In Pool")]
    CfeEsBufferNotInPool,
    /// Error deleting a task during cleanup.
    #[error("CFE-ES: Task Delete Error")]
    CfeEsTaskDeleteErr,
    /// The timeout for a given operation was exceeded.
    #[error("CFE-ES: Operation Timed Out")]
    CfeEsOperationTimedOut,
    /// Maximum number of resource identifiers has been reached.
    #[error("CFE-ES: No Resource IDs Available")]
    CfeEsNoResourceIdsAvailable,
    /// Attempted to "put" a block back into a pool which does not belong to it.
    #[error("CFE-ES: Invalid pool block")]
    CfeEsPoolBlockInvalid,
    /// Resource creation failed due to the name already existing.
    #[error("CFE-ES: Duplicate Name Error")]
    CfeEsErrDuplicateName,
    /// ES feature is not implemented.
    #[error("CFE-ES: Not Implemented")]
    CfeEsNotImplemented,

    // --- CFE FS (File Services) ---
    /// A parameter given to a File Services API did not pass validation.
    #[error("CFE-FS: Bad Argument")]
    CfeFsBadArgument,
    /// FS was unable to extract a filename from a path string.
    #[error("CFE-FS: Invalid Path")]
    CfeFsInvalidPath,
    /// FS filename string is too long.
    #[error("CFE-FS: Filename Too Long")]
    CfeFsFnameTooLong,
    /// FS feature is not implemented.
    #[error("CFE-FS: Not Implemented")]
    CfeFsNotImplemented,

    // --- CFE SB (Software Bus) ---
    /// A pipe receive operation timed out.
    #[error("CFE-SB: Time Out")]
    CfeSbTimeOut,
    /// A pipe was polled but contained no message.
    #[error("CFE-SB: No Message")]
    CfeSbNoMessage,
    /// A parameter given to a Software Bus API did not pass validation.
    #[error("CFE-SB: Bad Argument")]
    CfeSbBadArgument,
    /// The maximum number of pipes are already in use.
    #[error("CFE-SB: Max Pipes Met")]
    CfeSbMaxPipesMet,
    /// The underlying OS queue for a pipe could not be created.
    #[error("CFE-SB: Pipe Create Error")]
    CfeSbPipeCrErr,
    /// An error occurred at the OS queue read level.
    #[error("CFE-SB: Pipe Read Error")]
    CfeSbPipeRdErr,
    /// The message size exceeds the maximum allowed SB message size.
    #[error("CFE-SB: Message Too Big")]
    CfeSbMsgTooBig,
    /// The SB message buffer pool has been depleted.
    #[error("CFE-SB: Buffer Allocation Error")]
    CfeSbBufAlocErr,
    /// The SB routing table cannot accommodate another unique message ID.
    #[error("CFE-SB: Max Messages Met")]
    CfeSbMaxMsgsMet,
    /// The SB routing table cannot accommodate another destination for a message ID.
    #[error("CFE-SB: Max Destinations Met")]
    CfeSbMaxDestsMet,
    /// An internal SB index is out of range.
    #[error("CFE-SB: CFE-Internal Error")]
    CfeSbInternalErr,
    /// A message header operation was requested on a message of the wrong type.
    #[error("CFE-SB: Wrong Message Type")]
    CfeSbWrongMsgType,
    /// A request to release or send a zero-copy buffer is invalid.
    #[error("CFE-SB: Buffer Invalid")]
    CfeSbBufferInvalid,
    /// SB feature is not implemented.
    #[error("CFE-SB: Not Implemented")]
    CfeSbNotImplemented,

    // --- CFE TBL (Table Services) ---
    /// The provided table handle is not valid.
    #[error("CFE-TBL: Invalid Handle")]
    CfeTblErrInvalidHandle,
    /// The provided table name is not valid.
    #[error("CFE-TBL: Invalid Name")]
    CfeTblErrInvalidName,
    /// The provided table size is not valid.
    #[error("CFE-TBL: Invalid Size")]
    CfeTblErrInvalidSize,
    /// The table has not yet been loaded with data.
    #[error("CFE-TBL: Never Loaded")]
    CfeTblErrNeverLoaded,
    /// The table registry is full.
    #[error("CFE-TBL: Registry Full")]
    CfeTblErrRegistryFull,
    /// The application does not have access to the table.
    #[error("CFE-TBL: No Access")]
    CfeTblErrNoAccess,
    /// The application is trying to access an unregistered table.
    #[error("CFE-TBL: Unregistered")]
    CfeTblErrUnregistered,
    /// The table handle array is full.
    #[error("CFE-TBL: Handles Full")]
    CfeTblErrHandlesFull,
    /// An app tried to register a table with the same name but a different size.
    #[error("CFE-TBL: Duplicate Table With Different Size")]
    CfeTblErrDuplicateDiffSize,
    /// An app tried to register a table owned by a different application.
    #[error("CFE-TBL: Duplicate Table And Not Owned")]
    CfeTblErrDuplicateNotOwned,
    /// No working buffer was available for a table load.
    #[error("CFE-TBL: No Buffer Available")]
    CfeTblErrNoBufferAvail,
    /// A load was attempted on a "Dump Only" table.
    #[error("CFE-TBL: Dump Only Error")]
    CfeTblErrDumpOnly,
    /// The source type for a table load was illegal.
    #[error("CFE-TBL: Illegal Source Type")]
    CfeTblErrIllegalSrcType,
    /// A table load was attempted while another load was in progress.
    #[error("CFE-TBL: Load In Progress")]
    CfeTblErrLoadInProgress,
    /// The table file is larger than the table's buffer.
    #[error("CFE-TBL: File Too Large")]
    CfeTblErrFileTooLarge,
    /// The table file's content ID was not that of a table image.
    #[error("CFE-TBL: Bad Content ID")]
    CfeTblErrBadContentId,
    /// The table file's Subtype ID was not a table image file.
    #[error("CFE-TBL: Bad Subtype ID")]
    CfeTblErrBadSubtypeId,
    /// The table file's size is inconsistent with its header.
    #[error("CFE-TBL: File Size Inconsistent")]
    CfeTblErrFileSizeInconsistent,
    /// The table file's standard cFE File Header was invalid.
    #[error("CFE-TBL: No Standard Header")]
    CfeTblErrNoStdHeader,
    /// The table file's cFE Table File Header was invalid.
    #[error("CFE-TBL: No Table Header")]
    CfeTblErrNoTblHeader,
    /// The filename for a table load was too long.
    #[error("CFE-TBL: Filename Too Long")]
    CfeTblErrFilenameTooLong,
    /// The table file header indicates it is for a different table.
    #[error("CFE-TBL: File For Wrong Table")]
    CfeTblErrFileForWrongTable,
    /// The table file load was larger than what was read from the file.
    #[error("CFE-TBL: Load Incomplete")]
    CfeTblErrLoadIncomplete,
    /// A partial load was attempted on an uninitialized table.
    #[error("CFE-TBL: Partial Load Error")]
    CfeTblErrPartialLoad,
    /// An illegal combination of table options was used.
    #[error("CFE-TBL: Invalid Options")]
    CfeTblErrInvalidOptions,
    /// The table file failed validation for Spacecraft ID.
    #[error("CFE-TBL: Bad Spacecraft ID")]
    CfeTblErrBadSpacecraftId,
    /// The table file failed validation for Processor ID.
    #[error("CFE-TBL: Bad Processor ID")]
    CfeTblErrBadProcessorId,
    /// The TBL command was not processed successfully.
    #[error("CFE-TBL: Message Error")]
    CfeTblMessageError,
    /// The TBL file is shorter than indicated in the file header.
    #[error("CFE-TBL: Short File")]
    CfeTblErrShortFile,
    /// The TBL file could not be opened by the OS.
    #[error("CFE-TBL: Access error")]
    CfeTblErrAccess,
    /// A parameter given to a Table API did not pass validation.
    #[error("CFE-TBL: Bad Argument")]
    CfeTblBadArgument,
    /// TBL feature is not implemented.
    #[error("CFE-TBL: Not Implemented")]
    CfeTblNotImplemented,

    // --- CFE TIME (Time Services) ---
    /// TIME feature is not implemented.
    #[error("CFE-TIME: Not Implemented")]
    CfeTimeNotImplemented,
    /// TIME Services is commanded to not accept external time data.
    #[error("CFE-TIME: Internal Only")]
    CfeTimeInternalOnly,
    /// New time data from an external source is invalid.
    #[error("CFE-TIME: Out Of Range")]
    CfeTimeOutOfRange,
    /// An attempt to register too many Time Services Synchronization callbacks was made.
    #[error("CFE-TIME: Too Many Sync Callbacks")]
    CfeTimeTooManySynchCallbacks,
    /// The specified callback function was not in the Synchronization Callback Registry.
    #[error("CFE-TIME: Callback Not Registered")]
    CfeTimeCallbackNotRegistered,
    /// A parameter given to a TIME Services API did not pass validation.
    #[error("CFE-TIME: Bad Argument")]
    CfeTimeBadArgument,

    // --- OSAL Status Codes ---
    /// Failed execution.
    #[error("OSAL: Generic error")]
    OsError,
    /// Invalid pointer.
    #[error("OSAL: Invalid pointer")]
    OsInvalidPointer,
    /// Address misalignment.
    #[error("OSAL: Address misalignment")]
    OsErrorAddressMisaligned,
    /// Timeout.
    #[error("OSAL: Timeout")]
    OsErrorTimeout,
    /// Invalid Interrupt number.
    #[error("OSAL: Invalid Interrupt number")]
    OsInvalidIntNum,
    /// Semaphore failure.
    #[error("OSAL: Semaphore failure")]
    OsSemFailure,
    /// Semaphore timeout.
    #[error("OSAL: Semaphore timeout")]
    OsSemTimeout,
    /// Queue empty.
    #[error("OSAL: Queue empty")]
    OsQueueEmpty,
    /// Queue full.
    #[error("OSAL: Queue full")]
    OsQueueFull,
    /// Queue timeout.
    #[error("OSAL: Queue timeout")]
    OsQueueTimeout,
    /// Queue invalid size.
    #[error("OSAL: Queue invalid size")]
    OsQueueInvalidSize,
    /// Queue ID error.
    #[error("OSAL: Queue ID error")]
    OsQueueIdError,
    /// Name length too long.
    #[error("OSAL: Name length too long")]
    OsErrNameTooLong,
    /// No free IDs.
    #[error("OSAL: No free IDs")]
    OsErrNoFreeIds,
    /// Name taken.
    #[error("OSAL: Name taken")]
    OsErrNameTaken,
    /// Invalid ID.
    #[error("OSAL: Invalid ID")]
    OsErrInvalidId,
    /// Name not found.
    #[error("OSAL: Name not found")]
    OsErrNameNotFound,
    /// Semaphore not full.
    #[error("OSAL: Semaphore not full")]
    OsErrSemNotFull,
    /// Invalid priority.
    #[error("OSAL: Invalid priority")]
    OsErrInvalidPriority,
    /// Invalid semaphore value.
    #[error("OSAL: Invalid semaphore value")]
    OsInvalidSemValue,
    /// Generic file error.
    #[error("OSAL: File error")]
    OsErrFile,
    /// Not implemented.
    #[error("OSAL: Not implemented")]
    OsErrNotImplemented,
    /// Timer invalid arguments.
    #[error("OSAL: Timer invalid arguments")]
    OsTimerErrInvalidArgs,
    /// Timer ID error.
    #[error("OSAL: Timer ID error")]
    OsTimerErrTimerId,
    /// Timer unavailable.
    #[error("OSAL: Timer unavailable")]
    OsTimerErrUnavailable,
    /// Timer internal error.
    #[error("OSAL: Timer internal error")]
    OsTimerErrInternal,
    /// Object in use.
    #[error("OSAL: Object in use")]
    OsErrObjectInUse,
    /// Bad address.
    #[error("OSAL: Bad address")]
    OsErrBadAddress,
    /// Incorrect object state.
    #[error("OSAL: Incorrect object state")]
    OsErrIncorrectObjState,
    /// Incorrect object type.
    #[error("OSAL: Incorrect object type")]
    OsErrIncorrectObjType,
    /// Stream disconnected.
    #[error("OSAL: Stream disconnected")]
    OsErrStreamDisconnected,
    /// Requested operation not supported on supplied object(s).
    #[error("OSAL: Requested operation not supported on supplied object(s)")]
    OsErrOperationNotSupported,
    /// Invalid Size.
    #[error("OSAL: Invalid Size")]
    OsErrInvalidSize,
    /// Size of output exceeds limit.
    #[error("OSAL: Size of output exceeds limit")]
    OsErrOutputTooLarge,
    /// Invalid argument value.
    #[error("OSAL: Invalid argument value")]
    OsErrInvalidArgument,
    /// FS path too long.
    #[error("OSAL: FS path too long")]
    OsFsErrPathTooLong,
    /// FS name too long.
    #[error("OSAL: FS name too long")]
    OsFsErrNameTooLong,
    /// FS drive not created.
    #[error("OSAL: FS drive not created")]
    OsFsErrDriveNotCreated,
    /// FS device not free.
    #[error("OSAL: FS device not free")]
    OsFsErrDeviceNotFree,
    /// FS path invalid.
    #[error("OSAL: FS path invalid")]
    OsFsErrPathInvalid,

    // --- Other Errors ---
    /// A string passed to the API contained an interior null character.
    #[error("Invalid string: contains interior null character")]
    InvalidString,
    /// The task pool is full and cannot accept new tasks.
    #[error("The task pool is full")]
    TaskPoolFull,
    /// The task's stack size exceeds the maximum allowed size.
    #[error("The task's stack size exceeds the maximum allowed size")]
    TaskTooLarge,
    /// The task's alignment requirement exceeds the maximum allowed alignment.
    #[error("The task's alignment requirement exceeds the maximum allowed alignment")]
    TaskAlignmentTooLarge,
    /// An error occurred during a type conversion.
    #[error("Type conversion error")]
    ConversionError(&'static str),

    /// An unhandled or unknown CFE/OSAL/PSP error code. This may indicate a
    /// new or platform-specific error code not yet included in this enum.
    #[error("Unhandled error code: {0}")]
    Unhandled(i32),
}

impl From<ffi::CFE_Status_t> for Error {
    /// Converts a raw `CFE_Status_t` code into an `Error` enum.
    fn from(status: ffi::CFE_Status_t) -> Self {
        match status {
            // --- Generic CFE Status ---
            ffi::CFE_STATUS_WRONG_MSG_LENGTH => Error::CfeStatusWrongMsgLength,
            ffi::CFE_STATUS_UNKNOWN_MSG_ID => Error::CfeStatusUnknownMsgId,
            ffi::CFE_STATUS_BAD_COMMAND_CODE => Error::CfeStatusBadCommandCode,
            ffi::CFE_STATUS_EXTERNAL_RESOURCE_FAIL => Error::CfeStatusExternalResourceFail,
            ffi::CFE_STATUS_REQUEST_ALREADY_PENDING => Error::CfeStatusRequestAlreadyPending,
            ffi::CFE_STATUS_VALIDATION_FAILURE => Error::CfeStatusValidationFailure,
            ffi::CFE_STATUS_RANGE_ERROR => Error::CfeStatusRangeError,
            ffi::CFE_STATUS_INCORRECT_STATE => Error::CfeStatusIncorrectState,
            ffi::CFE_STATUS_NOT_IMPLEMENTED => Error::CfeStatusNotImplemented,

            // --- CFE EVS (Event Services) ---
            ffi::CFE_EVS_UNKNOWN_FILTER => Error::CfeEvsUnknownFilter,
            ffi::CFE_EVS_APP_NOT_REGISTERED => Error::CfeEvsAppNotRegistered,
            ffi::CFE_EVS_APP_ILLEGAL_APP_ID => Error::CfeEvsAppIllegalAppId,
            ffi::CFE_EVS_APP_FILTER_OVERLOAD => Error::CfeEvsAppFilterOverload,
            ffi::CFE_EVS_RESET_AREA_POINTER => Error::CfeEvsResetAreaPointer,
            ffi::CFE_EVS_EVT_NOT_REGISTERED => Error::CfeEvsEvtNotRegistered,
            ffi::CFE_EVS_FILE_WRITE_ERROR => Error::CfeEvsFileWriteError,
            ffi::CFE_EVS_INVALID_PARAMETER => Error::CfeEvsInvalidParameter,
            ffi::CFE_EVS_APP_SQUELCHED => Error::CfeEvsAppSquelched,
            ffi::CFE_EVS_NOT_IMPLEMENTED => Error::CfeEvsNotImplemented,

            // --- CFE ES (Executive Services) ---
            ffi::CFE_ES_ERR_RESOURCEID_NOT_VALID => Error::CfeEsErrResourceIdNotValid,
            ffi::CFE_ES_ERR_NAME_NOT_FOUND => Error::CfeEsErrNameNotFound,
            ffi::CFE_ES_ERR_APP_CREATE => Error::CfeEsErrAppCreate,
            ffi::CFE_ES_ERR_CHILD_TASK_CREATE => Error::CfeEsErrChildTaskCreate,
            ffi::CFE_ES_ERR_SYS_LOG_FULL => Error::CfeEsErrSysLogFull,
            ffi::CFE_ES_ERR_MEM_BLOCK_SIZE => Error::CfeEsErrMemBlockSize,
            ffi::CFE_ES_ERR_LOAD_LIB => Error::CfeEsErrLoadLib,
            ffi::CFE_ES_BAD_ARGUMENT => Error::CfeEsBadArgument,
            ffi::CFE_ES_ERR_CHILD_TASK_REGISTER => Error::CfeEsErrChildTaskRegister,
            ffi::CFE_ES_CDS_INSUFFICIENT_MEMORY => Error::CfeEsCdsInsufficientMemory,
            ffi::CFE_ES_CDS_INVALID_NAME => Error::CfeEsCdsInvalidName,
            ffi::CFE_ES_CDS_INVALID_SIZE => Error::CfeEsCdsInvalidSize,
            ffi::CFE_ES_CDS_INVALID => Error::CfeEsCdsInvalid,
            ffi::CFE_ES_CDS_ACCESS_ERROR => Error::CfeEsCdsAccessError,
            ffi::CFE_ES_FILE_IO_ERR => Error::CfeEsFileIoErr,
            ffi::CFE_ES_RST_ACCESS_ERR => Error::CfeEsRstAccessErr,
            ffi::CFE_ES_ERR_APP_REGISTER => Error::CfeEsErrAppRegister,
            ffi::CFE_ES_ERR_CHILD_TASK_DELETE => Error::CfeEsErrChildTaskDelete,
            ffi::CFE_ES_ERR_CHILD_TASK_DELETE_MAIN_TASK => Error::CfeEsErrChildTaskDeleteMainTask,
            ffi::CFE_ES_CDS_BLOCK_CRC_ERR => Error::CfeEsCdsBlockCrcErr,
            ffi::CFE_ES_MUT_SEM_DELETE_ERR => Error::CfeEsMutSemDeleteErr,
            ffi::CFE_ES_BIN_SEM_DELETE_ERR => Error::CfeEsBinSemDeleteErr,
            ffi::CFE_ES_COUNT_SEM_DELETE_ERR => Error::CfeEsCountSemDeleteErr,
            ffi::CFE_ES_QUEUE_DELETE_ERR => Error::CfeEsQueueDeleteErr,
            ffi::CFE_ES_FILE_CLOSE_ERR => Error::CfeEsFileCloseErr,
            ffi::CFE_ES_CDS_WRONG_TYPE_ERR => Error::CfeEsCdsWrongTypeErr,
            ffi::CFE_ES_CDS_OWNER_ACTIVE_ERR => Error::CfeEsCdsOwnerActiveErr,
            ffi::CFE_ES_APP_CLEANUP_ERR => Error::CfeEsAppCleanupErr,
            ffi::CFE_ES_TIMER_DELETE_ERR => Error::CfeEsTimerDeleteErr,
            ffi::CFE_ES_BUFFER_NOT_IN_POOL => Error::CfeEsBufferNotInPool,
            ffi::CFE_ES_TASK_DELETE_ERR => Error::CfeEsTaskDeleteErr,
            ffi::CFE_ES_OPERATION_TIMED_OUT => Error::CfeEsOperationTimedOut,
            ffi::CFE_ES_NO_RESOURCE_IDS_AVAILABLE => Error::CfeEsNoResourceIdsAvailable,
            ffi::CFE_ES_POOL_BLOCK_INVALID => Error::CfeEsPoolBlockInvalid,
            ffi::CFE_ES_ERR_DUPLICATE_NAME => Error::CfeEsErrDuplicateName,
            ffi::CFE_ES_NOT_IMPLEMENTED => Error::CfeEsNotImplemented,

            // --- CFE FS (File Services) ---
            ffi::CFE_FS_BAD_ARGUMENT => Error::CfeFsBadArgument,
            ffi::CFE_FS_INVALID_PATH => Error::CfeFsInvalidPath,
            ffi::CFE_FS_FNAME_TOO_LONG => Error::CfeFsFnameTooLong,
            ffi::CFE_FS_NOT_IMPLEMENTED => Error::CfeFsNotImplemented,

            // --- CFE SB (Software Bus) ---
            ffi::CFE_SB_TIME_OUT => Error::CfeSbTimeOut,
            ffi::CFE_SB_NO_MESSAGE => Error::CfeSbNoMessage,
            ffi::CFE_SB_BAD_ARGUMENT => Error::CfeSbBadArgument,
            ffi::CFE_SB_MAX_PIPES_MET => Error::CfeSbMaxPipesMet,
            ffi::CFE_SB_PIPE_CR_ERR => Error::CfeSbPipeCrErr,
            ffi::CFE_SB_PIPE_RD_ERR => Error::CfeSbPipeRdErr,
            ffi::CFE_SB_MSG_TOO_BIG => Error::CfeSbMsgTooBig,
            ffi::CFE_SB_BUF_ALOC_ERR => Error::CfeSbBufAlocErr,
            ffi::CFE_SB_MAX_MSGS_MET => Error::CfeSbMaxMsgsMet,
            ffi::CFE_SB_MAX_DESTS_MET => Error::CfeSbMaxDestsMet,
            ffi::CFE_SB_INTERNAL_ERR => Error::CfeSbInternalErr,
            ffi::CFE_SB_WRONG_MSG_TYPE => Error::CfeSbWrongMsgType,
            ffi::CFE_SB_BUFFER_INVALID => Error::CfeSbBufferInvalid,
            ffi::CFE_SB_NOT_IMPLEMENTED => Error::CfeSbNotImplemented,

            // --- CFE TBL (Table Services) ---
            ffi::CFE_TBL_ERR_INVALID_HANDLE => Error::CfeTblErrInvalidHandle,
            ffi::CFE_TBL_ERR_INVALID_NAME => Error::CfeTblErrInvalidName,
            ffi::CFE_TBL_ERR_INVALID_SIZE => Error::CfeTblErrInvalidSize,
            ffi::CFE_TBL_ERR_NEVER_LOADED => Error::CfeTblErrNeverLoaded,
            ffi::CFE_TBL_ERR_REGISTRY_FULL => Error::CfeTblErrRegistryFull,
            ffi::CFE_TBL_ERR_NO_ACCESS => Error::CfeTblErrNoAccess,
            ffi::CFE_TBL_ERR_UNREGISTERED => Error::CfeTblErrUnregistered,
            ffi::CFE_TBL_ERR_HANDLES_FULL => Error::CfeTblErrHandlesFull,
            ffi::CFE_TBL_ERR_DUPLICATE_DIFF_SIZE => Error::CfeTblErrDuplicateDiffSize,
            ffi::CFE_TBL_ERR_DUPLICATE_NOT_OWNED => Error::CfeTblErrDuplicateNotOwned,
            ffi::CFE_TBL_ERR_NO_BUFFER_AVAIL => Error::CfeTblErrNoBufferAvail,
            ffi::CFE_TBL_ERR_DUMP_ONLY => Error::CfeTblErrDumpOnly,
            ffi::CFE_TBL_ERR_ILLEGAL_SRC_TYPE => Error::CfeTblErrIllegalSrcType,
            ffi::CFE_TBL_ERR_LOAD_IN_PROGRESS => Error::CfeTblErrLoadInProgress,
            ffi::CFE_TBL_ERR_FILE_TOO_LARGE => Error::CfeTblErrFileTooLarge,
            ffi::CFE_TBL_ERR_BAD_CONTENT_ID => Error::CfeTblErrBadContentId,
            ffi::CFE_TBL_ERR_BAD_SUBTYPE_ID => Error::CfeTblErrBadSubtypeId,
            ffi::CFE_TBL_ERR_FILE_SIZE_INCONSISTENT => Error::CfeTblErrFileSizeInconsistent,
            ffi::CFE_TBL_ERR_NO_STD_HEADER => Error::CfeTblErrNoStdHeader,
            ffi::CFE_TBL_ERR_NO_TBL_HEADER => Error::CfeTblErrNoTblHeader,
            ffi::CFE_TBL_ERR_FILENAME_TOO_LONG => Error::CfeTblErrFilenameTooLong,
            ffi::CFE_TBL_ERR_FILE_FOR_WRONG_TABLE => Error::CfeTblErrFileForWrongTable,
            ffi::CFE_TBL_ERR_LOAD_INCOMPLETE => Error::CfeTblErrLoadIncomplete,
            ffi::CFE_TBL_ERR_PARTIAL_LOAD => Error::CfeTblErrPartialLoad,
            ffi::CFE_TBL_ERR_INVALID_OPTIONS => Error::CfeTblErrInvalidOptions,
            ffi::CFE_TBL_ERR_BAD_SPACECRAFT_ID => Error::CfeTblErrBadSpacecraftId,
            ffi::CFE_TBL_ERR_BAD_PROCESSOR_ID => Error::CfeTblErrBadProcessorId,
            ffi::CFE_TBL_MESSAGE_ERROR => Error::CfeTblMessageError,
            ffi::CFE_TBL_ERR_SHORT_FILE => Error::CfeTblErrShortFile,
            ffi::CFE_TBL_ERR_ACCESS => Error::CfeTblErrAccess,
            ffi::CFE_TBL_BAD_ARGUMENT => Error::CfeTblBadArgument,
            ffi::CFE_TBL_NOT_IMPLEMENTED => Error::CfeTblNotImplemented,

            // --- CFE TIME (Time Services) ---
            ffi::CFE_TIME_NOT_IMPLEMENTED => Error::CfeTimeNotImplemented,
            ffi::CFE_TIME_INTERNAL_ONLY => Error::CfeTimeInternalOnly,
            ffi::CFE_TIME_OUT_OF_RANGE => Error::CfeTimeOutOfRange,
            ffi::CFE_TIME_TOO_MANY_SYNCH_CALLBACKS => Error::CfeTimeTooManySynchCallbacks,
            ffi::CFE_TIME_CALLBACK_NOT_REGISTERED => Error::CfeTimeCallbackNotRegistered,
            ffi::CFE_TIME_BAD_ARGUMENT => Error::CfeTimeBadArgument,

            // --- OSAL Status Codes ---
            ffi::OS_ERROR => Error::OsError,
            ffi::OS_INVALID_POINTER => Error::OsInvalidPointer,
            ffi::OS_ERROR_ADDRESS_MISALIGNED => Error::OsErrorAddressMisaligned,
            ffi::OS_ERROR_TIMEOUT => Error::OsErrorTimeout,
            ffi::OS_INVALID_INT_NUM => Error::OsInvalidIntNum,
            ffi::OS_SEM_FAILURE => Error::OsSemFailure,
            ffi::OS_SEM_TIMEOUT => Error::OsSemTimeout,
            ffi::OS_QUEUE_EMPTY => Error::OsQueueEmpty,
            ffi::OS_QUEUE_FULL => Error::OsQueueFull,
            ffi::OS_QUEUE_TIMEOUT => Error::OsQueueTimeout,
            ffi::OS_QUEUE_INVALID_SIZE => Error::OsQueueInvalidSize,
            ffi::OS_QUEUE_ID_ERROR => Error::OsQueueIdError,
            ffi::OS_ERR_NAME_TOO_LONG => Error::OsErrNameTooLong,
            ffi::OS_ERR_NO_FREE_IDS => Error::OsErrNoFreeIds,
            ffi::OS_ERR_NAME_TAKEN => Error::OsErrNameTaken,
            ffi::OS_ERR_INVALID_ID => Error::OsErrInvalidId,
            ffi::OS_ERR_NAME_NOT_FOUND => Error::OsErrNameNotFound,
            ffi::OS_ERR_SEM_NOT_FULL => Error::OsErrSemNotFull,
            ffi::OS_ERR_INVALID_PRIORITY => Error::OsErrInvalidPriority,
            ffi::OS_INVALID_SEM_VALUE => Error::OsInvalidSemValue,
            ffi::OS_ERR_FILE => Error::OsErrFile,
            ffi::OS_ERR_NOT_IMPLEMENTED => Error::OsErrNotImplemented,
            ffi::OS_TIMER_ERR_INVALID_ARGS => Error::OsTimerErrInvalidArgs,
            ffi::OS_TIMER_ERR_TIMER_ID => Error::OsTimerErrTimerId,
            ffi::OS_TIMER_ERR_UNAVAILABLE => Error::OsTimerErrUnavailable,
            ffi::OS_TIMER_ERR_INTERNAL => Error::OsTimerErrInternal,
            ffi::OS_ERR_OBJECT_IN_USE => Error::OsErrObjectInUse,
            ffi::OS_ERR_BAD_ADDRESS => Error::OsErrBadAddress,
            ffi::OS_ERR_INCORRECT_OBJ_STATE => Error::OsErrIncorrectObjState,
            ffi::OS_ERR_INCORRECT_OBJ_TYPE => Error::OsErrIncorrectObjType,
            ffi::OS_ERR_STREAM_DISCONNECTED => Error::OsErrStreamDisconnected,
            ffi::OS_ERR_OPERATION_NOT_SUPPORTED => Error::OsErrOperationNotSupported,
            ffi::OS_ERR_INVALID_SIZE => Error::OsErrInvalidSize,
            ffi::OS_ERR_OUTPUT_TOO_LARGE => Error::OsErrOutputTooLarge,
            ffi::OS_ERR_INVALID_ARGUMENT => Error::OsErrInvalidArgument,
            ffi::OS_FS_ERR_PATH_TOO_LONG => Error::OsFsErrPathTooLong,
            ffi::OS_FS_ERR_NAME_TOO_LONG => Error::OsFsErrNameTooLong,
            ffi::OS_FS_ERR_DRIVE_NOT_CREATED => Error::OsFsErrDriveNotCreated,
            ffi::OS_FS_ERR_DEVICE_NOT_FREE => Error::OsFsErrDeviceNotFree,
            ffi::OS_FS_ERR_PATH_INVALID => Error::OsFsErrPathInvalid,

            other => Error::Unhandled(other),
        }
    }
}

impl Error {
    /// Retrieves the symbolic name for an OSAL status code.
    pub fn name(error: i32) -> Result<CString<{ ffi::OS_ERROR_NAME_LENGTH as usize }>> {
        const SIZE: usize = ffi::OS_ERROR_NAME_LENGTH as usize;
        let mut name_buf = [0u8; SIZE];
        check(unsafe {
            ffi::OS_GetErrorName(error, &mut name_buf as *mut _ as *mut [libc::c_char; SIZE])
        })?;

        // The result from OS_GetErrorName is null-terminated.
        let mut s = CString::new();
        s.extend_from_bytes(&name_buf)
            .map_err(|_| Error::OsErrNameTooLong)?;
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
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}

/// Converts an OSAL status code to its decimal or hex string representation.
pub fn osal_status_to_string(
    status: i32,
) -> Result<CString<{ ffi::OS_STATUS_STRING_LENGTH as usize }>> {
    const SIZE: usize = ffi::OS_STATUS_STRING_LENGTH as usize;
    let mut name_buf = [0u8; SIZE];
    unsafe { ffi::OS_StatusToString(status, &mut name_buf as *mut _ as *mut [libc::c_char; SIZE]) };

    let mut s = CString::new();
    s.extend_from_bytes(&name_buf)
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}
