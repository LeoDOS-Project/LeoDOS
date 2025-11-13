//! Application management.
//!
//! This module provides the `App` struct as a handle to cFS services and the
//! `AppMain` trait to define application behavior.

use core::ffi::CStr;
use core::fmt::Write;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::str;

use heapless::CString;
use heapless::String;

use crate::error::Error;
use crate::error::Result;
use crate::ffi;
use crate::log;
use crate::log::syslog;
use crate::status::check;

/// Represents the possible run statuses returned by `CFE_ES_RunLoop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RunStatus {
    /// The application run status is undefined.
    Undefined = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_UNDEFINED,
    /// The application should continue running.
    Run = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_APP_RUN,
    /// The application should exit gracefully.
    Exit = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_APP_EXIT,
    /// An error occurred; the application should handle it appropriately.
    Error = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_APP_ERROR,
    /// The application encountered a system exception.
    Exception = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_SYS_EXCEPTION,
    /// The application should be restarted by the system.
    Restart = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_SYS_RESTART,
    /// The application should be reloaded by the system.
    Reload = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_SYS_RELOAD,
    /// The application should be deleted by the system.
    Delete = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_SYS_DELETE,
    /// The core application failed to initialize.
    CoreAppInitError = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_CORE_APP_INIT_ERROR,
    /// The core application encountered a runtime error.
    CoreAppRuntimeError = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_CORE_APP_RUNTIME_ERROR,
}

impl From<u32> for RunStatus {
    fn from(value: u32) -> Self {
        match value {
            x if x == RunStatus::Undefined as u32 => RunStatus::Undefined,
            x if x == RunStatus::Run as u32 => RunStatus::Run,
            x if x == RunStatus::Exit as u32 => RunStatus::Exit,
            x if x == RunStatus::Error as u32 => RunStatus::Error,
            x if x == RunStatus::Exception as u32 => RunStatus::Exception,
            x if x == RunStatus::Restart as u32 => RunStatus::Restart,
            x if x == RunStatus::Reload as u32 => RunStatus::Reload,
            x if x == RunStatus::Delete as u32 => RunStatus::Delete,
            x if x == RunStatus::CoreAppInitError as u32 => RunStatus::CoreAppInitError,
            x if x == RunStatus::CoreAppRuntimeError as u32 => RunStatus::CoreAppRuntimeError,
            _ => RunStatus::Undefined, // Default case
        }
    }
}

/// A type-safe, zero-cost wrapper for a cFE Application ID.
///
/// This is a lightweight, `Copy`-able handle that represents a unique application.
/// It can be used to query information about that specific application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct AppId(pub(crate) ffi::CFE_ES_AppId_t);

impl AppId {
    /// Retrieves detailed information about the application with this ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the App ID is not valid or if the underlying
    /// CFE call fails.
    pub fn info(&self) -> Result<AppInfo> {
        let mut app_info_uninit = MaybeUninit::uninit();
        let status = unsafe { ffi::CFE_ES_GetAppInfo(app_info_uninit.as_mut_ptr(), self.0) };
        check(status)?;
        Ok(AppInfo {
            inner: unsafe { app_info_uninit.assume_init() },
        })
    }

    /// Retrieves the name for this application ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the App ID is not valid, the buffer is too small,
    /// or the name is not valid UTF-8.
    pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
        let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
        let status = unsafe {
            ffi::CFE_ES_GetAppName(
                buffer.as_mut_ptr() as *mut libc::c_char,
                self.0,
                buffer.len(),
            )
        };
        check(status)?;
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| Error::OsErrNameTooLong)?;
        String::from_utf8(vec).map_err(|_| Error::InvalidString)
    }

    /// Converts the App ID into a zero-based integer suitable for array indexing.
    ///
    /// # Errors
    ///
    /// Returns an error if the App ID is not valid or if the underlying
    /// CFE call fails.
    pub fn to_index(&self) -> Result<u32> {
        let mut index = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_AppID_ToIndex(self.0, index.as_mut_ptr()) })?;
        Ok(unsafe { index.assume_init() })
    }

    /// Requests cFE to restart this application.
    ///
    /// # Errors
    ///
    /// Returns an error if the `app_id` is invalid or if the restart command fails.
    pub fn restart(&self) -> Result<()> {
        check(unsafe { ffi::CFE_ES_RestartApp(self.0) })?;
        Ok(())
    }

    /// Requests cFE to reload this application from a new file.
    ///
    /// # Arguments
    /// * `filename`: The path to the new application binary file.
    ///
    /// # Errors
    ///
    /// Returns an error if the `app_id` is invalid, the filename is invalid,
    /// the file cannot be accessed, or the reload command fails.
    pub fn reload(&self, filename: &str) -> Result<()> {
        let mut c_filename = CString::<{ ffi::OS_MAX_PATH_LEN as usize }>::new();
        c_filename
            .extend_from_bytes(filename.as_bytes())
            .map_err(|_| Error::OsFsErrPathTooLong)?;
        check(unsafe { ffi::CFE_ES_ReloadApp(self.0, c_filename.as_ptr()) })?;
        Ok(())
    }

    /// Requests cFE to delete this application.
    ///
    /// # Errors
    ///
    /// Returns an error if the `app_id` is invalid or if the delete command fails.
    pub fn delete(&self) -> Result<()> {
        check(unsafe { ffi::CFE_ES_DeleteApp(self.0) })?;
        Ok(())
    }

    /// Retrieves the cFE Application ID for a given application name.
    ///
    /// # Arguments
    /// * `name`: The registered name of the application to look up.
    ///
    /// # Errors
    ///
    /// Returns an error if no application with the given name is found, or if the
    /// name is too long for the internal CFE buffers.
    pub fn from_name(name: &str) -> Result<AppId> {
        let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| Error::OsErrNameTooLong)?;

        let mut app_id = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_ES_GetAppIDByName(app_id.as_mut_ptr(), c_name.as_ptr()) })?;
        Ok(AppId(unsafe { app_id.assume_init() }))
    }
}

