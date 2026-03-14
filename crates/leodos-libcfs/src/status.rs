//! CFE informational status code handling.
//!
//! While error conditions are represented by the `Error` enum, cFE APIs can also
//! return a variety of non-error "informational" status codes. This module
//! provides the `Status` enum to represent these successful-but-noteworthy
//! outcomes, and a `check` function to triage a raw `CFE_Status_t` into either
//! a `Result<Status, Error>`.

use crate::error::CfsError;
use crate::ffi;

/// Represents non-error, informational status codes from cFE APIs.
pub enum Status {
    /// Command was processed successfully.
    Success,

    // --- Informational Status Codes ---
    /// Command was processed successfully, but command counter should not be incremented.
    StatusNoCounterIncrement,
    /// The application is receiving a pointer to a CDS that was already present.
    EsCdsAlreadyExists,
    /// CFE_ES_LoadLibrary detected that the requested library name is already loaded.
    EsLibAlreadyLoaded,
    /// The last syslog message was truncated.
    EsErrSysLogTruncated,
    /// The table has a load pending.
    TblInfoUpdatePending,
    /// A registration is trying to replace an existing table with the same name.
    TblWarnDuplicate,
    /// The table has been updated since the last time the address was obtained.
    TblInfoUpdated,
    /// A table file contained less data than the size of the table.
    TblWarnShortFile,
    /// An attempt was made to update a table without a pending load.
    TblInfoNoUpdatePending,
    /// An attempt was made to update a table locked by another user.
    TblInfoTableLocked,
    /// The application should call CFE_TBL_Validate for the specified table.
    TblInfoValidationPending,
    /// An attempt was made to validate a table that did not have a validation request pending.
    TblInfoNoValidationPending,
    /// A table file load did not start with the first byte.
    TblWarnPartialLoad,
    /// A dump of the Dump-Only table has been requested.
    TblInfoDumpPending,
    /// A table registered as "Critical" failed to create a CDS.
    TblWarnNotCritical,
    /// A critical table's contents were recovered from the CDS.
    TblInfoRecoveredTbl,
}

impl core::fmt::Display for Status {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let description = match self {
            Status::Success => "Success",
            // --- Informational Status Codes ---
            Status::StatusNoCounterIncrement => "Info: No Counter Increment",
            Status::EsCdsAlreadyExists => "Info ES: CDS Already Exists",
            Status::EsLibAlreadyLoaded => "Info ES: Library Already Loaded",
            Status::EsErrSysLogTruncated => "Info ES: System Log Message Truncated",
            Status::TblInfoUpdatePending => "Info TBL: Update Pending",
            Status::TblWarnDuplicate => "Warning TBL: Duplicate Table",
            Status::TblInfoUpdated => "Info TBL: Table Updated",
            Status::TblWarnShortFile => "Warning TBL: Short File",
            Status::TblInfoNoUpdatePending => "Info TBL: No Update Pending",
            Status::TblInfoTableLocked => "Info TBL: Table Locked",
            Status::TblInfoValidationPending => "Info TBL: Validation Pending",
            Status::TblInfoNoValidationPending => "Info TBL: No Validation Pending",
            Status::TblWarnPartialLoad => "Warning TBL: Partial Load",
            Status::TblInfoDumpPending => "Info TBL: Dump Pending",
            Status::TblWarnNotCritical => "Warning TBL: Table Not Critical",
            Status::TblInfoRecoveredTbl => "Info TBL: Recovered Table",
        };
        write!(f, "{}", description)
    }
}
/// Converts a raw CFE status code into a `Result<Status, Error>` for idiomatic error handling.
pub fn check(code: ffi::CFE_Status_t) -> Result<Status, CfsError> {
    Status::try_from(code)
}

impl TryFrom<ffi::CFE_Status_t> for Status {
    type Error = CfsError;
    fn try_from(status: ffi::CFE_Status_t) -> Result<Self, Self::Error> {
        let ok = match status {
            ffi::CFE_SUCCESS => Status::Success,
            ffi::CFE_STATUS_NO_COUNTER_INCREMENT => Status::StatusNoCounterIncrement,
            ffi::CFE_ES_CDS_ALREADY_EXISTS => Status::EsCdsAlreadyExists,
            ffi::CFE_ES_LIB_ALREADY_LOADED => Status::EsLibAlreadyLoaded,
            ffi::CFE_ES_ERR_SYS_LOG_TRUNCATED => Status::EsErrSysLogTruncated,
            ffi::CFE_TBL_INFO_UPDATE_PENDING => Status::TblInfoUpdatePending,
            ffi::CFE_TBL_WARN_DUPLICATE => Status::TblWarnDuplicate,
            ffi::CFE_TBL_INFO_UPDATED => Status::TblInfoUpdated,
            ffi::CFE_TBL_WARN_SHORT_FILE => Status::TblWarnShortFile,
            ffi::CFE_TBL_INFO_NO_UPDATE_PENDING => Status::TblInfoNoUpdatePending,
            ffi::CFE_TBL_INFO_TABLE_LOCKED => Status::TblInfoTableLocked,
            ffi::CFE_TBL_INFO_VALIDATION_PENDING => Status::TblInfoValidationPending,
            ffi::CFE_TBL_INFO_NO_VALIDATION_PENDING => Status::TblInfoNoValidationPending,
            ffi::CFE_TBL_WARN_PARTIAL_LOAD => Status::TblWarnPartialLoad,
            ffi::CFE_TBL_INFO_DUMP_PENDING => Status::TblInfoDumpPending,
            ffi::CFE_TBL_WARN_NOT_CRITICAL => Status::TblWarnNotCritical,
            ffi::CFE_TBL_INFO_RECOVERED_TBL => Status::TblInfoRecoveredTbl,

            other => return Err(CfsError::from(other)),
        };
        Ok(ok)
    }
}
