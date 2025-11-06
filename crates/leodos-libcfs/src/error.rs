//! Error types for cFS.

use crate::ffi;
use crate::status::check;
use core::fmt;
use heapless::CString;

/// A specialized `Result` type for CFE operations.
pub type Result<T> = core::result::Result<T, Error>;

/// Represents all possible errors and status codes from the CFE and OSAL APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    // --- Generic CFE Status ---
    /// Wrong Message Length.
    CfeStatusWrongMsgLength,
    /// Unknown Message ID.
    CfeStatusUnknownMsgId,
    /// Bad Command Code.
    CfeStatusBadCommandCode,
    /// External resource failure.
    CfeStatusExternalResourceFail,
    /// Request already pending.
    CfeStatusRequestAlreadyPending,
    /// Request or input value failed basic structural validation.
    CfeStatusValidationFailure,
    /// Request or input value is out of range.
    CfeStatusRangeError,
    /// Cannot process request at this time.
    CfeStatusIncorrectState,
    /// Not Implemented.
    CfeStatusNotImplemented,

    // --- CFE EVS (Event Services) ---
    /// Unknown Filter scheme.
    CfeEvsUnknownFilter,
    /// Application not registered with EVS.
    CfeEvsAppNotRegistered,
    /// Illegal Application ID.
    CfeEvsAppIllegalAppId,
    /// Application event filter overload.
    CfeEvsAppFilterOverload,
    /// Could not get pointer to the ES Reset area.
    CfeEvsResetAreaPointer,
    /// EventID argument was not found in any registered event filter.
    CfeEvsEvtNotRegistered,
    /// A file write error occurred while processing an EVS command.
    CfeEvsFileWriteError,
    /// Invalid parameter supplied to EVS command.
    CfeEvsInvalidParameter,
    /// Event squelched due to being sent at too high a rate.
    CfeEvsAppSquelched,
    /// EVS feature is not implemented.
    CfeEvsNotImplemented,

    // --- CFE ES (Executive Services) ---
    /// Resource ID is not valid.
    CfeEsErrResourceIdNotValid,
    /// Resource Name not found.
    CfeEsErrNameNotFound,
    /// Error loading or creating an App.
    CfeEsErrAppCreate,
    /// Error creating a child task.
    CfeEsErrChildTaskCreate,
    /// The cFE system Log is full.
    CfeEsErrSysLogFull,
    /// The memory block size requested is invalid.
    CfeEsErrMemBlockSize,
    /// Could not load the shared library.
    CfeEsErrLoadLib,
    /// Bad parameter passed into an ES API.
    CfeEsBadArgument,
    /// Errors occurred when trying to register a child task.
    CfeEsErrChildTaskRegister,
    /// CDS is larger than the remaining CDS memory.
    CfeEsCdsInsufficientMemory,
    /// CDS name is too long or empty.
    CfeEsCdsInvalidName,
    /// CDS size is beyond the applicable limits.
    CfeEsCdsInvalidSize,
    /// The CDS contents are invalid.
    CfeEsCdsInvalid,
    /// The CDS was inaccessible.
    CfeEsCdsAccessError,
    /// A file operation failed.
    CfeEsFileIoErr,
    /// BSP failed to return the reset area address.
    CfeEsRstAccessErr,
    /// A task cannot be registered in ES global tables.
    CfeEsErrAppRegister,
    /// Error deleting a child task.
    CfeEsErrChildTaskDelete,
    /// Attempted to delete a cFE App Main Task with DeleteChildTask API.
    CfeEsErrChildTaskDeleteMainTask,
    /// CDS Data Block CRC does not match stored CRC.
    CfeEsCdsBlockCrcErr,
    /// Error deleting a Mutex Semaphore during task cleanup.
    CfeEsMutSemDeleteErr,
    /// Error deleting a Binary Semaphore during task cleanup.
    CfeEsBinSemDeleteErr,
    /// Error deleting a Counting Semaphore during task cleanup.
    CfeEsCountSemDeleteErr,
    /// Error deleting a Queue during task cleanup.
    CfeEsQueueDeleteErr,
    /// Error closing a file during task cleanup.
    CfeEsFileCloseErr,
    /// CDS is not the correct type for the operation.
    CfeEsCdsWrongTypeErr,
    /// Attempted to delete a CDS while its owner application is still active.
    CfeEsCdsOwnerActiveErr,
    /// An error occurred during application cleanup.
    CfeEsAppCleanupErr,
    /// Error deleting a Timer during task cleanup.
    CfeEsTimerDeleteErr,
    /// The specified address is not in the memory pool.
    CfeEsBufferNotInPool,
    /// Error deleting a task during cleanup.
    CfeEsTaskDeleteErr,
    /// The timeout for a given operation was exceeded.
    CfeEsOperationTimedOut,
    /// Maximum number of resource identifiers has been reached.
    CfeEsNoResourceIdsAvailable,
    /// Attempted to "put" a block back into a pool which does not belong to it.
    CfeEsPoolBlockInvalid,
    /// Resource creation failed due to the name already existing.
    CfeEsErrDuplicateName,
    /// ES feature is not implemented.
    CfeEsNotImplemented,

    // --- CFE FS (File Services) ---
    /// A parameter given to a File Services API did not pass validation.
    CfeFsBadArgument,
    /// FS was unable to extract a filename from a path string.
    CfeFsInvalidPath,
    /// FS filename string is too long.
    CfeFsFnameTooLong,
    /// FS feature is not implemented.
    CfeFsNotImplemented,

    // --- CFE SB (Software Bus) ---
    /// A pipe receive operation timed out.
    CfeSbTimeOut,
    /// A pipe was polled but contained no message.
    CfeSbNoMessage,
    /// A parameter given to a Software Bus API did not pass validation.
    CfeSbBadArgument,
    /// The maximum number of pipes are already in use.
    CfeSbMaxPipesMet,
    /// The underlying OS queue for a pipe could not be created.
    CfeSbPipeCrErr,
    /// An error occurred at the OS queue read level.
    CfeSbPipeRdErr,
    /// The message size exceeds the maximum allowed SB message size.
    CfeSbMsgTooBig,
    /// The SB message buffer pool has been depleted.
    CfeSbBufAlocErr,
    /// The SB routing table cannot accommodate another unique message ID.
    CfeSbMaxMsgsMet,
    /// The SB routing table cannot accommodate another destination for a message ID.
    CfeSbMaxDestsMet,
    /// An internal SB index is out of range.
    SbInternalErr,
    /// A message header operation was requested on a message of the wrong type.
    SbWrongMsgType,
    /// A request to release or send a zero-copy buffer is invalid.
    SbBufferInvalid,
    /// SB feature is not implemented.
    SbNotImplemented,

    // --- CFE TBL (Table Services) ---
    /// The provided table handle is not valid.
    CfeTblErrInvalidHandle,
    /// The provided table name is not valid.
    CfeTblErrInvalidName,
    /// The provided table size is not valid.
    CfeTblErrInvalidSize,
    /// The table has not yet been loaded with data.
    CfeTblErrNeverLoaded,
    /// The table registry is full.
    CfeTblErrRegistryFull,
    /// The application does not have access to the table.
    CfeTblErrNoAccess,
    /// The application is trying to access an unregistered table.
    CfeTblErrUnregistered,
    /// The table handle array is full.
    CfeTblErrHandlesFull,
    /// An app tried to register a table with the same name but a different size.
    CfeTblErrDuplicateDiffSize,
    /// An app tried to register a table owned by a different application.
    CfeTblErrDuplicateNotOwned,
    /// No working buffer was available for a table load.
    CfeTblErrNoBufferAvail,
    /// A load was attempted on a "Dump Only" table.
    CfeTblErrDumpOnly,
    /// The source type for a table load was illegal.
    CfeTblErrIllegalSrcType,
    /// A table load was attempted while another load was in progress.
    CfeTblErrLoadInProgress,
    /// The table file is larger than the table's buffer.
    CfeTblErrFileTooLarge,
    /// The table file's content ID was not that of a table image.
    CfeTblErrBadContentId,
    /// The table file's Subtype ID was not a table image file.
    CfeTblErrBadSubtypeId,
    /// The table file's size is inconsistent with its header.
    CfeTblErrFileSizeInconsistent,
    /// The table file's standard cFE File Header was invalid.
    CfeTblErrNoStdHeader,
    /// The table file's cFE Table File Header was invalid.
    CfeTblErrNoTblHeader,
    /// The filename for a table load was too long.
    CfeTblErrFilenameTooLong,
    /// The table file header indicates it is for a different table.
    CfeTblErrFileForWrongTable,
    /// The table file load was larger than what was read from the file.
    CfeTblErrLoadIncomplete,
    /// A partial load was attempted on an uninitialized table.
    CfeTblErrPartialLoad,
    /// An illegal combination of table options was used.
    CfeTblErrInvalidOptions,
    /// The table file failed validation for Spacecraft ID.
    CfeTblErrBadSpacecraftId,
    /// The table file failed validation for Processor ID.
    CfeTblErrBadProcessorId,
    /// The TBL command was not processed successfully.
    CfeTblMessageError,
    /// The TBL file is shorter than indicated in the file header.
    CfeTblErrShortFile,
    /// The TBL file could not be opened by the OS.
    CfeTblErrAccess,
    /// A parameter given to a Table API did not pass validation.
    CfeTblBadArgument,
    /// TBL feature is not implemented.
    CfeTblNotImplemented,

    // --- CFE TIME (Time Services) ---
    /// TIME feature is not implemented.
    CfeTimeNotImplemented,
    /// TIME Services is commanded to not accept external time data.
    CfeTimeInternalOnly,
    /// New time data from an external source is invalid.
    CfeTimeOutOfRange,
    /// An attempt to register too many Time Services Synchronization callbacks was made.
    CfeTimeTooManySynchCallbacks,
    /// The specified callback function was not in the Synchronization Callback Registry.
    CfeTimeCallbackNotRegistered,
    /// A parameter given to a TIME Services API did not pass validation.
    CfeTimeBadArgument,

    // --- OSAL Status Codes ---
    /// Failed execution.
    OsError,
    /// Invalid pointer.
    OsInvalidPointer,
    /// Address misalignment.
    OsErrorAddressMisaligned,
    /// Timeout.
    OsErrorTimeout,
    /// Invalid Interrupt number.
    OsInvalidIntNum,
    /// Semaphore failure.
    OsSemFailure,
    /// Semaphore timeout.
    OsSemTimeout,
    /// Queue empty.
    OsQueueEmpty,
    /// Queue full.
    OsQueueFull,
    /// Queue timeout.
    OsQueueTimeout,
    /// Queue invalid size.
    OsQueueInvalidSize,
    /// Queue ID error.
    OsQueueIdError,
    /// Name length too long.
    OsErrNameTooLong,
    /// No free IDs.
    OsErrNoFreeIds,
    /// Name taken.
    OsErrNameTaken,
    /// Invalid ID.
    OsErrInvalidId,
    /// Name not found.
    OsErrNameNotFound,
    /// Semaphore not full.
    OsErrSemNotFull,
    /// Invalid priority.
    OsErrInvalidPriority,
    /// Invalid semaphore value.
    OsInvalidSemValue,
    /// Generic file error.
    OsErrFile,
    /// Not implemented.
    OsErrNotImplemented,
    /// Timer invalid arguments.
    OsTimerErrInvalidArgs,
    /// Timer ID error.
    OsTimerErrTimerId,
    /// Timer unavailable.
    OsTimerErrUnavailable,
    /// Timer internal error.
    OsTimerErrInternal,
    /// Object in use.
    OsErrObjectInUse,
    /// Bad address.
    OsErrBadAddress,
    /// Incorrect object state.
    OsErrIncorrectObjState,
    /// Incorrect object type.
    OsErrIncorrectObjType,
    /// Stream disconnected.
    OsErrStreamDisconnected,
    /// Requested operation not supported on supplied object(s).
    OsErrOperationNotSupported,
    /// Invalid Size.
    OsErrInvalidSize,
    /// Size of output exceeds limit.
    OsErrOutputTooLarge,
    /// Invalid argument value.
    OsErrInvalidArgument,
    /// FS path too long.
    OsFsErrPathTooLong,
    /// FS name too long.
    OsFsErrNameTooLong,
    /// FS drive not created.
    OsFsErrDriveNotCreated,
    /// FS device not free.
    OsFsErrDeviceNotFree,
    /// FS path invalid.
    OsFsErrPathInvalid,

    // --- Other Errors ---
    /// A string passed to the API contained an interior null character.
    InvalidString,

    /// An unhandled or unknown CFE/OSAL/PSP error code. This may indicate a
    /// new or platform-specific error code not yet included in this enum.
    Unhandled(i32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Use `#[doc]` attributes as a source for display messages.
        // This is a simplified version; a more advanced solution could use a build script
        // or macro to automatically generate this from the doc comments.
        let desc = match self {
            // --- Generic CFE Status ---
            Error::CfeStatusWrongMsgLength => "Generic: Wrong Message Length",
            Error::CfeStatusUnknownMsgId => "Generic: Unknown Message ID",
            Error::CfeStatusBadCommandCode => "Generic: Bad Command Code",
            Error::CfeStatusExternalResourceFail => "Generic: External resource failure",
            Error::CfeStatusRequestAlreadyPending => "Generic: Request already pending",
            Error::CfeStatusValidationFailure => "Generic: Validation Failure",
            Error::CfeStatusRangeError => "Generic: Input value out of range",
            Error::CfeStatusIncorrectState => "Generic: Cannot process request in current state",
            Error::CfeStatusNotImplemented => "Generic: Not Implemented",

            // --- CFE EVS (Event Services) ---
            Error::CfeEvsUnknownFilter => "EVS: Unknown Filter scheme",
            Error::CfeEvsAppNotRegistered => "EVS: Application not registered",
            Error::CfeEvsAppIllegalAppId => "EVS: Illegal Application ID",
            Error::CfeEvsAppFilterOverload => "EVS: Application filter overload",
            Error::CfeEvsResetAreaPointer => "EVS: Reset Area Pointer Failure",
            Error::CfeEvsEvtNotRegistered => "EVS: Event not registered for filtering",
            Error::CfeEvsFileWriteError => "EVS: File write error",
            Error::CfeEvsInvalidParameter => "EVS: Invalid parameter in command",
            Error::CfeEvsAppSquelched => "EVS: Event squelched due to high rate",
            Error::CfeEvsNotImplemented => "EVS: Not Implemented",

            // --- CFE ES (Executive Services) ---
            Error::CfeEsErrResourceIdNotValid => "ES: Resource ID is not valid",
            Error::CfeEsErrNameNotFound => "ES: Resource Name not found",
            Error::CfeEsErrAppCreate => "ES: Application Create Error",
            Error::CfeEsErrChildTaskCreate => "ES: Child Task Create Error",
            Error::CfeEsErrSysLogFull => "ES: System Log Full",
            Error::CfeEsErrMemBlockSize => "ES: Memory Block Size Error",
            Error::CfeEsErrLoadLib => "ES: Load Library Error",
            Error::CfeEsBadArgument => "ES: Bad Argument",
            Error::CfeEsErrChildTaskRegister => "ES: Child Task Register Error",
            Error::CfeEsCdsInsufficientMemory => "ES: CDS Insufficient Memory",
            Error::CfeEsCdsInvalidName => "ES: CDS Invalid Name",
            Error::CfeEsCdsInvalidSize => "ES: CDS Invalid Size",
            Error::CfeEsCdsInvalid => "ES: CDS Invalid",
            Error::CfeEsCdsAccessError => "ES: CDS Access Error",
            Error::CfeEsFileIoErr => "ES: File IO Error",
            Error::CfeEsRstAccessErr => "ES: Reset Area Access Error",
            Error::CfeEsErrAppRegister => "ES: Application Register Error",
            Error::CfeEsErrChildTaskDelete => "ES: Child Task Delete Error",
            Error::CfeEsErrChildTaskDeleteMainTask => "ES: Attempted to delete a main task",
            Error::CfeEsCdsBlockCrcErr => "ES: CDS Block CRC Error",
            Error::CfeEsMutSemDeleteErr => "ES: Mutex Semaphore Delete Error",
            Error::CfeEsBinSemDeleteErr => "ES: Binary Semaphore Delete Error",
            Error::CfeEsCountSemDeleteErr => "ES: Counting Semaphore Delete Error",
            Error::CfeEsQueueDeleteErr => "ES: Queue Delete Error",
            Error::CfeEsFileCloseErr => "ES: File Close Error",
            Error::CfeEsCdsWrongTypeErr => "ES: CDS Wrong Type Error",
            Error::CfeEsCdsOwnerActiveErr => "ES: CDS Owner Active Error",
            Error::CfeEsAppCleanupErr => "ES: Application Cleanup Error",
            Error::CfeEsTimerDeleteErr => "ES: Timer Delete Error",
            Error::CfeEsBufferNotInPool => "ES: Buffer Not In Pool",
            Error::CfeEsTaskDeleteErr => "ES: Task Delete Error",
            Error::CfeEsOperationTimedOut => "ES: Operation Timed Out",
            Error::CfeEsNoResourceIdsAvailable => "ES: No Resource IDs Available",
            Error::CfeEsPoolBlockInvalid => "ES: Invalid pool block",
            Error::CfeEsErrDuplicateName => "ES: Duplicate Name Error",
            Error::CfeEsNotImplemented => "ES: Not Implemented",

            // --- CFE FS (File Services) ---
            Error::CfeFsBadArgument => "FS: Bad Argument",
            Error::CfeFsInvalidPath => "FS: Invalid Path",
            Error::CfeFsFnameTooLong => "FS: Filename Too Long",
            Error::CfeFsNotImplemented => "FS: Not Implemented",

            // --- CFE SB (Software Bus) ---
            Error::CfeSbTimeOut => "SB: Time Out",
            Error::CfeSbNoMessage => "SB: No Message",
            Error::CfeSbBadArgument => "SB: Bad Argument",
            Error::CfeSbMaxPipesMet => "SB: Max Pipes Met",
            Error::CfeSbPipeCrErr => "SB: Pipe Create Error",
            Error::CfeSbPipeRdErr => "SB: Pipe Read Error",
            Error::CfeSbMsgTooBig => "SB: Message Too Big",
            Error::CfeSbBufAlocErr => "SB: Buffer Allocation Error",
            Error::CfeSbMaxMsgsMet => "SB: Max Messages Met",
            Error::CfeSbMaxDestsMet => "SB: Max Destinations Met",
            Error::SbInternalErr => "SB: Internal Error",
            Error::SbWrongMsgType => "SB: Wrong Message Type",
            Error::SbBufferInvalid => "SB: Buffer Invalid",
            Error::SbNotImplemented => "SB: Not Implemented",

            // --- CFE TBL (Table Services) ---
            Error::CfeTblErrInvalidHandle => "TBL: Invalid Handle",
            Error::CfeTblErrInvalidName => "TBL: Invalid Name",
            Error::CfeTblErrInvalidSize => "TBL: Invalid Size",
            Error::CfeTblErrNeverLoaded => "TBL: Never Loaded",
            Error::CfeTblErrRegistryFull => "TBL: Registry Full",
            Error::CfeTblErrNoAccess => "TBL: No Access",
            Error::CfeTblErrUnregistered => "TBL: Unregistered",
            Error::CfeTblErrHandlesFull => "TBL: Handles Full",
            Error::CfeTblErrDuplicateDiffSize => "TBL: Duplicate Table With Different Size",
            Error::CfeTblErrDuplicateNotOwned => "TBL: Duplicate Table And Not Owned",
            Error::CfeTblErrNoBufferAvail => "TBL: No Buffer Available",
            Error::CfeTblErrDumpOnly => "TBL: Dump Only Error",
            Error::CfeTblErrIllegalSrcType => "TBL: Illegal Source Type",
            Error::CfeTblErrLoadInProgress => "TBL: Load In Progress",
            Error::CfeTblErrFileTooLarge => "TBL: File Too Large",
            Error::CfeTblErrBadContentId => "TBL: Bad Content ID",
            Error::CfeTblErrBadSubtypeId => "TBL: Bad Subtype ID",
            Error::CfeTblErrFileSizeInconsistent => "TBL: File Size Inconsistent",
            Error::CfeTblErrNoStdHeader => "TBL: No Standard Header",
            Error::CfeTblErrNoTblHeader => "TBL: No Table Header",
            Error::CfeTblErrFilenameTooLong => "TBL: Filename Too Long",
            Error::CfeTblErrFileForWrongTable => "TBL: File For Wrong Table",
            Error::CfeTblErrLoadIncomplete => "TBL: Load Incomplete",
            Error::CfeTblErrPartialLoad => "TBL: Partial Load Error",
            Error::CfeTblErrInvalidOptions => "TBL: Invalid Options",
            Error::CfeTblErrBadSpacecraftId => "TBL: Bad Spacecraft ID",
            Error::CfeTblErrBadProcessorId => "TBL: Bad Processor ID",
            Error::CfeTblMessageError => "TBL: Message Error",
            Error::CfeTblErrShortFile => "TBL: Short File",
            Error::CfeTblErrAccess => "TBL: Access error",
            Error::CfeTblBadArgument => "TBL: Bad Argument",
            Error::CfeTblNotImplemented => "TBL: Not Implemented",

            // --- CFE TIME (Time Services) ---
            Error::CfeTimeNotImplemented => "TIME: Not Implemented",
            Error::CfeTimeInternalOnly => "TIME: Internal Only",
            Error::CfeTimeOutOfRange => "TIME: Out Of Range",
            Error::CfeTimeTooManySynchCallbacks => "TIME: Too Many Sync Callbacks",
            Error::CfeTimeCallbackNotRegistered => "TIME: Callback Not Registered",
            Error::CfeTimeBadArgument => "TIME: Bad Argument",

            // --- OSAL Status Codes ---
            Error::OsError => "OSAL: Generic error",
            Error::OsInvalidPointer => "OSAL: Invalid pointer",
            Error::OsErrorAddressMisaligned => "OSAL: Address misalignment",
            Error::OsErrorTimeout => "OSAL: Timeout",
            Error::OsInvalidIntNum => "OSAL: Invalid Interrupt number",
            Error::OsSemFailure => "OSAL: Semaphore failure",
            Error::OsSemTimeout => "OSAL: Semaphore timeout",
            Error::OsQueueEmpty => "OSAL: Queue empty",
            Error::OsQueueFull => "OSAL: Queue full",
            Error::OsQueueTimeout => "OSAL: Queue timeout",
            Error::OsQueueInvalidSize => "OSAL: Queue invalid size",
            Error::OsQueueIdError => "OSAL: Queue ID error",
            Error::OsErrNameTooLong => "OSAL: Name length too long",
            Error::OsErrNoFreeIds => "OSAL: No free IDs",
            Error::OsErrNameTaken => "OSAL: Name taken",
            Error::OsErrInvalidId => "OSAL: Invalid ID",
            Error::OsErrNameNotFound => "OSAL: Name not found",
            Error::OsErrSemNotFull => "OSAL: Semaphore not full",
            Error::OsErrInvalidPriority => "OSAL: Invalid priority",
            Error::OsInvalidSemValue => "OSAL: Invalid semaphore value",
            Error::OsErrFile => "OSAL: File error",
            Error::OsErrNotImplemented => "OSAL: Not implemented",
            Error::OsTimerErrInvalidArgs => "OSAL: Timer invalid arguments",
            Error::OsTimerErrTimerId => "OSAL: Timer ID error",
            Error::OsTimerErrUnavailable => "OSAL: Timer unavailable",
            Error::OsTimerErrInternal => "OSAL: Timer internal error",
            Error::OsErrObjectInUse => "OSAL: Object in use",
            Error::OsErrBadAddress => "OSAL: Bad address",
            Error::OsErrIncorrectObjState => "OSAL: Incorrect object state",
            Error::OsErrIncorrectObjType => "OSAL: Incorrect object type",
            Error::OsErrStreamDisconnected => "OSAL: Stream disconnected",
            Error::OsErrOperationNotSupported => "OSAL: Operation not supported",
            Error::OsErrInvalidSize => "OSAL: Invalid Size",
            Error::OsErrOutputTooLarge => "OSAL: Size of output exceeds limit",
            Error::OsErrInvalidArgument => "OSAL: Invalid argument value",
            Error::OsFsErrPathTooLong => "OSAL: FS path too long",
            Error::OsFsErrNameTooLong => "OSAL: FS name too long",
            Error::OsFsErrDriveNotCreated => "OSAL: FS drive not created",
            Error::OsFsErrDeviceNotFree => "OSAL: FS device not free",
            Error::OsFsErrPathInvalid => "OSAL: FS path invalid",

            // --- Other Errors ---
            Error::InvalidString => "String contains interior null character",
            Error::Unhandled(_) => "Unhandled CFE/OSAL error code: 0x{:08x}",
        };

        if !matches!(self, Error::InvalidString | Error::Unhandled(_)) {
            write!(f, "{}", desc)
        } else {
            Ok(())
        }
    }
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
            ffi::CFE_SB_INTERNAL_ERR => Error::SbInternalErr,
            ffi::CFE_SB_WRONG_MSG_TYPE => Error::SbWrongMsgType,
            ffi::CFE_SB_BUFFER_INVALID => Error::SbBufferInvalid,
            ffi::CFE_SB_NOT_IMPLEMENTED => Error::SbNotImplemented,

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