impl Deref for AppId {
    type Target = ffi::CFE_ES_AppId_t;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A high-level wrapper around the FFI's `CFE_ES_AppInfo_t`.
///
/// This struct contains detailed information about a cFE application, such as its
/// name, entry point, memory layout, and task IDs.
#[derive(Debug, Clone)]
pub struct AppInfo {
    /// The underlying FFI `CFE_ES_AppInfo_t` struct.
    pub(crate) inner: ffi::CFE_ES_AppInfo_t,
}

impl AppInfo {
    /// Returns the registered name of the application.
    ///
    /// # Errors
    ///
    /// Returns an error if the name from the underlying FFI struct is not
    /// valid UTF-8 or cannot fit into the `CString` buffer.
    pub fn name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
        let c_str = unsafe { CStr::from_ptr(self.inner.Name.as_ptr()) };
        let bytes = c_str.to_bytes();
        let mut s = CString::new();
        s.extend_from_bytes(bytes)
            .map_err(|_| Error::OsErrNameTooLong)?;
        Ok(s)
    }

    /// Copies the entry point name of the application into the provided buffer.
    /// Returns a &str slice of the valid UTF-8 part of the buffer.
    ///
    /// # Arguments
    /// * `buffer`: A mutable byte slice to copy the name into.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is too small or the name is not valid UTF-8.
    pub fn copy_entry_point<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a str> {
        let c_str = unsafe { CStr::from_ptr(self.inner.EntryPoint.as_ptr()) };
        let bytes = c_str.to_bytes();
        if bytes.len() >= buffer.len() {
            return Err(Error::OsErrInvalidSize);
        }
        buffer[..bytes.len()].copy_from_slice(bytes);
        buffer[bytes.len()] = 0;
        str::from_utf8(&buffer[..bytes.len()]).map_err(|_| Error::InvalidString)
    }

    /// Copies the file name of the application into the provided buffer.
    /// Returns a &str slice of the valid UTF-8 part of the buffer.
    ///
    /// # Arguments
    /// * `buffer`: A mutable byte slice to copy the name into.
    ///
    /// # Errors
    ///
    /// Returns an error if the buffer is too small or the name is not valid UTF-8.
    pub fn copy_file_name<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a str> {
        let c_str = unsafe { CStr::from_ptr(self.inner.FileName.as_ptr()) };
        let bytes = c_str.to_bytes();
        if bytes.len() >= buffer.len() {
            return Err(Error::OsErrInvalidSize);
        }
        buffer[..bytes.len()].copy_from_slice(bytes);
        buffer[bytes.len()] = 0;
        str::from_utf8(&buffer[..bytes.len()]).map_err(|_| Error::InvalidString)
    }
}

/// Defines the behavior of a cFS application.
///
/// Your application's state struct should implement this trait.
pub trait AppMain: Sized {
    /// The initialization routine for the application.
    ///
    /// # Errors
    ///
    /// This function should return an error if initialization fails. The error will be
    /// logged to the system log, and the application will exit.
    /// This function is called once at startup. It should perform all necessary
    /// cFS resource registration (EVS, SB pipes, tables, etc.).
    ///
    /// On success, it returns `Ok(Self)`, creating the initial state of your application.
    fn init() -> Result<Self>;

