//! TBL (Table Services) interface.

use crate::cfe::sb::msg::MsgId;
use crate::cfe::time::SysTime;
use crate::error::{CfsError, Result, TblError};
use crate::cstring;
use crate::status::check;
use crate::{ffi, status};
use core::ffi::c_void;
use core::marker::PhantomData;
use core::mem::{size_of, MaybeUninit};
use core::ops::{Deref, Drop};

/// A type alias for the callback function used to validate table loads.
///
/// The function receives a pointer to the table data to be validated. It should
/// return `CFE_SUCCESS` if the data is valid, or a negative error code otherwise.
pub type ValidationFn = ffi::CFE_TBL_CallbackFuncPtr_t;

/// A handle to a cFE table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TableHandle(pub ffi::CFE_TBL_Handle_t);

/// A handle to a cFE table, generic over the table's data type `T`.
///
/// This struct handles registration and unregistration of the table, providing
/// a safe, RAII-based interface.
#[derive(Debug)]
pub struct Table<T: Sized> {
    handle: TableHandle,
    is_owner: bool,
    _phantom: PhantomData<T>,
}

use bitflags::bitflags;

bitflags! {
    /// Options for table registration.
    pub struct TableOptions: u16 {
        /// Default table options (single-buffered, load/dump enabled).
        const DEFAULT         = ffi::CFE_TBL_OPT_DEFAULT as u16;
        /// The table will use a single buffer. Updates are copied from a shared working buffer.
        const SINGLE_BUFFERED = ffi::CFE_TBL_OPT_SNGL_BUFFER as u16;
        /// The table will have two dedicated buffers (active and inactive) for faster updates.
        const DOUBLE_BUFFERED = ffi::CFE_TBL_OPT_DBL_BUFFER as u16;
        /// The table's contents can be dumped but not loaded via Table Services commands.
        const DUMP_ONLY       = ffi::CFE_TBL_OPT_DUMP_ONLY as u16;
        /// The table is critical and its contents will be preserved in the Critical Data Store (CDS).
        const CRITICAL        = ffi::CFE_TBL_OPT_CRITICAL as u16;
    }
}

/// Information about a cFE table.
pub struct TableInfo(pub(crate) ffi::CFE_TBL_Info_t);

impl TableInfo {
    /// Returns the size of the table in bytes.
    pub fn size(&self) -> usize {
        self.0.Size as usize
    }

    /// Returns the number of applications with access to the table.
    pub fn num_users(&self) -> u32 {
        self.0.NumUsers
    }

    /// Returns the file creation time from the last file loaded into the table.
    pub fn file_time(&self) -> SysTime {
        SysTime(self.0.FileTime)
    }

    /// Returns the most recently calculated CRC of the table contents.
    pub fn crc(&self) -> u32 {
        self.0.Crc
    }

    /// Returns the time when the table was last updated.
    pub fn time_of_last_update(&self) -> SysTime {
        SysTime(self.0.TimeOfLastUpdate)
    }

    /// Returns whether the table has been loaded at least once.
    pub fn table_loaded_once(&self) -> bool {
        self.0.TableLoadedOnce
    }

    /// Returns whether the table is marked as Dump Only.
    pub fn dump_only(&self) -> bool {
        self.0.DumpOnly
    }

    /// Returns whether the table is double-buffered.
    pub fn double_buffered(&self) -> bool {
        self.0.DoubleBuffered
    }

    /// Returns whether the table address was defined by the owner application.
    pub fn user_def_addr(&self) -> bool {
        self.0.UserDefAddr
    }

    /// Returns whether the table is critical (backed by the CDS).
    pub fn critical(&self) -> bool {
        self.0.Critical
    }

    /// Returns the filename of the last file loaded into the table.
    pub fn last_file_loaded(&self) -> &str {
        let len = self
            .0
            .LastFileLoaded
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(self.0.LastFileLoaded.len());
        let bytes = &self.0.LastFileLoaded[..len];
        let u8slice = unsafe { core::slice::from_raw_parts(bytes.as_ptr() as *const u8, len) };
        core::str::from_utf8(u8slice).unwrap_or("")
    }
}

