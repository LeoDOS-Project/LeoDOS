//! Safe wrappers for generic CFE Resource ID functions.

use crate::cfe::es::app::AppId;
use crate::cfe::es::app::AppInfo;
use crate::cfe::es::lib::LibId;
use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// A generic, type-safe wrapper for a CFE Resource ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ResourceId(pub ffi::CFE_ResourceId_t);

impl ResourceId {
    /// Gets the base value (type/category) from this resource ID.
    pub fn base(&self) -> u32 {
        unsafe { ffi::CFE_ResourceId_GetBase(self.0) }
    }

    /// Gets the serial number from this resource ID.
    pub fn serial(&self) -> u32 {
        unsafe { ffi::CFE_ResourceId_GetSerial(self.0) }
    }

    /// Retrieves information about an Application or Library given this Resource ID.
    ///
    /// This is a generic wrapper that can be used for either an `AppId` or a `LibId`.
    pub fn module_info(&self) -> Result<AppInfo> {
        let mut module_info_uninit = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetModuleInfo(module_info_uninit.as_mut_ptr(), self.0) })?;
        Ok(AppInfo {
            inner: unsafe { module_info_uninit.assume_init() },
        })
    }
}

// ADD these From implementations
impl From<AppId> for ResourceId {
    fn from(app_id: AppId) -> Self {
        ResourceId(app_id.0)
    }
}

impl From<LibId> for ResourceId {
    fn from(lib_id: LibId) -> Self {
        ResourceId(lib_id.0)
    }
}