    /// The main processing loop for the application.
    ///
    /// This function is called once per cFS scheduler cycle.
    ///
    /// # Errors
    ///
    /// If this function returns an error, the main application loop will terminate,
    /// and the application will exit.
    /// primary logic of your application, such as reading from a software bus pipe.
    fn run_cycle(&mut self) -> Result<()>;
}

/// A context handle for the *currently running* cFS application.
///
/// This struct provides safe access to cFS services that are contextual to the
/// calling application. An instance is passed to your `AppMain` implementation.
#[derive(Debug)]
pub struct App {
    app_id: AppId,
}

impl App {
    /// Retrieves a handle to the context of the currently running application.
    ///
    /// This is the primary entry point for acquiring an `App` handle at startup.
    ///
    /// # Errors
    ///
    /// Returns an error if called from a context that is not a registered cFE
    /// application task.
    pub fn this() -> Result<Self> {
        let mut app_id = MaybeUninit::uninit();
        let status = unsafe { ffi::CFE_ES_GetAppID(app_id.as_mut_ptr()) };
        check(status)?;
        Ok(App {
            app_id: AppId(unsafe { app_id.assume_init() }),
        })
    }

    /// Returns the application's unique cFE ID.
    pub fn id(&self) -> AppId {
        self.app_id
    }
}

/// The main entry point and lifecycle manager for a cFS application.
///
/// This function is typically not called directly. Use the `libcfs::main!` macro instead.
///
/// It performs the following steps:
/// 1. Acquires the application context using `App::this()`.
/// 2. Calls the `AppMain::init` function to initialize the application state.
/// 3. Enters the main processing loop, repeatedly calling `AppMain::run_cycle`.
/// 4. Exits gracefully when `run_cycle` returns an error or when cFE commands an exit.
pub fn start<T: AppMain>() {
    let Ok(mut state) = T::init() else {
        syslog("Application initialization failed.").ok();
        unsafe { ffi::CFE_ES_ExitApp(RunStatus::Error as u32) };
        return;
    };

    loop {
        let mut status = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_APP_RUN;
        let should_run = unsafe { ffi::CFE_ES_RunLoop(&mut status) };
        match RunStatus::from(status) {
            RunStatus::Run if should_run => {
                if let Err(err) = state.run_cycle() {
                    log!("Application error: {:?}", err).ok();
                    break;
                }
            }
            RunStatus::Exit | RunStatus::Delete => {
                log!("Received exit/delete command, exiting application.").ok();
                break;
            }
            other_status => {
                log!("Exiting application due to run status: {:?}", other_status).ok();
                break;
            }
        }
    }

    unsafe { ffi::CFE_ES_ExitApp(RunStatus::Exit as u32) };
}

/// A macro to define the entry point for a cFS application.
///
/// This macro generates the required `CFE_ES_Main` C function, which serves as the
/// official entry point for a cFE application. It links this entry point to the
/// safe Rust application lifecycle managed by `leodos-libcfs::cfe::es::app::start`.
///
/// # Example
///
/// ```rust,ignore
/// use leodos_libcfs::cfe::es::app::{App, AppMain};
/// use leodos_libcfs::error::Result;
/// // Define the state for your application.
/// struct MyAppState { /* ... */ }
///
/// impl AppMain for MyAppState {
///     fn init(app: &mut App) -> Result<Self> {
///         // ...
///         Ok(Self { /* ... */ })
///     }
///
///     fn run_cycle(&mut self, app: &App) -> Result<()> {
///         // ...
///         Ok(())
///     }
/// }
///
/// leodos_libcfs::main!(MyAppState);
/// ```
#[macro_export]
macro_rules! main {
    ($app_main_struct:ty) => {
        #[no_mangle]
        pub extern "C" fn CFE_ES_Main() {
            $crate::cfe::es::app::start::<$app_main_struct>();
        }
    };
}

/// Provides a default panic handler that logs the panic to the cFE System Log
/// and exits the application. This is highly recommended for all applications.
///
/// To use this, add the following to your application's `main.rs` or `lib.rs`:
///
/// ```rust,ignore
/// #[panic_handler]
/// fn panic(info: &core::panic::PanicInfo) -> ! { // This signature is required.
///     libcfs::es::app::default_panic_handler(info);
/// }
/// ```
pub fn default_panic_handler(info: &core::panic::PanicInfo) -> ! {
    let mut message: String<256> = String::new();

    write!(message, "PANIC: ").ok();
    if let Some(location) = info.location() {
        write!(message, " at {}:{}", location.file(), location.line()).ok();
    }

    let _ = syslog(&message);

    unsafe { ffi::CFE_ES_ExitApp(RunStatus::Error as u32) };

    loop {}
}