impl<T: Sized> Table<T> {
    /// Registers a new table with cFE Table Services.
    ///
    /// This call can block. Must not be called from ISR context.
    ///
    /// # Arguments
    /// * `name`: The application-local name for the table.
    /// * `options`: Bitwise-ORed flags for table options (e.g., `TableOptions::DEFAULT`).
    /// * `validation_fn`: An optional callback function to validate table loads.
    pub fn new(name: &str, options: TableOptions, validation_fn: ValidationFn) -> Result<Self> {
        let mut handle = MaybeUninit::uninit();
        let c_name = cstring::<{ ffi::CFE_MISSION_TBL_MAX_NAME_LENGTH as usize }>(name)
            .map_err(|_| CfsError::Tbl(TblError::InvalidName))?;

        let status = unsafe {
            ffi::CFE_TBL_Register(
                handle.as_mut_ptr(),
                c_name.as_ptr(),
                size_of::<T>(),
                options.bits(),
                validation_fn,
            )
        };
        check(status)?;

        Ok(Self {
            handle: TableHandle(unsafe { handle.assume_init() }),
            is_owner: true,
            _phantom: PhantomData,
        })
    }

    /// Obtains a handle to a table registered by another application.
    ///
    /// This does not take ownership of the table. When this `Table` instance is dropped,
    /// it only releases the shared handle; it does not unregister the table itself.
    ///
    /// # Arguments
    /// * `name`: The full name of the table, in the format "AppName.TableName".
    pub fn share(name: &str) -> Result<Self> {
        let mut handle = MaybeUninit::uninit();
        let c_name = cstring::<{ ffi::CFE_MISSION_TBL_MAX_FULL_NAME_LEN as usize }>(name)
            .map_err(|_| CfsError::Tbl(TblError::InvalidName))?;

        let status = unsafe { ffi::CFE_TBL_Share(handle.as_mut_ptr(), c_name.as_ptr()) };
        check(status)?;

        Ok(Self {
            handle: TableHandle(unsafe { handle.assume_init() }),
            is_owner: false,
            _phantom: PhantomData,
        })
    }

