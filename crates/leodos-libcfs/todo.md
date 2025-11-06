```diff
--- a/src/cfe/es/app.rs
+++ b/src/cfe/es/app.rs
@@ -21,6 +21,11 @@
     
     impl AppId {
         /// Retrieves detailed information about the application with this ID.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the App ID is not valid or if the underlying
+        /// CFE call fails.
         pub fn info(&self) -> Result<AppInfo> {
             let mut app_info_uninit = MaybeUninit::uninit();
             let status = unsafe { ffi::CFE_ES_GetAppInfo(app_info_uninit.as_mut_ptr(), self.0) };
@@ -30,6 +35,11 @@
         }
     
         /// Retrieves the name for this application ID.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the App ID is not valid, the buffer is too small
+        /// (unlikely with `heapless`), or the name is not valid UTF-8.
         pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
             let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
             let status =
@@ -40,6 +50,11 @@
         }
     
         /// Converts the App ID into a zero-based integer suitable for array indexing.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the App ID is not valid or if the underlying
+        /// CFE call fails.
         pub fn to_index(&self) -> Result<u32> {
             let mut index = MaybeUninit::uninit();
             check(unsafe { ffi::CFE_ES_AppID_ToIndex(self.0, index.as_mut_ptr()) })?;
@@ -62,6 +77,9 @@
     }
     
     /// A high-level wrapper around the FFI's `CFE_ES_AppInfo_t`.
+    ///
+    /// This struct contains detailed information about a cFE application, such as its
+    /// name, entry point, memory layout, and task IDs.
     #[derive(Debug, Clone)]
     pub struct AppInfo {
         pub inner: ffi::CFE_ES_AppInfo_t,
@@ -69,6 +87,11 @@
     
     impl AppInfo {
         /// Returns the registered name of the application.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the name from the underlying FFI struct is not
+        /// valid UTF-8 or cannot fit into the `CString` buffer.
         pub fn name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
             let c_str = unsafe { CStr::from_ptr(self.inner.Name.as_ptr()) };
             let bytes = c_str.to_bytes();
@@ -80,6 +103,13 @@
     
         /// Copies the entry point name of the application into the provided buffer.
         /// Returns a &str slice of the valid UTF-8 part of the buffer.
+        ///
+        /// # Arguments
+        /// * `buffer`: A mutable byte slice to copy the name into.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the buffer is too small or the name is not valid UTF-8.
         pub fn copy_entry_point<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a str> {
             let c_str = unsafe { CStr::from_ptr(self.inner.EntryPoint.as_ptr()) };
             let bytes = c_str.to_bytes();
@@ -92,6 +122,13 @@
     
         /// Copies the file name of the application into the provided buffer.
         /// Returns a &str slice of the valid UTF-8 part of the buffer.
+        ///
+        /// # Arguments
+        /// * `buffer`: A mutable byte slice to copy the name into.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the buffer is too small or the name is not valid UTF-8.
         pub fn copy_file_name<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a str> {
             let c_str = unsafe { CStr::from_ptr(self.inner.FileName.as_ptr()) };
             let bytes = c_str.to_bytes();
@@ -109,6 +146,13 @@
     pub trait AppMain: Sized {
         /// The initialization routine for the application.
         ///
+        /// # Arguments
+        /// * `app`: A mutable handle to the current application context.
+        ///
+        /// # Errors
+        ///
+        /// This function should return an error if initialization fails. The error will be
+        /// logged to the system log, and the application will exit.
         /// This function is called once at startup. It should perform all necessary
         /// cFS resource registration (EVS, SB pipes, tables, etc.).
         ///
@@ -118,6 +162,13 @@
         /// The main processing loop for the application.
         ///
         /// This function is called once per cFS scheduler cycle. It should contain the
+        /// # Arguments
+        /// * `app`: An immutable handle to the current application context.
+        ///
+        /// # Errors
+        ///
+        /// If this function returns an error, the main application loop will terminate,
+        /// and the application will exit.
         /// primary logic of your application, such as reading from a software bus pipe.
         fn run_cycle(&mut self, app: &App) -> Result<()>;
     }
@@ -134,6 +185,11 @@
         /// Retrieves a handle to the context of the currently running application.
         ///
         /// This is the primary entry point for acquiring an `App` handle at startup.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if called from a context that is not a registered cFE
+        /// application task.
         pub fn this() -> Result<Self> {
             let mut app_id = MaybeUninit::uninit();
             let status = unsafe { ffi::CFE_ES_GetAppID(app_id.as_mut_ptr()) };
@@ -144,6 +200,14 @@
         }
     
         /// Retrieves the cFE Application ID for a given application name.
+        ///
+        /// # Arguments
+        /// * `name`: The registered name of the application to look up.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if no application with the given name is found, or if the
+        /// name is too long for the internal CFE buffers.
         pub fn id_from_name(name: &str) -> Result<AppId> {
             let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
             c_name
@@ -156,6 +220,13 @@
         }
     
         /// Writes a message to the cFE system log.
+        ///
+        /// This is useful for logging critical events, especially during initialization
+        /// before Event Services (EVS) are available, or in error paths where EVS
+        /// might fail.
+        ///
+        /// The `syslog!` macro provides a more convenient, `println!`-like interface
+        /// for this functionality.
         pub fn syslog(message: &str) -> Result<()> {
             let mut c_string = CString::<256>::new();
             c_string
@@ -171,11 +242,20 @@
         }
     
         /// Registers the application with Event Services.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the registration fails, for example, if the application
+        /// ID is invalid or if Event Services has an internal error.
         /// This must be called before sending events.
         pub fn register_for_events(&self) -> Result<()> {
             evs::event::register()
         }
     
         /// Sends a formatted software event from this application.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the application is not registered for events or if the
+        /// underlying `CFE_EVS_SendEvent` call fails.
         pub fn send_event(
             &self,
             event_id: u16,
@@ -186,11 +266,28 @@
         }
     
         /// Creates a new software bus pipe for this application.
+        ///
+        /// # Arguments
+        /// * `name`: A unique string to identify the pipe within the application.
+        /// * `depth`: The maximum number of messages the pipe can hold.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the pipe cannot be created, for example, if the name is
+        /// too long, the maximum number of pipes has been reached, or an OS-level
+        /// queue creation fails.
         pub fn create_pipe(&self, name: &str, depth: u16) -> Result<Pipe> {
             Pipe::new(name, depth)
         }
     
         /// Registers a new table with cFE Table Services, owned by this application.
+        ///
+        //
+        /// # Errors
+        ///
+        /// Returns an error if the table cannot be registered, for reasons such as an
+        /// invalid name or size, invalid options, or if the table registry is full.
+        /// See `tbl::Table::new` for more details.
         pub fn register_table<T: Sized>(
             &self,
             name: &str,
@@ -204,6 +301,11 @@
     /// The main entry point and lifecycle manager for a cFS application.
     ///
     /// This function is typically not called directly. Use the `libcfs::main!` macro instead.
+    ///
+    /// It performs the following steps:
+    /// 1. Acquires the application context using `App::this()`.
+    /// 2. Calls the `AppMain::init` function to initialize the application state.
+    /// 3. Enters the main processing loop, repeatedly calling `AppMain::run_cycle`.
+    /// 4. Exits gracefully when `run_cycle` returns an error or when cFE commands an exit.
     pub fn start<T: AppMain>() {
         let mut app = match App::this() {
             Ok(app) => app,
@@ -248,15 +350,16 @@
     
     /// A macro to define the entry point for a cFS application.
     ///
-    /// This macro generates the required `CFE_ES_Main` C function and links it to the
-    /// `libcfs` application framework.
+    /// This macro generates the required `CFE_ES_Main` C function, which serves as the
+    /// official entry point for a cFE application. It links this entry point to the
+    /// safe Rust application lifecycle managed by `leodos-libcfs::cfe::es::app::start`.
     ///
-    /// # Usage
+    /// # Example
     ///
     /// ```rust,ignore
-    /// use libcfs::app::{App, AppMain};
-    /// use libcfs::error::Result;
-    ///
+    /// use leodos_libcfs::cfe::es::app::{App, AppMain};
+    /// use leodos_libcfs::error::Result;
+    /// // Define the state for your application.
     /// struct MyAppState { /* ... */ }
     ///
     /// impl AppMain for MyAppState {
@@ -271,7 +374,7 @@
     ///     }
     /// }
     ///
-    /// libcfs::main!(MyAppState);
+    /// leodos_libcfs::main!(MyAppState);
     /// ```
     #[macro_export]
     macro_rules! main {
@@ -291,7 +394,7 @@
     ///
     /// ```rust,ignore
     /// #[panic_handler]
-    /// fn panic(info: &core::panic::PanicInfo) -> ! {
+    /// fn panic(info: &core::panic::PanicInfo) -> ! { // This signature is required.
     ///     libcfs::es::app::default_panic_handler(info);
     /// }
     /// ```
@@ -314,6 +417,14 @@
     }
     
     /// Checks for exit, restart, or reload commands from the system.
+    ///
+    /// This is a wrapper around `CFE_ES_RunLoop`. It should be called once per
+    /// main application cycle.
+    ///
+    /// # Returns
+    ///
+    /// * `Ok(true)`: The application should continue running.
+    /// * `Ok(false)`: The application has been commanded to exit.
+    /// * `Err(e)`: An error occurred, or an unexpected run status was received.
     fn run_loop() -> Result<bool> {
         let mut run_status = ffi::CFE_ES_RunStatus_CFE_ES_RunStatus_APP_RUN;
         let should_run = unsafe { ffi::CFE_ES_RunLoop(&mut run_status) };
@@ -328,12 +439,30 @@
     }
     
     /// Requests cFE to restart another application.
+    ///
+    /// # Arguments
+    /// * `app_id`: The `AppId` of the application to restart.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the `app_id` is invalid or if the restart command fails.
     pub fn restart_app(app_id: AppId) -> Result<()> {
         check(unsafe { ffi::CFE_ES_RestartApp(app_id.0) })?;
         Ok(())
     }
     
     /// Requests cFE to reload another application from a new file.