/// Retrieves the symbolic name for an OSAL status code.
pub fn get_error_name(error_num: i32) -> Result<CString<{ ffi::OS_ERROR_NAME_LENGTH as usize }>> {
    let mut name_buf = [0 as libc::c_char; ffi::OS_ERROR_NAME_LENGTH as usize];
    check(unsafe { ffi::OS_GetErrorName(error_num, &mut name_buf) })?;

    // The result from OS_GetErrorName is null-terminated.
    let c_str = unsafe { core::ffi::CStr::from_ptr(name_buf.as_ptr()) };
    let mut s = CString::new();
    s.extend_from_bytes(c_str.to_bytes())
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}

/// Retrieves the symbolic name for a CFE status code.
pub fn get_cfe_status_name(
    status_code: i32,
) -> Result<CString<{ ffi::CFE_STATUS_STRING_LENGTH as usize }>> {
    let mut name_buf = [0 as libc::c_char; ffi::CFE_STATUS_STRING_LENGTH as usize];
    unsafe { ffi::CFE_ES_StatusToString(status_code, &mut name_buf) };

    let c_str = unsafe { core::ffi::CStr::from_ptr(name_buf.as_ptr()) };
    let mut s = CString::new();
    s.extend_from_bytes(c_str.to_bytes())
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}

/// Converts an OSAL status code to its decimal or hex string representation.
pub fn osal_status_to_string(
    status_code: i32,
) -> Result<CString<{ ffi::OS_STATUS_STRING_LENGTH as usize }>> {
    let mut name_buf = [0 as libc::c_char; ffi::OS_STATUS_STRING_LENGTH as usize];
    unsafe { ffi::OS_StatusToString(status_code, &mut name_buf) };

    let c_str = unsafe { core::ffi::CStr::from_ptr(name_buf.as_ptr()) };
    let mut s = CString::new();
    s.extend_from_bytes(c_str.to_bytes())
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}