    /// Loads data into the table from a file.
    ///
    /// This call can block. Must not be called from ISR context.
    pub fn load_from_file(&self, filename: &str) -> Result<()> {
        let c_filename = cstring::<{ ffi::OS_MAX_PATH_LEN as usize }>(filename)
            .map_err(|_| CfsError::Tbl(TblError::FilenameTooLong))?;
        let status = unsafe {
            ffi::CFE_TBL_Load(
                self.handle.0,
                ffi::CFE_TBL_SrcEnum_CFE_TBL_SRC_FILE,
                c_filename.as_ptr() as *const _,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Loads data into the table from a memory slice.
    ///
    /// This call can block. Must not be called from ISR context.
    pub fn load_from_slice(&self, data: &[T]) -> Result<()> {
        let status = unsafe {
            ffi::CFE_TBL_Load(
                self.handle.0,
                ffi::CFE_TBL_SrcEnum_CFE_TBL_SRC_ADDRESS,
                data.as_ptr() as *const _,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Performs periodic processing for the table (update, validate, dump).
    /// This should be called once per application cycle for each owned table.
    pub fn manage(&self) -> Result<()> {
        check(unsafe { ffi::CFE_TBL_Manage(self.handle.0) })?;
        Ok(())
    }

    /// Notifies Table Services that the application has modified the table's contents.
    /// This is important for critical tables backed by the CDS.
    pub fn modified(&self) -> Result<()> {
        check(unsafe { ffi::CFE_TBL_Modified(self.handle.0) })?;
        Ok(())
    }

    /// Gets a read-only accessor to the table's data.
    /// The accessor locks the table and automatically releases it when dropped.
    pub fn get_accessor(&self) -> Result<TableAccessor<'_, T>> {
        TableAccessor::new(self.handle)
    }

    /// Returns the underlying `TableHandle`.
    pub fn handle(&self) -> TableHandle {
        self.handle
    }

    /// Obtains characteristics and information about a specified table by name.
    ///
    /// # Arguments
    /// * `name`: The full name of the table, in the format "AppName.TableName".
    pub fn get_info(name: &str) -> Result<TableInfo> {
        let c_name = cstring::<{ ffi::CFE_MISSION_TBL_MAX_FULL_NAME_LEN as usize }>(name)
            .map_err(|_| CfsError::Tbl(TblError::InvalidName))?;

        let mut tbl_info_uninit = MaybeUninit::uninit();

        check(unsafe { ffi::CFE_TBL_GetInfo(tbl_info_uninit.as_mut_ptr(), c_name.as_ptr()) })?;

        Ok(TableInfo(unsafe { tbl_info_uninit.assume_init() }))
    }

    /// Obtains the current status of pending actions for a table.
    pub fn status(&self) -> Result<status::Status> {
        check(unsafe { ffi::CFE_TBL_GetStatus(self.handle.0) })
    }

    /// Updates the contents of the table if an update is pending.
    pub fn update(&self) -> Result<()> {
        check(unsafe { ffi::CFE_TBL_Update(self.handle.0) })?;
        Ok(())
    }

    /// Validates the contents of a table if a validation is pending.
    pub fn validate(&self) -> Result<()> {
        check(unsafe { ffi::CFE_TBL_Validate(self.handle.0) })?;
        Ok(())
    }

    /// Copies the contents of a Dump Only Table to a shared buffer.
    ///
    /// This should only be called by the table owner in response to a dump request,
    /// typically after `manage()` returns `Ok(TblInfoDumpPending)`.
    pub fn dump_to_buffer(&self) -> Result<()> {
        check(unsafe { ffi::CFE_TBL_DumpToBuffer(self.handle.0) })?;
        Ok(())
    }

    /// Instructs Table Services to send a message when this table requires management.
    ///
    /// This allows an application to be event-driven for table maintenance instead of
    /// polling with `manage()`.
    ///
    /// # Arguments
    /// * `msg_id`: Message ID to be used in the notification message.
    /// * `command_code`: Command code to be placed in the secondary header.
    /// * `parameter`: Application-defined value to be passed as a parameter in the message.
    pub fn notify_by_message(
        &self,
        msg_id: MsgId,
        command_code: u16,
        parameter: u32,
    ) -> Result<()> {
        check(unsafe {
            ffi::CFE_TBL_NotifyByMessage(self.handle.0, msg_id.0, command_code, parameter)
        })?;
        Ok(())
    }

    /// Gets read-only accessors for multiple tables at once.
    ///
    /// # Safety
    /// The caller must ensure that the types `U` in the returned accessors match
    /// the actual types of the tables identified by the handles.
    pub unsafe fn get_accessors<const N: usize>(
        handles: [TableHandle; N],
    ) -> Result<[TableAccessor<'static, ()>; N]> {
        let mut ptrs: [*mut c_void; N] = [core::ptr::null_mut(); N];
        let mut ptr_ptrs: [*mut *mut c_void; N] = [core::ptr::null_mut(); N];
        for i in 0..N {
            ptr_ptrs[i] = &raw mut ptrs[i];
        }
        check(ffi::CFE_TBL_GetAddresses(
            ptr_ptrs.as_mut_ptr(),
            N as u16,
            handles.as_ptr() as *const _,
        ))?;

        let mut accessors: [MaybeUninit<TableAccessor<'static, ()>>; N] =
            MaybeUninit::uninit().assume_init();

        for i in 0..N {
            accessors[i].write(TableAccessor {
                ptr: ptrs[i] as *const (),
                handle: handles[i],
                _phantom: PhantomData,
            });
        }

        Ok(accessors.map(|a| a.assume_init()))
    }
}

impl<T: Sized> Drop for Table<T> {
    /// Unregisters the table if this instance is the owner.
    fn drop(&mut self) {
        // Only the original registrant should unregister the table.
        // Shared handles are simply released without unregistering.
        if self.is_owner {
            let _ = unsafe { ffi::CFE_TBL_Unregister(self.handle.0) };
        }
    }
}

/// A safe RAII wrapper for accessing a CFE table's memory.
///
/// It acquires the table pointer on creation and automatically releases it when dropped.
#[derive(Debug)]
pub struct TableAccessor<'a, T: 'a> {
    ptr: *const T,
    handle: TableHandle,
    _phantom: PhantomData<&'a T>,
}

impl<'a, T> TableAccessor<'a, T> {
    /// Acquires a pointer to the table data.
    ///
    /// Can block on shared single-buffered tables. The address must
    /// be released (by dropping the accessor) before calling
    /// [`Table::update`] or any blocking call (e.g. pending on SB).
    ///
    /// Returns a zeroed table pointer if the table has never been
    /// loaded.
    pub fn new(handle: TableHandle) -> Result<Self> {
        let mut ptr = core::ptr::null_mut();
        let status = unsafe { ffi::CFE_TBL_GetAddress(&mut ptr, handle.0) };
        // CFE_TBL_INFO_UPDATED is a success code, but indicates a change.
        if status != ffi::CFE_SUCCESS && status != ffi::CFE_TBL_INFO_UPDATED {
            return Err(CfsError::from(status));
        }

        Ok(Self {
            ptr: ptr as *const T,
            handle,
            _phantom: PhantomData,
        })
    }
}

impl<'a, T> Deref for TableAccessor<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<'a, T> Drop for TableAccessor<'a, T> {
    fn drop(&mut self) {
        // Automatically release the address when the accessor goes out of scope.
        let _ = unsafe { ffi::CFE_TBL_ReleaseAddress(self.handle.0) };
    }
}
