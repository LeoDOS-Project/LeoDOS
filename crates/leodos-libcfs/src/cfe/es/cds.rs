//! Safe, idiomatic wrappers for the CFE Critical Data Store (CDS) API.
//!
//! This module provides a generic, type-safe `CdsBlock<T>` for persisting
//! application data across resets.

use crate::error::{CfsError, EsError, OsalError};
use crate::error::Result;
use crate::ffi;
use crate::cstring;
use crate::status;
use crate::status::check;
use core::marker::PhantomData;
use core::mem;
use core::mem::MaybeUninit;
use heapless::String;

/// A type-safe, zero-cost wrapper for a cFE Critical Data Store handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct CdsHandle(pub ffi::CFE_ES_CDSHandle_t);

impl CdsHandle {
    /// Retrieves the full name ("AppName.CDSName") for this CDS handle.
    pub fn name(&self) -> Result<String<{ ffi::CFE_MISSION_ES_CDS_MAX_FULL_NAME_LEN as usize }>> {
        let mut buffer = [0u8; ffi::CFE_MISSION_ES_CDS_MAX_FULL_NAME_LEN as usize];
        check(unsafe {
            ffi::CFE_ES_GetCDSBlockName(
                buffer.as_mut_ptr() as *mut libc::c_char,
                self.0,
                buffer.len(),
            )
        })?;
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        let str = String::from_utf8(vec).map_err(|_| CfsError::Osal(OsalError::NameTooLong))?;
        Ok(str)
    }
}

/// Information about the status of a CDS block upon registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CdsInfo {
    /// The CDS block was newly created and its contents are uninitialized.
    Created,
    /// The CDS block already existed and its contents have been restored.
    Restored,
}

/// A handle to a block of data in the cFE Critical Data Store.
///
/// This struct is generic over the data type `T` to be stored. The underlying
/// CDS block is registered when `new` is called and persists for the life of the
/// cFE instance (across resets).
///
/// The type `T` must be `Copy` and `Sized`, as the underlying CFE API performs
/// a raw byte copy of the data.
#[derive(Debug)]
pub struct CdsBlock<T: Copy + Sized> {
    handle: ffi::CFE_ES_CDSHandle_t,
    _phantom: PhantomData<T>,
}

impl<T: Copy + Sized> CdsBlock<T> {
    /// Registers a new CDS block with the given name or retrieves an existing one.
    ///
    /// This function will attempt to create a CDS block of `size_of::<T>()`.
    /// The block contents are NOT cleared or initialized on creation.
    ///
    /// If a block with this name already existed but with a different size,
    /// it is replaced. The new block contains uninitialized data and
    /// `CdsInfo::Created` is returned (not `Restored`).
    ///
    /// # Return Value
    ///
    /// On success, returns `Ok((CdsBlock, CdsInfo))`. The `CdsInfo` indicates
    /// whether the block was newly created or was restored from a previous run.
    ///
    /// - If `CdsInfo::Created`, the application is responsible for initializing
    ///   the data by calling `store()`.
    /// - If `CdsInfo::Restored`, the application can immediately call `restore()`
    ///   to retrieve the previous state.
    ///
    /// # Arguments
    /// * `name`: A unique, application-local name for the CDS block.
    pub fn new(name: &str) -> Result<(Self, CdsInfo)> {
        let c_name = cstring::<{ ffi::CFE_MISSION_ES_CDS_MAX_NAME_LENGTH as usize }>(name)
            .map_err(|_| CfsError::Es(EsError::CdsInvalidName))?;

        let mut handle = MaybeUninit::uninit();
        let status = unsafe {
            ffi::CFE_ES_RegisterCDS(handle.as_mut_ptr(), mem::size_of::<T>(), c_name.as_ptr())
        };

        // check() handles true errors. We need to handle the special success cases.
        match check(status) {
            Ok(status::Status::EsCdsAlreadyExists) => Ok((
                Self {
                    handle: unsafe { handle.assume_init() },
                    _phantom: PhantomData,
                },
                CdsInfo::Restored,
            )),
            Ok(_) => {
                // Any other Ok status, including Status::Success, means it was created.
                Ok((
                    Self {
                        handle: unsafe { handle.assume_init() },
                        _phantom: PhantomData,
                    },
                    CdsInfo::Created,
                ))
            }
            Err(e) => Err(e),
        }
    }

    /// Stores a copy of `data` into the CDS block.
    ///
    /// This should be called after `new` reports `CdsInfo::Created`, or any
    /// time the application wishes to update the persistent state.
    pub fn store(&self, data: &T) -> Result<()> {
        check(unsafe { ffi::CFE_ES_CopyToCDS(self.handle, data as *const T as *const _) })?;
        Ok(())
    }

    /// Restores the contents of the CDS block into a new instance of `T`.
    ///
    /// This will perform a raw byte copy from the CDS into the returned struct.
    /// It is safe because `T` is constrained to be `Copy`.
    ///
    /// Returns `Err` if the data in the CDS has been corrupted
    /// (e.g. CRC mismatch). The corrupted data is not returned.
    pub fn restore(&self) -> Result<T> {
        let mut data = MaybeUninit::<T>::uninit();
        let status =
            unsafe { ffi::CFE_ES_RestoreFromCDS(data.as_mut_ptr() as *mut _, self.handle) };
        check(status)?;
        Ok(unsafe { data.assume_init() })
    }

    /// Returns the underlying CDS handle.
    pub fn handle(&self) -> CdsHandle {
        CdsHandle(self.handle)
    }

    /// Finds an existing CDS Block ID by its full name ("AppName.CDSName").
    pub fn get_id_by_name(name: &str) -> Result<CdsHandle> {
        let c_name = cstring::<{ ffi::CFE_MISSION_ES_CDS_MAX_FULL_NAME_LEN as usize }>(name)?;

        let mut handle = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetCDSBlockIDByName(handle.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(CdsHandle(unsafe { handle.assume_init() }))
    }
}