+    ///
+    /// # Arguments
+    /// * `app_id`: The `AppId` of the application to reload.
+    /// * `filename`: The path to the new application binary file.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the `app_id` is invalid, the filename is invalid,
+    /// the file cannot be accessed, or the reload command fails.
     pub fn reload_app(app_id: AppId, filename: &str) -> Result<()> {
         let mut c_filename = CString::<{ ffi::OS_MAX_PATH_LEN as usize }>::new();
         c_filename
@@ -344,6 +473,14 @@
     }
     
     /// Requests cFE to delete another application.
+    ///
+    /// # Arguments
+    /// * `app_id`: The `AppId` of the application to delete.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the `app_id` is invalid or if the delete command fails.
     pub fn delete_app(app_id: AppId) -> Result<()> {
         check(unsafe { ffi::CFE_ES_DeleteApp(app_id.0) })?;
         Ok(())
--- a/src/cfe/es/cds.rs
+++ b/src/cfe/es/cds.rs
@@ -62,6 +62,11 @@
         /// # Arguments
         /// * `name`: A unique, application-local name for the CDS block.
         pub fn new(name: &str) -> Result<(Self, CdsInfo)> {
+            // The C string must fit within the mission-defined maximum length.
             let mut c_name = CString::<{ ffi::CFE_MISSION_ES_CDS_MAX_NAME_LENGTH as usize }>::new();
             c_name
                 .extend_from_bytes(name.as_bytes())
@@ -101,6 +106,12 @@
         ///
         /// This should be called after `new` reports `CdsInfo::Created`, or any
         /// time the application wishes to update the persistent state.
+        ///
+        /// # Arguments
+        /// * `data`: A reference to the data to be copied into the CDS.
+        ///
+        /// # Errors
+        /// Returns an error if the underlying CFE call fails (e.g., invalid handle).
         pub fn store(&self, data: &T) -> Result<()> {
             check(unsafe { ffi::CFE_ES_CopyToCDS(self.handle, data as *const T as *const _) })?;
             Ok(())
@@ -112,9 +123,12 @@
         /// It is safe because `T` is constrained to be `Copy`.
         ///
         /// Returns `Error::EsCdsBlockCrcErr` if the data in the CDS has been corrupted.
-        /// In this case, the (corrupted) data is still copied, and the application
-        /// must decide how to proceed.
+        ///
+        /// # Errors
+        /// Returns `Error::EsCdsBlockCrcErr` if the data's CRC check fails, indicating
+        /// potential corruption. Note that even in this case, the (corrupted) data is
+        /// still copied, and the application must decide how to proceed.
         pub fn restore(&self) -> Result<T> {
             let mut data = MaybeUninit::<T>::uninit();
             let status =
@@ -130,6 +144,14 @@
         }
     
         /// Finds an existing CDS Block ID by its full name ("AppName.CDSName").
+        ///
+        /// # Arguments
+        /// * `name`: The full name of the CDS block.
+        ///
+        /// # Errors
+        /// Returns an error if no CDS block with the given name is found or if the
+        /// name is too long.
         pub fn get_id_by_name(name: &str) -> Result<CdsHandle> {
             let mut c_name = CString::<{ ffi::CFE_MISSION_ES_CDS_MAX_FULL_NAME_LEN as usize }>::new();
             c_name
@@ -142,6 +164,15 @@
     }
     
     /// Retrieves the full name ("AppName.CDSName") for a given CDS handle.
+    ///
+    /// # Arguments
+    /// * `handle`: The `CdsHandle` to look up.
+    ///
+    /// # Errors
+    /// Returns an error if the handle is invalid, the name cannot fit in the
+    /// buffer (unlikely with `heapless`), or if the name is not valid UTF-8.
     pub fn get_block_name(
         handle: CdsHandle,
     ) -> Result<String<{ ffi::CFE_MISSION_ES_CDS_MAX_FULL_NAME_LEN as usize }>> {
--- a/src/cfe/es/counter.rs
+++ b/src/cfe/es/counter.rs
@@ -14,6 +14,7 @@
         id: CounterId,
     }
     
+    /// A type-safe, zero-cost wrapper for a cFE Generic Counter ID.
     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
     pub struct CounterId(ffi::CFE_ES_CounterId_t);
     
@@ -25,6 +26,10 @@
         ///
         /// # Arguments
         /// * `name`: A unique string to identify the counter.
+        ///
+        /// # Errors
+        /// Returns an error if a counter with the same name already exists, if no more
+        /// counter IDs are available, or if the name is too long.
         pub fn new(name: &str) -> Result<Self> {
             let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
             c_name
@@ -38,20 +43,44 @@
         }
     
         /// Atomically increments the counter's value by one.
+        ///
+        /// # Errors
+        /// Returns an error if the counter ID is invalid.
         pub fn inc(&self) -> Result<()> {
             check(unsafe { ffi::CFE_ES_IncrementGenCounter(self.id.0) })?;
             Ok(())
         }
     
         /// Sets the counter's value to a specific number.
+        ///
+        /// # Arguments
+        /// * `count`: The new value for the counter.
+        ///
+        /// # Errors
+        /// Returns an error if the counter ID is invalid.
         pub fn set(&self, count: u32) -> Result<()> {
             check(unsafe { ffi::CFE_ES_SetGenCount(self.id.0, count) })?;
             Ok(())
         }
     
         /// Retrieves the current value of the counter.
+        ///
+        /// # Errors
+        /// Returns an error if the counter ID is invalid.
         pub fn get(&self) -> Result<u32> {
             let mut count = 0;
             check(unsafe { ffi::CFE_ES_GetGenCount(self.id.0, &mut count) })?;
             Ok(count)
         }
     
         /// Returns the underlying cFE ID of the counter.
         pub fn id(&self) -> CounterId {
             self.id
         }
     
         /// Gets the cFE ID for a generic counter by its registered name.
+        ///
+        /// # Errors
+        /// Returns an error if no counter with the given name is found or if the
+        /// name is too long.
         pub fn get_id_by_name(name: &str) -> Result<CounterId> {
             let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
             c_name
@@ -74,6 +103,12 @@
     }
     
     /// Gets the cFE registered name for a generic counter by its ID.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the counter ID is invalid, the buffer is too small
+    /// (unlikely with `heapless`), or the name is not valid UTF-8.
     pub fn get_name_by_id(id: CounterId) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
         let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
         check(unsafe {
@@ -87,6 +122,11 @@
     
     impl CounterId {
         /// Converts the Counter ID into a zero-based integer suitable for array indexing.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the counter ID is not valid or if the underlying
+        /// CFE call fails.
         pub fn to_index(&self) -> Result<u32> {
             let mut index = MaybeUninit::uninit();
             check(unsafe { ffi::CFE_ES_CounterID_ToIndex(self.0, index.as_mut_ptr()) })?;
--- a/src/cfe/es/lib.rs
+++ b/src/cfe/es/lib.rs
@@ -1,3 +1,5 @@
+//! Safe wrappers for CFE Library query APIs.
+
 use crate::cfe::es::app::AppInfo;
 use crate::error::{Error, Result};
 use crate::ffi;
@@ -5,12 +7,23 @@
 use core::mem::MaybeUninit;
 use heapless::{CString, String};
 
+/// A type-safe, zero-cost wrapper for a cFE Library ID.
+///
+/// This is a lightweight, `Copy`-able handle that represents a unique loaded library.
+/// It can be used to query information about that specific library.
 #[derive(Debug, Clone, Copy, PartialEq, Eq)]
 #[repr(transparent)]
 pub struct LibId(pub ffi::CFE_ES_LibId_t);
 
 impl LibId {
     /// Retrieves the cFE Library ID for a given library name.
+    ///
+    /// # Arguments
+    /// * `name`: The registered name of the library to look up.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if no library with the given name is found.
     pub fn from_name(name: &str) -> Result<Self> {
         let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
         c_name
@@ -23,6 +36,11 @@
     }
 
     /// Retrieves the name for this library ID.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the Lib ID is not valid, the buffer is too small
+    /// (unlikely with `heapless`), or the name is not valid UTF-8.
     pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
         let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
         check(unsafe {
@@ -38,6 +56,11 @@
     ///
     /// Note: This reuses the `AppInfo` struct, as the underlying FFI type is the same.
     /// Fields related to tasks (e.g., `MainTaskId`) will not be meaningful for a library.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the Lib ID is not valid or if the underlying
+    /// CFE call fails.
     pub fn info(&self) -> Result<AppInfo> {
         let mut lib_info_uninit = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_GetLibInfo(lib_info_uninit.as_mut_ptr(), self.0) })?;
@@ -47,6 +70,11 @@
     }
 
     /// Converts the Lib ID into a zero-based integer suitable for array indexing.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the Lib ID is not valid or if the underlying
+    /// CFE call fails.
     pub fn to_index(&self) -> Result<u32> {
         let mut index = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_LibID_ToIndex(self.0, index.as_mut_ptr()) })?;
--- a/src/cfe/es/mod.rs
+++ b/src/cfe/es/mod.rs
@@ -1,4 +1,11 @@
-    //! ES (Executive Services) interface.
-    
+//! CFE Executive Services (ES) interface.
+//!
+//! This module provides safe, idiomatic Rust wrappers for the cFE Executive
+//! Services API. ES is the core of cFE, responsible for application and task
+//! management, system-wide resources like memory pools and counters, and the
+//! overall application lifecycle.
+//!
+//! The primary entry point for most applications is the [`app`] module, which
+//! provides the `App` context and the `AppMain` trait for structuring a cFE application.
+
     pub mod app;
     pub mod cds;
     pub mod counter;
--- a/src/cfe/es/perf.rs
+++ b/src/cfe/es/perf.rs
@@ -1,3 +1,5 @@
+//! Safe wrapper for CFE Performance Logging.
+
 use crate::ffi;
 
     /// A performance marker that logs entry and exit points for performance measurement.
@@ -8,7 +10,13 @@
     }
     
     impl PerfMarker {
-        /// Creates a new performance marker with the given ID.
+        /// Creates a new performance marker with the given ID and logs an "entry" event.
+        ///
+        /// # Arguments
+        /// * `id`: A numeric identifier for the performance event.
+        ///
+        /// # C-API Mapping
+        /// This calls `CFE_ES_PerfLogAdd(id, 0)`.
         pub fn new(id: u32) -> Self {
             unsafe {
                 ffi::CFE_ES_PerfLogAdd(id, 0);
@@ -18,6 +26,10 @@
     }
     
     impl Drop for PerfMarker {
+        /// Logs an "exit" event when the marker goes out of scope.
+        ///
+        /// # C-API Mapping
+        /// This calls `CFE_ES_PerfLogAdd(self.id, 1)`.
         fn drop(&mut self) {
             unsafe {
                 ffi::CFE_ES_PerfLogAdd(self.id, 1);
--- a/src/cfe/es/pool.rs
+++ b/src/cfe/es/pool.rs
@@ -107,6 +107,10 @@
         /// # Arguments
         /// * `memory`: A mutable static byte slice to be used as the pool's memory.
         /// * `use_mutex`: If `true`, access to the pool will be protected by a mutex.
+        ///
+        /// # Errors
+        /// Returns an error if the memory pool cannot be created, e.g., due to an
+        //  invalid memory pointer or size.
         pub fn new(memory: &'static mut [u8], use_mutex: bool) -> Result<Self> {
             let mut handle = ffi::CFE_ES_MEMHANDLE_UNDEFINED;
             let status = if use_mutex {
@@ -131,6 +135,11 @@
         /// * `memory`: A mutable static byte slice for the pool's memory.
         /// * `use_mutex`: If `true`, access to the pool is protected by a mutex.
         /// * `block_sizes`: A slice of `usize` defining the bucket sizes for the pool.
+        ///
+        /// # Errors
+        /// Returns an error if the pool cannot be created, e.g., due to an invalid
+        /// argument, too many block sizes, or an external resource failure (like
+        /// failing to create a mutex).
         pub fn new_ex(
             memory: &'static mut [u8],
             use_mutex: bool,
@@ -156,6 +165,11 @@
         ///
         /// Returns a `PoolBuffer` guard. When this guard is dropped, the memory is
         /// automatically returned to the pool.
+        ///
+        /// # Errors
+        /// Returns an error if a buffer cannot be allocated, for example, if the pool
+        /// is out of memory or the requested size is larger than the largest available
+        /// block size.
         pub fn get_buf(&self, size: usize) -> Result<PoolBuffer<'_>> {
             let mut buf_ptr = core::ptr::null_mut();
             let actual_size = unsafe { ffi::CFE_ES_GetPoolBuf(&mut buf_ptr, self.handle, size) };
@@ -172,6 +186,10 @@
         }
     
         /// Retrieves statistics about this memory pool.
+        ///
+        /// # Errors
+        /// Returns an error if the pool handle is invalid or the underlying CFE
+        /// call fails.
         pub fn stats(&self) -> Result<MemPoolStats> {
             let mut stats = MaybeUninit::uninit();
             check(unsafe { ffi::CFE_ES_GetMemPoolStats(stats.as_mut_ptr(), self.handle) })?;
@@ -181,6 +199,10 @@
         /// Gets information about a buffer previously allocated from this pool.
         ///
         /// Returns the allocated size of the buffer.
+        ///
+        /// # Errors
+        /// Returns an error if the pool handle is invalid or the provided buffer
+        /// pointer does not belong to this pool.
         pub fn get_buf_info(&self, buf: &PoolBuffer) -> Result<usize> {
             let status = unsafe { ffi::CFE_ES_GetPoolBufInfo(self.handle, buf.ptr) };
             if status < 0 {
--- a/src/cfe/es/resource.rs
+++ b/src/cfe/es/resource.rs
@@ -1,4 +1,11 @@
 //! Safe wrappers for generic CFE Resource ID functions.
+//!
+//! This module provides utilities for introspecting generic `CFE_ResourceId_t`
+//! values, which are the underlying type for various specific IDs like `AppId`,
+//! `LibId`, `CounterId`, etc.
+//!
+//! It allows for converting specific IDs into the generic `ResourceId` and
+//! querying information about them in a type-agnostic way.
     
     use crate::cfe::es::app::AppId;
     use crate::cfe::es::app::AppInfo;
@@ -8,7 +15,10 @@
     use crate::status::check;
     use core::mem::MaybeUninit;
     
-    /// A generic, type-safe wrapper for a CFE Resource ID.
+    /// A generic, type-safe wrapper for a `CFE_ResourceId_t`.
+    ///
+    /// This can represent any CFE resource, such as an application, library,
+    /// counter, etc.
     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
     #[repr(transparent)]
     pub struct ResourceId(pub ffi::CFE_ResourceId_t);
@@ -26,17 +36,36 @@
     }
     
     /// Gets the base value (type/category) from a resource ID value.
+    ///
+    /// This masks out the ID serial number to obtain the base value, which is
+    /// different for each resource type (e.g., all App IDs share the same base).
+    ///
+    /// # C-API Mapping
+    /// This is a wrapper for `CFE_ResourceId_GetBase`.
     pub fn get_base(resource_id: ResourceId) -> u32 {
         unsafe { ffi::CFE_ResourceId_GetBase(resource_id.0) }
     }
     
     /// Gets the serial number from a resource ID value.
+    ///
+    /// This masks out the ID base value to obtain the unique serial number for
+    /// this specific resource instance.
+    ///
+    /// # C-API Mapping
+    /// This is a wrapper for `CFE_ResourceId_GetSerial`.
     pub fn get_serial(resource_id: ResourceId) -> u32 {
         unsafe { ffi::CFE_ResourceId_GetSerial(resource_id.0) }
     }
     
     /// Retrieves information about an Application or Library given a specified Resource ID.
     ///
-    /// This is a generic wrapper that can be used for either an `AppId` or a `LibId`.
+    /// This is a generic wrapper that inspects the resource ID and calls the
+    /// appropriate underlying function (`CFE_ES_GetAppInfo` or `CFE_ES_GetLibInfo`).
+    ///
+    /// # Errors
+    /// Returns an error if the resource ID is not a valid App or Library ID, or if
+    /// the underlying CFE call fails.
     pub fn get_module_info(res_id: ResourceId) -> Result<AppInfo> {
         let mut module_info_uninit = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_GetModuleInfo(module_info_uninit.as_mut_ptr(), res_id.0) })?;
--- a/src/cfe/es/system.rs
+++ b/src/cfe/es/system.rs
@@ -1,3 +1,5 @@
+//! Safe wrappers for CFE system-level functions and types.
+
 use crate::error::Result;
 use crate::ffi;
 use crate::status::check;
@@ -107,6 +109,11 @@
     
     /// Allows an application to wait until the cFE system reaches a minimum state.
     ///
+    /// # C-API Mapping
+    ///
+    /// This is a safe wrapper for `CFE_ES_WaitForSystemState`.
+    ///
     /// This is useful for synchronizing application startup phases. For example, an
     /// application can wait until `SystemState::CoreReady` before attempting to
     /// subscribe to messages from core cFE services.
@@ -122,6 +129,9 @@
     }
     
     /// Returns the cFE-defined processor ID for the current CPU.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_PSP_GetProcessorId`.
     pub fn get_processor_id() -> u32 {
         unsafe { ffi::CFE_PSP_GetProcessorId() }
     }
@@ -131,8 +141,11 @@
         unsafe { ffi::CFE_PSP_GetSpacecraftId() }
     }
     
-    /// Reset the cFE Core and all cFE Applications. This function does not return.
-    ///
+    /// Resets the cFE Core and all cFE Applications. This function does not return.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_ES_ResetCFE`. If the underlying function
+    /// returns (which indicates an error), this function will loop infinitely.
     /// # Arguments
     /// * `reset_type`: The type of reset to perform (`PowerOn` or `Processor`).
     pub fn reset_cfe(reset_type: ResetType) -> ! {
@@ -165,6 +178,9 @@
     /// Normally the ES background task wakes up at a periodic interval.
     /// Whenever new background work is added, this can be used to wake the task
     /// early, which may reduce the delay before the job is processed.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_ES_BackgroundWakeup`.
     pub fn background_wakeup() {
         unsafe { ffi::CFE_ES_BackgroundWakeup() };
     }
@@ -173,6 +189,9 @@
     ///
     /// This hook routine is called from the PSP when an exception or
     /// other asynchronous system event occurs.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_ES_ProcessAsyncEvent`.
     pub fn process_async_event() {
         unsafe { ffi::CFE_ES_ProcessAsyncEvent() };
     }
--- a/src/cfe/es/task.rs
+++ b/src/cfe/es/task.rs
@@ -37,6 +37,12 @@
         /// * `stack_size`: The size of the stack to allocate for the new task.
         /// * `priority`: The priority of the new task (0=highest, 255=lowest).
         /// * `flags`: Reserved for future use.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the task cannot be created, for example due to an
+        /// invalid name, invalid priority, or if the OS fails to create the
+        /// underlying task resource.
         pub fn new(
             name: &str,
             entry_point: TaskEntryPoint,
@@ -88,11 +94,15 @@
     }
     
     /// A high-level wrapper around the FFI's `CFE_ES_TaskInfo_t`.
+    ///
+    /// This struct contains detailed information about a cFE task, such as its
+    /// name, parent application, stack size, and priority.
     #[derive(Debug, Clone)]
     pub struct TaskInfo {
         inner: ffi::CFE_ES_TaskInfo_t,
     }
     
+    /// Public accessors for `TaskInfo` fields.
     impl TaskInfo {
         /// Returns the OSAL Task ID for this task.
         pub fn task_id(&self) -> TaskId {
@@ -109,6 +119,11 @@
         }
     
         /// Returns the registered name of the task.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the name from the underlying FFI struct is not
+        /// valid UTF-8 or cannot fit into the `CString` buffer.
         pub fn name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
             let c_str = unsafe { CStr::from_ptr(self.inner.TaskName.as_ptr()) };
             let mut s = CString::new();
@@ -118,6 +133,11 @@
         }
     
         /// Returns the registered name of the parent application.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the name from the underlying FFI struct is not
+        /// valid UTF-8 or cannot fit into the `CString` buffer.
         pub fn app_name(&self) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
             let c_str = unsafe { CStr::from_ptr(self.inner.AppName.as_ptr()) };
             let mut s = CString::new();
@@ -128,6 +148,11 @@
     }
     
     /// Retrieves the cFE Task ID for a given task name.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if no task with the given name is found or if the name
+    /// is too long.
     pub fn get_task_id_by_name(name: &str) -> Result<TaskId> {
         let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
         c_name
@@ -140,6 +165,11 @@
     }
     
     /// Retrieves detailed information about the task with the given ID.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the task ID is not valid or if the underlying
+    /// CFE call fails.
     pub fn get_task_info(task_id: TaskId) -> Result<TaskInfo> {
         let mut task_info_uninit = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_GetTaskInfo(task_info_uninit.as_mut_ptr(), task_id.0) })?;
@@ -152,12 +182,22 @@
     /// This function is a standalone wrapper for `CFE_ES_DeleteChildTask`. Using the
     /// `ChildTask` RAII struct is generally preferred to ensure the task is always deleted.
     /// It must not be called for an Application's Main Task.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the task ID is invalid, if the task is a main application task,
+    /// or if the OS-level task deletion fails.
     pub fn delete_child_task(task_id: TaskId) -> Result<()> {
         check(unsafe { ffi::CFE_ES_DeleteChildTask(task_id.0) })?;
         Ok(())
     }
     
     /// Retrieves the cFE Task Name for a given task ID.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the task ID is invalid, the buffer is too small, or the
+    /// name is not valid UTF-8.
     pub fn get_task_name(task_id: TaskId) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
         let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
         check(unsafe {
@@ -175,6 +215,9 @@
     ///
     /// This function terminates the currently running child task and does not return.
     /// It must not be called from an Application's Main Task.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_ES_ExitChildTask`.
     pub fn exit_child_task() -> ! {
         unsafe {
             ffi::CFE_ES_ExitChildTask();
@@ -188,6 +231,9 @@
     /// (via `CFE_ES_RunLoop`), as the counter is incremented automatically.
     /// It is useful for child tasks or other contexts where the counter needs to
     /// be manually managed to indicate liveness.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_ES_IncrementTaskCounter`.
     pub fn increment_task_counter() {
         unsafe {
             ffi::CFE_ES_IncrementTaskCounter();
@@ -195,12 +241,22 @@
     }
     
     /// Converts a CFE Task ID into a zero-based integer suitable for array indexing.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the task ID is not valid or if the underlying CFE
+    /// call fails.
     pub fn task_id_to_index(task_id: TaskId) -> Result<u32> {
         let mut index = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_TaskID_ToIndex(task_id.0, index.as_mut_ptr()) })?;
         Ok(unsafe { index.assume_init() })
     }
     
     /// Retrieves the CFE Task ID of the currently executing task.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if called from a context that is not a registered CFE
+    /// task.
     pub fn current_cfe_id() -> Result<TaskId> {
         let mut task_id = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_GetTaskID(task_id.as_mut_ptr()) })?;
--- a/src/cfe/es/util.rs
+++ b/src/cfe/es/util.rs
@@ -16,6 +16,9 @@
     ///   this should be the result of a previous call to this function. For a new calculation,
     ///   this should typically be 0.
     /// * `crc_type`: The CRC algorithm to use.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_ES_CalculateCRC`.
     pub fn calculate_crc(data: &[u8], input_crc: u32, crc_type: CrcType) -> u32 {
         unsafe {
             ffi::CFE_ES_CalculateCRC(
--- a/src/cfe/evs/event.rs
+++ b/src/cfe/evs/event.rs
@@ -1,8 +1,11 @@
+//! Safe, idiomatic wrappers for the CFE Event Services (EVS) API.
+
 use crate::cfe::es::app::AppId;
 use crate::cfe::time::SysTime;
 use crate::error::{Error, Result};
 use crate::ffi;
 use crate::status::check;
 
+/// The type or severity of a cFE software event.
 #[derive(Debug, Clone, Copy)]
 #[repr(u16)]
 pub enum EventType {
@@ -13,6 +16,12 @@
     }
     
     /// Registers the calling application with the Event Services. Must be called before sending events.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the registration fails. This can happen if the
+    /// application ID is invalid or if EVS has an internal error.
     pub fn register() -> Result<()> {
         // For simplicity, not registering any filters initially. This can be expanded.
         let status = unsafe {
@@ -31,7 +40,15 @@
     /// This is a safe wrapper around `CFE_EVS_SendEvent`. It handles creating a
     /// C-style format string and passing the arguments.
     ///
-    /// NOTE: Due to the varargs nature of the underlying C function, this wrapper uses `core::fmt`
+    /// # Arguments
+    /// * `event_id`: An application-specific numeric identifier for the event.
+    /// * `event_type`: The severity of the event (`Debug`, `Information`, `Error`, `Critical`).
+    /// * `message`: The event message string.
+    ///
+    /// # Errors
+    /// Returns an error if the application is not registered with EVS or if the
+    /// message is too long to fit in the internal buffer.
+    ///
     /// and a temporary buffer. Ensure the buffer is large enough for your event messages.
     pub fn send_event(event_id: u16, event_type: EventType, message: &str) -> Result<()> {
         // Create a heapless CString for the message.
@@ -58,6 +75,15 @@
     
     /// Sends a formatted software event from a specified application ID.
     ///
+    /// # Arguments
+    /// * `app_id`: The `AppId` from which the event should appear to originate.
+    /// * `event_id`: An application-specific numeric identifier for the event.
+    /// * `event_type`: The severity of the event.
+    /// * `message`: The event message string.
+    ///
+    /// # Errors
+    /// Returns an error if the specified `app_id` is invalid, if the message is too long,
+    /// or if the underlying `CFE_EVS_SendEventWithAppID` call fails.
     /// This allows a library or shared service to send an event that appears to
     /// originate from the calling application.
     pub fn send_event_with_app_id(
@@ -86,6 +112,18 @@
     }
     
     /// Sends a formatted software event with a specific time tag.
+    ///
+    /// This is useful in situations where an error condition is detected at one
+    /// time, but the event message is reported at a later time.
+    ///
+    /// # Arguments
+    /// * `time`: The timestamp to include in the event message.
+    /// * `event_id`: An application-specific numeric identifier for the event.
+    /// * `event_type`: The severity of the event.
+    /// * `message`: The event message string.
+    ///
+    /// # Errors
+    /// Returns an error if the application is not registered or if the message is too long.
     pub fn send_timed_event(
         time: SysTime,
         event_id: u16,
@@ -109,12 +147,20 @@
     }
     
     /// Resets the filter for a single event ID for the calling application.
+    ///
+    /// # Errors
+    /// Returns an error if the application is not registered or if the specified
+    /// `event_id` has no filter registered.
     pub fn reset_filter(event_id: u16) -> Result<()> {
         check(unsafe { ffi::CFE_EVS_ResetFilter(event_id) })?;
         Ok(())
     }
     
     /// Resets all event filters for the calling application.
+    ///
+    /// # Errors
+    /// Returns an error if the application is not registered with Event Services.
     pub fn reset_all_filters() -> Result<()> {
         check(unsafe { ffi::CFE_EVS_ResetAllFilters() })?;
         Ok(())
--- a/src/cfe/evs/mod.rs
+++ b/src/cfe/evs/mod.rs
@@ -1,4 +1,7 @@
-    //! EVS (Event Service) interface.
-    
+//! CFE Event Services (EVS) interface.
+//!
+//! This module provides safe wrappers for sending software event messages, which
+//! are used for telemetry, on-board logging, and debugging.
+
     pub mod event;
--- a/src/cfe/mod.rs
+++ b/src/cfe/mod.rs
@@ -1,4 +1,7 @@
-    //! CFE (Core Flight Executive) interface for CFS.
-    
+//! CFE (Core Flight Executive) API wrappers.
+//!
+//! This module provides safe, idiomatic Rust wrappers for the primary CFE services:
+//! Executive Services (ES), Event Services (EVS), Software Bus (SB),
+//! Table Services (TBL), and Time Services (TIME).
+
     pub mod es;
     pub mod evs;
     pub mod sb;
--- a/src/cfe/sb/bus.rs
+++ b/src/cfe/sb/bus.rs
@@ -1,10 +1,24 @@
+//! High-level interface for the cFE Software Bus.
+
 use crate::cfe::sb::msg::MessageRef;
 use crate::error::Result;
 use crate::ffi;
 use crate::status::check;
-    
+
+/// A handle to the cFE Software Bus.
+///
+/// This is a zero-sized type used as a namespace for software bus operations.
 pub struct SoftwareBus;
-    
+
 impl SoftwareBus {
-        /// Transmits a message by copying its contents into the Software Bus.
+    /// Transmits a message by copying its contents into the Software Bus.
+    ///
+    /// # Arguments
+    /// * `msg`: A `MessageRef` pointing to the message to be sent.
+    /// * `is_origination`: `true` if this is the first time the message is sent
+    ///   (allowing CFE to update headers like sequence count), `false` if forwarding.
+    ///
+    /// # Errors
+    /// Returns an error if the message is too large or if the SB memory pool is exhausted.
         pub fn transmit_msg(msg: MessageRef, is_origination: bool) -> Result<()> {
             check(unsafe {
                 ffi::CFE_SB_TransmitMsg(msg.as_slice().as_ptr() as *const _, is_origination)
--- a/src/cfe/sb/mod.rs
+++ b/src/cfe/sb/mod.rs
@@ -1,4 +1,8 @@
-    //! SB (Software Bus) interface.
-    
+//! CFE Software Bus (SB) interface.
+//!
+//! This module provides safe, idiomatic Rust wrappers for the cFE Software Bus
+//! and Message Services APIs. It enables applications to create message pipes,
+//! subscribe to messages, and send/receive messages in a type-safe manner.
+
     pub mod bus;
     pub mod msg;
     pub mod pipe;
--- a/src/cfe/sb/msg.rs
+++ b/src/cfe/sb/msg.rs
@@ -15,10 +15,12 @@
     #[derive(Debug, Clone, Copy, Default)]
     pub struct TlmHeader(pub ffi::CFE_MSG_TelemetryHeader_t);
     
+    /// A type-safe, zero-cost wrapper for a cFE Software Bus Message ID.
     #[derive(Debug, Clone, Copy)]
     #[repr(transparent)]
     pub struct MsgId(pub ffi::CFE_SB_MsgId_t);
     
+    // Manual implementation because the inner C struct doesn't derive it.
     impl PartialEq for MsgId {
         fn eq(&self, other: &Self) -> bool {
             self.0.Value == other.0.Value
@@ -29,14 +31,13 @@
     impl MsgId {
         /// Checks if the message ID is numerically within the valid mission-defined range.
         ///
-        /// This is a safe Rust implementation of the C macro `CFE_SB_IsValidMsgId`.
-        /// It checks that the ID is not zero and does not exceed `CFE_PLATFORM_SB_HIGHEST_VALID_MSGID`.
+        /// # C-API Mapping
+        /// This is a safe Rust implementation of the C function `CFE_SB_IsValidMsgId`.
         pub fn is_valid(&self) -> bool {
             // Per the CFE_SB_IsValidMsgId logic, a valid ID is non-zero and within the platform-defined range.
             self.0.Value != 0 && self.0.Value <= ffi::CFE_PLATFORM_SB_HIGHEST_VALID_MSGID
         }
     
-        // Add these new constructors:
         /// Creates a command `MsgId` from a mission-defined topic ID and a CPU instance number.
         pub fn from_cmd_topic(topic_id: u16, instance_num: u16) -> Self {
             Self(ffi::CFE_SB_MsgId_t {
@@ -157,6 +158,11 @@
         /// Gets the timestamp from a telemetry message header.
         ///
         /// Returns an error if the message does not have a secondary telemetry header.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbWrongMsgType` if the message header does not contain a
+        /// time field (e.g., for a standard command message).
         pub fn time(&self) -> Result<SysTime> {
             let mut time = MaybeUninit::uninit();
             let status = unsafe {
@@ -183,7 +191,10 @@
         /// Validates the checksum of a command message.
         ///
         /// Returns `Ok(true)` if the checksum is valid, `Ok(false)` if it is not.
-        /// Returns an error if the message does not have a command secondary header.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbWrongMsgType` if the message does not have a checksum field.
         pub fn validate_checksum(&self) -> Result<bool> {
             let mut is_valid = false;
             let status = unsafe {
@@ -216,9 +227,11 @@
     
         /// Gets a pointer to the user data portion of the message.
         ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_SB_GetUserData`.
+        ///
         /// # Safety
-        /// The returned pointer should be cast to the appropriate payload struct type.
-        /// The caller must ensure that the payload struct matches the message definition.
+        /// The caller must ensure the returned pointer is cast to the correct payload struct type.
         pub unsafe fn user_data(&self) -> *mut libc::c_void {
             ffi::CFE_SB_GetUserData(self.slice.as_ptr() as *mut ffi::CFE_MSG_Message_t)
         }
@@ -228,6 +241,12 @@
         }
     
         /// Gets the segmentation flag from the message header.
+        ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_MSG_GetSegmentationFlag`.
+        ///
+        /// # Errors
+        /// Returns `Error::MsgBadArgument` if the message pointer is invalid.
         pub fn segmentation_flag(&self) -> Result<ffi::CFE_MSG_SegmentationFlag_t> {
             let mut flag = MaybeUninit::uninit();
             check(unsafe {
@@ -240,6 +259,12 @@
         }
     
         /// Gets the EDS version from the message header.
+        ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_MSG_GetEDSVersion`.
+        ///
+        /// # Errors
+        /// Returns `Error::MsgBadArgument` if the message pointer is invalid.
         pub fn eds_version(&self) -> Result<u16> {
             let mut version = MaybeUninit::uninit();
             check(unsafe {
@@ -250,6 +275,12 @@
         }
     
         /// Gets the endianness indicator from the message header.
+        ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_MSG_GetEndian`.
+        ///
+        /// # Errors
+        /// Returns `Error::MsgBadArgument` if the message pointer is invalid.
         pub fn endian(&self) -> Result<ffi::CFE_MSG_Endian_t> {
             let mut endian = MaybeUninit::uninit();
             check(unsafe {
@@ -260,6 +291,12 @@
         }
     
         /// Gets the playback flag from the message header.
+        ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_MSG_GetPlaybackFlag`.
+        ///
+        /// # Errors
+        /// Returns `Error::MsgBadArgument` if the message pointer is invalid.
         pub fn playback_flag(&self) -> Result<ffi::CFE_MSG_PlaybackFlag_t> {
             let mut flag = MaybeUninit::uninit();
             check(unsafe {
@@ -270,6 +307,12 @@
         }
     
         /// Gets the subsystem ID from the message header.
+        ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_MSG_GetSubsystem`.
+        ///
+        /// # Errors
+        /// Returns `Error::MsgBadArgument` if the message pointer is invalid.
         pub fn subsystem(&self) -> Result<u16> {
             let mut subsystem = MaybeUninit::uninit();
             check(unsafe {
@@ -280,6 +323,12 @@
         }
     
         /// Gets the system ID from the message header.
+        ///
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_MSG_GetSystem`.
+        ///
+        /// # Errors
+        /// Returns `Error::MsgBadArgument` if the message pointer is invalid.
         pub fn system(&self) -> Result<u16> {
             let mut system = MaybeUninit::uninit();
             check(unsafe {
@@ -467,9 +516,11 @@
         }
     
         /// Gets a raw pointer to the user data portion of the message.
+        /// # C-API Mapping
+        /// This is a wrapper for `CFE_SB_GetUserData`.
         ///
         /// # Safety
-        /// The returned pointer should be cast to the appropriate payload struct type.
-        /// The caller must ensure that the payload struct matches the message definition.
+        /// The caller must ensure the returned pointer is cast to the correct payload struct type
+        /// and that the size of the payload struct does not exceed the user data length.
         pub unsafe fn user_data(&mut self) -> *mut libc::c_void {
             ffi::CFE_SB_GetUserData(self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t)
         }
@@ -477,11 +528,15 @@
         /// Returns a mutable reference to the message payload, interpreted as type `P`.
         ///
         /// This is the primary safe method for accessing the payload of a message.
-        /// It performs a size check to prevent buffer overruns.
+        /// It performs a size check to ensure the payload type `P` fits within the
+        /// available user data area of the message buffer, preventing buffer overruns.
         ///
         /// # Errors
+        ///
         /// Returns `Error::StatusWrongMsgLength` if `size_of::<P>()` is larger
         /// than the available user data length in the message buffer.
+        ///
+        /// # Example
         pub fn payload<P: Sized>(&mut self) -> Result<&mut P> {
             if core::mem::size_of::<P>() > self.user_data_length() {
                 return Err(Error::StatusWrongMsgLength);
@@ -582,6 +637,12 @@
     }
     
     /// Gets the message type (Command or Telemetry) from a message ID.
+    ///
+    /// # C-API Mapping
+    /// This is a wrapper for `CFE_MSG_GetTypeFromMsgId`.
+    ///
+    /// # Errors
+    /// Returns `Error::MsgBadArgument` if the `msg_id` is invalid.
     pub fn get_type_from_msg_id(msg_id: MsgId) -> Result<MsgType> {
         let mut msg_type = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_MSG_GetTypeFromMsgId(msg_id.0, msg_type.as_mut_ptr()) })?;
@@ -591,13 +652,19 @@
     /// Copies a Rust string slice into a fixed-size C-style char array within a message.
     ///
     /// This is a safe wrapper around `CFE_SB_MessageStringSet`. It handles truncation
-    /// and null-padding correctly.
+    /// and null-padding correctly, ensuring the entire destination buffer is initialized.
     ///
     /// # Arguments
     /// * `dest`: A mutable slice representing the fixed-size char array in the message.
     /// * `src`: The Rust string slice to copy from.
+    ///
+    /// # Returns
+    ///
+    /// On success, returns the number of bytes copied from `src` (which may be less
+    /// than the size of `dest` if `src` was shorter).
     pub fn message_string_set(dest: &mut [i8], src: &str) -> Result<usize> {
         let bytes_copied = unsafe {
             ffi::CFE_SB_MessageStringSet(
@@ -624,6 +691,14 @@
     /// * `src`: The fixed-size C-style `i8` array from the message.
     /// * `default_src`: An optional default string to use if the source string is empty.
     pub fn message_string_get<'a>(
+    ///
+    /// # Returns
+    ///
+    /// On success, returns a `&str` slice of the copied, null-terminated string
+    /// within the `dest` buffer.
+    ///
+    /// # Errors
+    /// Returns an error if the underlying CFE call fails or if the resulting string is not valid UTF-8.
         dest: &'a mut [u8],
         src: &[i8],
         default_src: Option<&str>,
@@ -642,6 +717,8 @@
     }
     
     /// Gets the next sequence count value, handling rollovers correctly.
+    ///
+    /// This is a pure function that calculates the next sequence count, wrapping around to 0 after the maximum value (0x3FFF).
     pub fn get_next_sequence_count(current_count: u16) -> u16 {
         unsafe { ffi::CFE_MSG_GetNextSequenceCount(current_count) }
     }
--- a/src/cfe/sb/pipe.rs
+++ b/src/cfe/sb/pipe.rs
@@ -10,6 +10,7 @@
     /// Option for `receive` to perform a non-blocking poll for a message.
     pub const POLL: i32 = ffi::CFE_SB_POLL as i32;
     
+    /// A type-safe, zero-cost wrapper for a cFE Software Bus Pipe ID.
     #[derive(Debug, Clone, Copy)]
     #[repr(transparent)]
     pub struct PipeId(pub ffi::CFE_SB_PipeId_t);
@@ -21,6 +22,12 @@
     impl Eq for PipeId {}
     
     impl PipeId {
+        /// Converts the Pipe ID into a zero-based integer suitable for array indexing.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the Pipe ID is not valid or if the underlying
+        /// CFE call fails.
         pub fn to_index(&self) -> Result<u32> {
             let mut index = MaybeUninit::uninit();
             check(unsafe { ffi::CFE_SB_PipeId_ToIndex(self.0, index.as_mut_ptr()) })?;
@@ -66,6 +73,12 @@
         /// # Arguments
         /// * `name` - A unique string to identify the pipe.
         /// * `depth` - The maximum number of messages the pipe can hold.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the pipe cannot be created, e.g., if the name is
+        /// too long, the maximum number of pipes has been reached, or if the
+        /// underlying OS queue creation fails.
         pub fn new(name: &str, depth: u16) -> Result<Self> {
             let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
             c_name
@@ -87,6 +100,11 @@
         /// * `qos`: The requested Quality of Service.
         /// * `msg_lim`: The maximum number of messages with this Message ID to
         ///   allow in this pipe at the same time.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the subscription fails, e.g., due to an invalid argument,
+        /// or if the SB routing tables are full.
         pub fn subscribe_ex(&self, msg_id: MsgId, qos: Qos, msg_lim: u16) -> Result<()> {
             check(unsafe { ffi::CFE_SB_SubscribeEx(msg_id.0, self.id.0, qos.0, msg_lim) })?;
             Ok(())
@@ -96,18 +114,35 @@
         ///
         /// # Arguments
         /// * `msg_id`: The message ID of the message to be subscribed to.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the subscription fails, e.g., due to an invalid argument,
+        /// or if the SB routing tables are full.
         pub fn subscribe(&self, msg_id: MsgId) -> Result<()> {
             check(unsafe { ffi::CFE_SB_Subscribe(msg_id.0, self.id.0) })?;
             Ok(())
         }
     
         /// Unsubscribes this pipe from messages with the specified `MsgId`.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbBadArgument` if the message ID or pipe ID is invalid.
+        /// Note: Unsubscribing from a message that was not previously subscribed to
+        /// is not considered an error and will return `Ok(())`.
         pub fn unsubscribe(&self, msg_id: MsgId) -> Result<()> {
             check(unsafe { ffi::CFE_SB_Unsubscribe(msg_id.0, self.id.0) })?;
             Ok(())
         }
     
         /// Unsubscribes this pipe from messages, keeping the request local to this CPU.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbBadArgument` if the message ID or pipe ID is invalid.
+        /// Note: Unsubscribing from a message that was not previously subscribed to
+        /// is not considered an error and will return `Ok(())`.
         ///
         /// This is typically only used by a Software Bus Network (SBN) application.
         pub fn unsubscribe_local(&self, msg_id: MsgId) -> Result<()> {
@@ -115,7 +150,9 @@
             Ok(())
         }
     
-        /// Receives a message from this pipe into a user-provided buffer.
+        /// Receives a message from this pipe, copying it into a user-provided buffer.
+        ///
+        /// This method receives a message from the CFE-managed internal buffer and safely copies it into the provided `buf`.
         ///
         /// # Arguments
         /// * `timeout`: Timeout in milliseconds. Use `sb::pipe::PEND_FOREVER` to block
@@ -124,7 +161,12 @@
         ///
         /// # Returns
         /// A `MessageRef` containing the message data, tied to the lifetime of `buffer`.
+        ///
+        /// # Errors
+        /// Returns `Error::SbTimeOut` or `Error::SbNoMessage` if no message is received within the timeout.
+        /// Returns `Error::OsErrInvalidSize` if the received message is larger than `buf`.
         pub fn receive<'a>(&self, timeout: i32, buf: &'a mut [u8]) -> Result<MessageRef<'a>> {
             let mut buf_ptr = MaybeUninit::uninit();
     
@@ -158,12 +200,20 @@
         }
     
         /// Sets options on the pipe, such as `PipeOptions::IGNORE_MINE`.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbBadArgument` if the pipe ID is invalid.
         pub fn set_opts(&self, opts: PipeOptions) -> Result<()> {
             check(unsafe { ffi::CFE_SB_SetPipeOpts(self.id.0, opts.bits()) })?;
             Ok(())
         }
     
         /// Gets the current options for the pipe.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbBadArgument` if the pipe ID is invalid.
         pub fn get_opts(&self) -> Result<PipeOptions> {
             let mut opts = MaybeUninit::uninit();
             check(unsafe { ffi::CFE_SB_GetPipeOpts(self.id.0, opts.as_mut_ptr()) })?;
@@ -177,6 +227,11 @@
         }
     
         /// Gets the registered name of this pipe.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the pipe ID is invalid, the buffer is too small,
+        /// or the name is not valid UTF-8.
         pub fn name(&self) -> Result<String<{ ffi::OS_MAX_API_NAME as usize }>> {
             let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
             check(unsafe {
@@ -189,6 +244,11 @@
         }
     
         /// Finds the `PipeId` for a pipe with the given name.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if no pipe with the given name is found or if the
+        /// name is too long.
         pub fn get_id_by_name(name: &str) -> Result<PipeId> {
             let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
             c_name
@@ -208,6 +268,11 @@
         /// * `msg_id`: The message ID of the message to be subscribed to.
         /// * `msg_lim`: The maximum number of messages with this Message ID to
         ///   allow in this pipe at the same time.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the subscription fails, e.g., due to an invalid argument,
+        /// or if the SB routing tables are full.
         pub fn subscribe_local(&self, msg_id: MsgId, msg_lim: u16) -> Result<()> {
             check(unsafe { ffi::CFE_SB_SubscribeLocal(msg_id.0, self.id.0, msg_lim) })?;
             Ok(())
--- a/src/cfe/sb/send_buf.rs
+++ b/src/cfe/sb/send_buf.rs
@@ -1,3 +1,5 @@
+//! Safe, idiomatic wrapper for cFE Software Bus "zero-copy" message buffers.
+
 use core::mem;
 use core::ops::Deref;
 use core::ops::DerefMut;
@@ -22,6 +24,10 @@
     
     impl SendBuffer {
         /// Allocates a new zero-copy send buffer of the specified size from the CFE SB pool.
+        ///
+        /// # Errors
+        ///
+        /// Returns `Error::SbBufAlocErr` if the SB memory pool is exhausted.
         pub fn new(size: usize) -> Result<Self> {
             let ptr = unsafe { ffi::CFE_SB_AllocateMessageBuffer(size) };
             if ptr.is_null() {
@@ -40,6 +46,10 @@
         /// # Arguments
         /// * `is_origination`: Set to `true` to have CFE automatically fill in fields like
         ///   sequence count and timestamp. Set to `false` when forwarding a message.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the transmission fails, e.g., due to an invalid buffer pointer.
         pub fn send(self, is_origination: bool) -> Result<()> {
             let status = unsafe { ffi::CFE_SB_TransmitBuffer(self.ptr, is_origination) };
     
--- a/src/cfe/tbl.rs
+++ b/src/cfe/tbl.rs
@@ -49,6 +49,14 @@
         /// # Arguments
         /// * `name`: The application-local name for the table.
         /// * `options`: Bitwise-ORed flags for table options (e.g., `TableOptions::DEFAULT`).
+        /// # Errors
+        ///
+        /// Returns an error if the table cannot be registered. This can happen for
+        /// several reasons:
+        /// - The table registry or handle list is full (`TblErrRegistryFull`, `TblErrHandlesFull`).
+        /// - The provided `name` or `size` is invalid (`TblErrInvalidName`, `TblErrInvalidSize`).
+        /// - A table with the same name exists but has a different size or is owned by another app
+        ///   (`TblErrDuplicateDiffSize`, `TblErrDuplicateNotOwned`).
+        /// - The `options` flags are an invalid combination (`TblErrInvalidOptions`).
         /// * `validation_fn`: An optional callback function to validate table loads.
         pub fn new(name: &str, options: TableOptions, validation_fn: ValidationFn) -> Result<Self> {
             let mut handle = MaybeUninit::uninit();
@@ -79,6 +87,12 @@
         ///
         /// # Arguments
         /// * `name`: The full name of the table, in the format "AppName.TableName".
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if no table with the given name exists, if the handle
+        /// list is full, or if the calling application does not have permission
+        /// to access the table.
         pub fn share(name: &str) -> Result<Self> {
             let mut handle = MaybeUninit::uninit();
             let mut c_name = CString::<{ ffi::CFE_MISSION_TBL_MAX_FULL_NAME_LEN as usize }>::new();
@@ -95,6 +109,11 @@
         }
     
         /// Loads data into the table from a file.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the file cannot be found, is improperly formatted,
+        /// or if the load is attempted on a "dump-only" table.
         pub fn load_from_file(&self, filename: &str) -> Result<()> {
             let mut c_filename = CString::<{ ffi::OS_MAX_PATH_LEN as usize }>::new();
             c_filename
@@ -112,6 +131,11 @@
         }
     
         /// Loads data into the table from a memory slice.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the load is attempted on a "dump-only" table or if the
+        /// underlying CFE call fails.
         pub fn load_from_slice(&self, data: &[T]) -> Result<()> {
             let status = unsafe {
                 ffi::CFE_TBL_Load(
@@ -125,12 +149,20 @@
     
         /// Performs periodic processing for the table (update, validate, dump).
         /// This should be called once per application cycle for each owned table.
+        ///
+        /// # Errors
+        /// Returns an error if the table handle is invalid. Informational status codes
+        /// indicating pending actions (like `TblInfoUpdatePending`) are returned as `Ok(Status)`.
         pub fn manage(&self) -> Result<()> {
             check(unsafe { ffi::CFE_TBL_Manage(self.handle.0) })?;
             Ok(())
         }
     
         /// Notifies Table Services that the application has modified the table's contents.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the table handle is invalid or the app has no access.
         /// This is important for critical tables backed by the CDS.
         pub fn modified(&self) -> Result<()> {
             check(unsafe { ffi::CFE_TBL_Modified(self.handle.0) })?;
@@ -139,6 +171,11 @@
     
         /// Gets a read-only accessor to the table's data.
         /// The accessor locks the table and automatically releases it when dropped.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the table handle is invalid, the table has not been
+        /// loaded, or the app has no access.
         pub fn get_accessor(&self) -> Result<TableAccessor<'_, T>> {
             TableAccessor::new(self.handle)
         }
@@ -151,6 +188,11 @@
         ///
         /// # Arguments
         /// * `name`: The full name of the table, in the format "AppName.TableName".
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if no table with the given name is found or if the name
+        /// is malformed.
         pub fn get_info(name: &str) -> Result<TableInfo> {
             let mut c_name = CString::<{ ffi::CFE_MISSION_TBL_MAX_FULL_NAME_LEN as usize }>::new();
             c_name
@@ -163,16 +205,30 @@
             Ok(TableInfo(unsafe { tbl_info_uninit.assume_init() }))
         }
     
+        /// Gets the current status of the table (e.g., update pending, validation pending).
+        /// This is a lower-level alternative to `manage()`.
         pub fn status(&self) -> Result<status::Status> {
             check(unsafe { ffi::CFE_TBL_GetStatus(self.handle.0) })
         }
     
+        /// Updates the table contents if a load is pending.
+        /// This is a lower-level alternative to `manage()`.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the handle is invalid or if no update is pending.
         pub fn update(&self) -> Result<()> {
             check(unsafe { ffi::CFE_TBL_Update(self.handle.0) })?;
             Ok(())
         }
     
+        /// Validates the table contents if a validation is pending.
+        /// This is a lower-level alternative to `manage()`.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the handle is invalid or if no validation is pending.
         pub fn validate(&self) -> Result<()> {
             check(unsafe { ffi::CFE_TBL_Validate(self.handle.0) })?;
             Ok(())
@@ -182,6 +238,11 @@
         ///
         /// This should only be called by the table owner in response to a dump request,
         /// typically after `manage()` returns `Ok(TblInfoDumpPending)`.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the handle is invalid or if no dump is pending.
         pub fn dump_to_buffer(&self) -> Result<()> {
             check(unsafe { ffi::CFE_TBL_DumpToBuffer(self.handle.0) })?;
             Ok(())
@@ -196,6 +257,11 @@
         /// * `msg_id`: Message ID to be used in the notification message.
         /// * `command_code`: Command code to be placed in the secondary header.
         /// * `parameter`: Application-defined value to be passed as a parameter in the message.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if the handle is invalid or if the calling application
+        /// does not own the table.
         pub fn notify_by_message(
             &self,
             msg_id: MsgId,
@@ -211,6 +277,15 @@
     
         /// Gets read-only accessors for multiple tables at once.
         ///
+        /// This is more efficient than calling `get_accessor` for each table individually.
+        ///
+        /// # Errors
+        ///
+        /// Returns an error if any of the handles are invalid or if access cannot be
+        /// granted to any of the tables. If successful, all tables are locked; if it
+        /// fails, no tables are locked.
+        ///
         /// # Safety
         /// The caller must ensure that the types `U` in the returned accessors match
         /// the actual types of the tables identified by the handles.
--- a/src/cfe/time.rs
+++ b/src/cfe/time.rs
@@ -1,4 +1,8 @@
-    //! Time interface for CFE.
-    
+//! CFE Time Services (TIME) interface.
+//!
+//! This module provides safe wrappers for the cFE Time Services API, which is
+//! the primary source for mission-synchronized time in a cFS system. It handles
+//! spacecraft time, Mission Elapsed Time (MET), and conversions between them.
+
     use crate::error::Result;
     use crate::ffi;
     use crate::status::check;
--- a/src/error.rs
+++ b/src/error.rs
@@ -1,5 +1,9 @@
-    use crate::ffi;
+//! Error and status handling for the `leodos-libcfs` library.
+//!
+//! This module defines the `Error` enum, which represents all possible error
+//! conditions from the underlying CFE, OSAL, and PSP APIs. It also provides a
+//! specialized `Result` type for convenience.
+
     use crate::status::check;
     use core::fmt;
     use heapless::CString;
--- a/src/ffi.rs
+++ b/src/ffi.rs
@@ -1,3 +1,11 @@
+/*!
+Low-level FFI bindings for cFE, OSAL, and PSP.
+
+This module contains the raw, `unsafe` function and type definitions generated
+by `rust-bindgen`. It is not intended for direct use by applications. Instead,
+the safe, idiomatic wrappers in other modules of this crate should be used.
+*/
+
     #![allow(clippy::all)]
     #![allow(non_upper_case_globals)]
     #![allow(non_camel_case_types)]
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,4 +1,21 @@
-    #![cfg_attr(not(feature = "std"), no_std)]
-    
+//! # leodos-libcfs: A Safe Rust Wrapper for the Core Flight System
+//!
+//! `leodos-libcfs` provides safe, idiomatic, and zero-cost wrappers around the C APIs of the
+//! NASA Core Flight System (cFS), including the Core Flight Executive (CFE),
+//! Operating System Abstraction Layer (OSAL), and Platform Support Package (PSP).
+//!
+//! This crate is designed to enable the development of cFS applications in Rust
+//! with a high degree of safety and ergonomics, leveraging Rust's ownership,
+//! borrowing, and type system to prevent common errors found in C-based cFS development.
+//!
+//! ## Key Features
+//!
+//! - **Safe Abstractions**: RAII guards for resource management (e.g., `Pipe`, `Table`, `Mutex`),
+//!   preventing resource leaks.
+//! - **Type Safety**: Generic wrappers for message passing (`Queue<T>`), tables (`Table<T>`),
+//!   and critical data stores (`CdsBlock<T>`) ensure data integrity at compile time.
+//! - **Ergonomic API**: A high-level application framework (`App`, `AppMain` trait) simplifies
+//!   the boilerplate of a cFS application.
+//! - **Comprehensive Coverage**: Wrappers for major cFE services (ES, EVS, SB, TBL, TIME)
+//!   and key OSAL/PSP functionalities.
+
     pub mod cfe;
     pub mod error;
     pub mod ffi;
--- a/src/log.rs
+++ b/src/log.rs
@@ -23,6 +23,9 @@
     ///
     /// # Arguments
     /// * `message`: The string to write. It will be truncated if its byte length
+    /// # C-API Mapping
+    /// This is a safe wrapper around the variadic C function `OS_printf`.
+    ///
     ///   exceeds `MAX_PRINTF_MSG_SIZE`.
     pub fn printf(message: &str) {
         let mut c_message = CString::<MAX_PRINTF_MSG_SIZE>::new();
@@ -38,12 +41,18 @@
     }
     
     /// Enables output from the `printf!` macro and the underlying `OS_printf` function.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_printf_enable`.
     pub fn printf_enable() {
         unsafe {
             ffi::OS_printf_enable();
         }
     }
     
     /// Disables output from the `printf!` macro and the underlying `OS_printf` function.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_printf_disable`.
     pub fn printf_disable() {
         unsafe {
             ffi::OS_printf_disable();
--- a/src/os/app.rs
+++ b/src/os/app.rs
@@ -1,4 +1,7 @@
-    //! Safe wrappers for the top-level OSAL application lifecycle API.
-    //!
+//! Top-level OSAL application lifecycle API.
+//!
+//! # Note
+//!
     //! These functions are typically part of the BSP/runtime and not called by
     //! individual cFS applications, but are provided for completeness and for
     //! special cases like unit testing environments.
@@ -12,6 +15,9 @@
     
     /// Initializes the OS Abstraction Layer.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_API_Init`.
+    ///
     /// This must be called before any other OSAL routine. It is typically handled
     /// by the cFE Main entry point.
     pub fn api_init() -> Result<()> {
@@ -21,6 +27,9 @@
     
     /// Tears down and de-initializes the OSAL.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_API_Teardown`.
+    ///
     /// This will release all OS resources and is intended for testing or controlled
     /// shutdown scenarios.
     pub fn api_teardown() {
@@ -29,6 +38,9 @@
     
     /// A background thread implementation that waits for events.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_IdleLoop`.
+    ///
     /// This is typically called by the BSP main routine after all other initialization
     /// has taken place. It waits until `application_shutdown` is called.
     pub fn idle_loop() {
@@ -37,6 +49,9 @@
     
     /// Deletes all resources (tasks, queues, etc.) created in OSAL.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_DeleteAllObjects`.
+    ///
     /// This is useful for cleaning up during an orderly shutdown or for testing.
     pub fn delete_all_objects() {
         unsafe { ffi::OS_DeleteAllObjects() };
@@ -44,6 +59,9 @@
     
     /// Initiates an orderly shutdown of the OSAL application.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_ApplicationShutdown`.
+    ///
     /// This allows the task currently blocked in `idle_loop` to wake up and return.
     pub fn application_shutdown(should_shutdown: bool) {
         unsafe { ffi::OS_ApplicationShutdown(should_shutdown as u8) };
@@ -51,6 +69,9 @@
     
     /// Exits/aborts the entire application process immediately.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `OS_ApplicationExit`.
+    ///
     /// This function does not return and is typically only used in non-production
     /// scenarios like unit testing.
     pub fn application_exit(status: i32) -> ! {
--- a/src/os/fs.rs
+++ b/src/os/fs.rs
@@ -242,6 +242,10 @@
     }
     
     /// Changes the permission mode of a file.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the path is invalid or the underlying OS call fails.
     pub fn chmod(path: &str, mode: FileMode) -> Result<()> {
         let c_path = c_path_from_str(path)?;
         check(unsafe { ffi::OS_chmod(c_path.as_ptr(), mode.bits()) })?;
--- a/src/os/heap.rs
+++ b/src/os/heap.rs
@@ -1,4 +1,4 @@
-    //! Safe, idiomatic wrapper for querying OSAL heap statistics.
-    
+//! Safe, idiomatic wrapper for querying OSAL heap statistics.
+
     use crate::error::Result;
     use crate::ffi;
     use crate::status::check;
--- a/src/os/id.rs
+++ b/src/os/id.rs
@@ -1,4 +1,7 @@
-    //! Safe, idiomatic wrappers for generic OSAL object ID APIs.
-    
+//! Generic OSAL object ID APIs.
+//!
+//! This module provides utilities for working with generic `OsalId`s, which
+//! are the underlying type for all OSAL resources (tasks, queues, mutexes, etc.).
+
     use crate::error::{Error, Result};
     use crate::ffi::{self, OS_OBJECT_CREATOR_ANY};
     use crate::status::check;
@@ -69,6 +72,12 @@
     }
     
     /// Retrieves the name of any valid OSAL object ID.
+    ///
+    /// # Errors
+    ///
+    /// Returns an error if the `object_id` is invalid, the buffer is too small
+    /// (unlikely with `heapless`), or the name is not valid UTF-8.
     pub fn get_resource_name(object_id: OsalId) -> Result<CString<{ ffi::OS_MAX_API_NAME as usize }>> {
         let mut buffer = [0u8; ffi::OS_MAX_API_NAME as usize];
         check(unsafe {
@@ -87,6 +96,11 @@
     }
     
     /// Converts an abstract ID into a zero-based integer suitable for use as an array index.
+    ///
+    /// # Errors
+    ///
+    /// Returns `Error::OsErrInvalidId` if the provided `object_id` does not map to a
+    /// valid index for its type.
     pub fn convert_to_array_index(object_id: OsalId) -> Result<u32> {
         let mut index = MaybeUninit::uninit();
         check(unsafe { ffi::OS_ConvertToArrayIndex(object_id.0, index.as_mut_ptr()) })?;
@@ -94,6 +108,12 @@
     }
     
     /// Converts an abstract ID of a specific type into a zero-based integer.
+    ///
+    /// # Errors
+    ///
+    /// Returns `Error::OsErrInvalidId` if the provided `object_id` is not of the
+    /// specified `obj_type` or does not map to a valid index.
     pub fn object_id_to_array_index(obj_type: ObjectType, object_id: OsalId) -> Result<u32> {
         let mut index = MaybeUninit::uninit();
         check(unsafe {
--- a/src/os/mod.rs
+++ b/src/os/mod.rs
@@ -1,3 +1,7 @@
+//! OSAL (Operating System Abstraction Layer) interface.
+//!
+//! This module provides safe, idiomatic Rust wrappers for the OSAL API, which
+//! abstracts away the details of the underlying real-time operating system (RTOS).
+
     pub mod app;
     pub mod fs;
     pub mod heap;
--- a/src/psp/mod.rs
+++ b/src/psp/mod.rs
@@ -1,13 +1,15 @@
-    //! Safe, idiomatic wrappers for the cFE Platform Support Package (PSP) API.
-    //!
-    //! The PSP provides an abstraction layer between the OSAL and the specific
-    //! hardware and board support package. These wrappers expose some of the
-    //! more common and useful PSP functions to applications.
-    
+//! CFE Platform Support Package (PSP) interface.
+//!
+//! The PSP provides the lowest-level abstraction layer, interacting directly with
+//! the hardware and board support package (BSP). These wrappers expose some of
+//! the more common and useful PSP functions to applications, but many are `unsafe`
+//! due to their low-level nature.
+
     use core::ffi::CStr;
     
     use heapless::CString;
     
     use crate::error::Error;
     use crate::error::Result;
     use crate::ffi;
     
     pub mod cds;
     pub mod eeprom;
     pub mod exception;
@@ -20,12 +22,17 @@
     /// Flushes processor data or instruction caches for a given memory range.
     ///
     /// # Safety
+    ///
     /// Flushing caches can have significant system-wide effects. The address and
     /// size must correspond to a valid memory region.
     pub unsafe fn flush_caches(cache_type: u32, address: *mut (), size: u32) {
         ffi::CFE_PSP_FlushCaches(cache_type, address as *mut _, size);
     }
     
     /// Returns the PSP-defined processor name.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_PSP_GetProcessorName`.
     pub fn get_processor_name() -> &'static str {
         unsafe {
             CStr::from_ptr(ffi::CFE_PSP_GetProcessorName())
@@ -34,6 +41,12 @@
     }
     
     /// Converts a PSP status code to its symbolic name.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_PSP_StatusToString`.
+    ///
+    /// # Errors
+    /// Returns an error if the resulting string is too long for the internal buffer.
     pub fn status_to_string(
         status: i32,
     ) -> Result<CString<{ ffi::CFE_PSP_STATUS_STRING_LENGTH as usize }>> {
--- a/src/psp/restart.rs
+++ b/src/psp/restart.rs
@@ -1,13 +1,18 @@
-    //! Wrappers for PSP restart and reset functions.
-    
+//! PSP restart and reset functions.
+
     use crate::cfe::es::system::ResetSubtype;
     use crate::ffi;
     use core::mem::MaybeUninit;
     
     /// Requests the PSP to restart the processor. This function does not return.
     ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_PSP_Restart`.
+    ///
     /// # Arguments
     /// * `reset_type`: The type of reset to perform (e.g., `ffi::CFE_PSP_RST_TYPE_PROCESSOR`).
     pub fn restart(reset_type: u32) -> ! {
         unsafe {
             ffi::CFE_PSP_Restart(reset_type);
@@ -17,6 +22,9 @@
     }
     
     /// Returns the last reset type and subtype recorded by the PSP.
+    ///
+    /// # C-API Mapping
+    /// This is a safe wrapper for `CFE_PSP_GetRestartType`.
     pub fn get_restart_type() -> (u32, ResetSubtype) {
         let mut subtype = MaybeUninit::uninit();
         let reset_type = unsafe { ffi::CFE_PSP_GetRestartType(subtype.as_mut_ptr()) };
--- a/src/status.rs
+++ b/src/status.rs
@@ -1,9 +1,14 @@
-    use crate::error::Error;
-    
+//! CFE informational status code handling.
+//!
+//! While error conditions are represented by the `Error` enum, cFE APIs can also
+//! return a variety of non-error "informational" status codes. This module
+//! provides the `Status` enum to represent these successful-but-noteworthy
+//! outcomes, and a `check` function to triage a raw `CFE_Status_t` into either
+//! a `Result<Status, Error>`.
+
     use crate::ffi;
     
     pub enum Status {
         /// Command was processed successfully.
         Success,
--- a/src/cfe/es/resource.rs
+++ b/src/cfe/es/resource.rs
@@ -1,3 +1,11 @@
+//! Safe wrappers for generic CFE Resource ID functions.
+//!
+//! This module provides utilities for introspecting generic `CFE_ResourceId_t`
+//! values, which are the underlying type for various specific IDs like `AppId`,
+//! `LibId`, `CounterId`, etc.
+//!
+//! It allows for converting specific IDs into the generic `ResourceId` and
+//! querying information about them in a type-agnostic way.
 //! Safe wrappers for generic CFE Resource ID functions.
     
     use crate::cfe::es::app::AppId;
@@ -8,7 +16,10 @@
     use crate::status::check;
     use core::mem::MaybeUninit;
     
-    /// A generic, type-safe wrapper for a CFE Resource ID.
+    /// A generic, type-safe wrapper for a `CFE_ResourceId_t`.
+    ///
+    /// This can represent any CFE resource, such as an application, library,
+    /// counter, etc.
     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
     #[repr(transparent)]
     pub struct ResourceId(pub ffi::CFE_ResourceId_t);
@@ -26,17 +37,36 @@
     }
     
     /// Gets the base value (type/category) from a resource ID value.
+    ///
+    /// This masks out the ID serial number to obtain the base value, which is
+    /// different for each resource type (e.g., all App IDs share the same base).
+    ///
+    /// # C-API Mapping
+    /// This is a wrapper for `CFE_ResourceId_GetBase`.
     pub fn get_base(resource_id: ResourceId) -> u32 {
         unsafe { ffi::CFE_ResourceId_GetBase(resource_id.0) }
     }
     
     /// Gets the serial number from a resource ID value.
+    ///
+    /// This masks out the ID base value to obtain the unique serial number for
+    /// this specific resource instance.
+    ///
+    /// # C-API Mapping
+    /// This is a wrapper for `CFE_ResourceId_GetSerial`.
     pub fn get_serial(resource_id: ResourceId) -> u32 {
         unsafe { ffi::CFE_ResourceId_GetSerial(resource_id.0) }
     }
     
     /// Retrieves information about an Application or Library given a specified Resource ID.
     ///
-    /// This is a generic wrapper that can be used for either an `AppId` or a `LibId`.
+    /// This is a generic wrapper that inspects the resource ID and calls the
+    /// appropriate underlying function (`CFE_ES_GetAppInfo` or `CFE_ES_GetLibInfo`).
+    ///
+    /// # Errors
+    /// Returns an error if the resource ID is not a valid App or Library ID, or if
+    /// the underlying CFE call fails.
     pub fn get_module_info(res_id: ResourceId) -> Result<AppInfo> {
         let mut module_info_uninit = MaybeUninit::uninit();
         check(unsafe { ffi::CFE_ES_GetModuleInfo(module_info_uninit.as_mut_ptr(), res_id.0) })?;
```
