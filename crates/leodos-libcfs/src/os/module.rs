//! Safe, idiomatic wrappers for OSAL dynamic module loading APIs.
//!
//! This module provides a `Module` struct that is a safe RAII handle for a
//! dynamically loaded shared library, ensuring it is properly unloaded when
//! it goes out of scope.

use crate::error::Result;
use crate::ffi;
use crate::os::util::c_name_from_str;
use crate::os::util::c_path_from_str;
use crate::os::util::path_from_c_buf;
use crate::string_from_c_buf;
use crate::status::check;
use bitflags::bitflags;
use core::ffi::c_void;
use core::mem::MaybeUninit;
use heapless::String;

/// A type-safe, zero-cost wrapper for an OSAL Module ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ModuleId(pub ffi::osal_id_t);

bitflags! {
    /// Flags that control the behavior of module loading.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct ModuleFlags: u32 {
        /// Symbols in the loaded module are added to the global symbol table.
        const GLOBAL_SYMBOLS = ffi::OS_MODULE_FLAG_GLOBAL_SYMBOLS;
        /// Symbols are kept local/private to this module.
        const LOCAL_SYMBOLS = ffi::OS_MODULE_FLAG_LOCAL_SYMBOLS;
    }
}

/// A high-level wrapper for module properties.
#[derive(Debug, Clone)]
pub struct ModuleProp {
    /// The registered logical name of the module.
    pub name: String<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The filename from which the module was loaded.
    pub filename: String<{ ffi::OS_MAX_PATH_LEN as usize }>,
    /// The entry point address of the module.
    pub entry_point: usize,
}

/// A handle to a dynamically loaded OSAL module (shared library).
///
/// This is a wrapper around an `osal_id_t` that will automatically call
/// `OS_ModuleUnload` when it goes out of scope, preventing resource leaks.
#[derive(Debug)]
pub struct Module {
    id: ModuleId,
}

impl Module {
    /// Loads a shared library object file into the running system.
    ///
    /// `GLOBAL_SYMBOLS` is the default; use `LOCAL_SYMBOLS` for
    /// safer unloading, then use [`Module::symbol`] to look up
    /// local symbols.
    ///
    /// # Arguments
    /// * `name`: A unique logical name to identify the module.
    /// * `filename`: The path to the object file to load (e.g., "/cf/my_lib.so").
    /// * `flags`: Options for how symbols are loaded.
    pub fn load(name: &str, filename: &str, flags: ModuleFlags) -> Result<Self> {
        let c_name = c_name_from_str(name)?;
        let c_filename = c_path_from_str(filename)?;
        let mut module_id = MaybeUninit::uninit();

        check(unsafe {
            ffi::OS_ModuleLoad(
                module_id.as_mut_ptr(),
                c_name.as_ptr(),
                c_filename.as_ptr(),
                flags.bits(),
            )
        })?;

        Ok(Self {
            id: ModuleId(unsafe { module_id.assume_init() }),
        })
    }

    /// Looks up the address of a symbol within this module.
    ///
    /// This is useful for finding function pointers in modules loaded with
    /// `ModuleFlags::LOCAL_SYMBOLS`.
    ///
    /// # Safety
    ///
    /// Using the returned pointer is inherently unsafe. The caller must ensure
    /// it is cast to the correct function pointer type and that the function
    /// signature is correct. The symbol's lifetime is tied to the `Module` instance.
    pub fn symbol(&self, name: &str) -> Result<*mut c_void> {
        let c_name = c_name_from_str(name)?;
        let mut symbol_addr = 0;
        check(unsafe { ffi::OS_ModuleSymbolLookup(self.id.0, &mut symbol_addr, c_name.as_ptr()) })?;
        Ok(symbol_addr as *mut c_void)
    }

    /// Returns the underlying `ModuleId`.
    pub fn id(&self) -> ModuleId {
        self.id
    }

    /// Retrieves information about this module.
    pub fn info(&self) -> Result<ModuleProp> {
        let mut prop = MaybeUninit::uninit();
        check(unsafe { ffi::OS_ModuleInfo(self.id.0, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        Ok(ModuleProp {
            name: string_from_c_buf(&prop.name)?,
            filename: path_from_c_buf(&prop.filename)?,
            entry_point: prop.entry_point,
        })
    }
}

impl Drop for Module {
    /// Unloads the module when the `Module` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_ModuleUnload(self.id.0) };
    }
}

/// Dumps the system symbol table to the specified file.
///
/// Not all RTOS support this. Returns `OS_ERR_NOT_IMPLEMENTED`
/// if not available.
///
/// # Arguments
/// * `filename`: The path to the file to write the symbol table to.
/// * `size_limit`: Maximum number of bytes to write.
pub fn symbol_table_dump(filename: &str, size_limit: usize) -> Result<()> {
    let c_filename = c_path_from_str(filename)?;
    check(unsafe { ffi::OS_SymbolTableDump(c_filename.as_ptr(), size_limit) })?;
    Ok(())
}

/// Looks up the address of a symbol in the global symbol table.
///
/// This can find symbols in the main executable or in any module loaded
/// with `ModuleFlags::GLOBAL_SYMBOLS`.
///
/// # Safety
/// Using the returned pointer is inherently unsafe. The caller must ensure
/// it is cast to the correct function pointer type and that the function
/// signature is correct. The lifetime of the symbol is not managed by this function.
pub fn symbol_lookup(name: &str) -> Result<*mut c_void> {
    let c_name = c_name_from_str(name)?;
    let mut symbol_addr = 0;
    check(unsafe { ffi::OS_SymbolLookup(&mut symbol_addr, c_name.as_ptr()) })?;
    Ok(symbol_addr as *mut c_void)
}
