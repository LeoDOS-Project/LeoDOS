//! Safe, idiomatic wrappers for CFE File Services (CFE_FS) and OSAL file APIs.
//!
//! This module provides a `File` struct that acts as a safe handle for
//! file operations, ensuring that files are closed when they go out of scope.
//! It also provides wrappers for standalone filesystem operations like `mkdir` and `remove`.

use crate::cfe::time::SysTime;
use crate::error::{Error, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::os::time::OsTime;
use crate::os::util::c_path_from_str;
use crate::status::{check, Status};
use bitflags::bitflags;
use core::ffi::CStr;
use core::mem::MaybeUninit;
use core::ops::Drop;
use heapless::CString;

bitflags! {
    /// File permission modes for `fs::chmod`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct FileMode: u32 {
        /// Read permission.
        const READ = ffi::OS_FILESTAT_MODE_READ;
        /// Write permission.
        const WRITE = ffi::OS_FILESTAT_MODE_WRITE;
        /// Execute permission.
        const EXEC = ffi::OS_FILESTAT_MODE_EXEC;
    }
}

/// Generalized file types/categories known to FS.
///
/// This is used by `parse_input_filename` to apply default paths and extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum FileCategory {
    /// An unknown or unspecified file category.
    Unknown = ffi::CFE_FS_FileCategory_t_CFE_FS_FileCategory_UNKNOWN,
    /// A dynamically loadable module file (e.g., .so).
    DynamicModule = ffi::CFE_FS_FileCategory_t_CFE_FS_FileCategory_DYNAMIC_MODULE,
    /// A binary data dump file.
    BinaryDataDump = ffi::CFE_FS_FileCategory_t_CFE_FS_FileCategory_BINARY_DATA_DUMP,
    /// A text-based log file.
    TextLog = ffi::CFE_FS_FileCategory_t_CFE_FS_FileCategory_TEXT_LOG,
    /// A script file (e.g., an ES startup script).
    Script = ffi::CFE_FS_FileCategory_t_CFE_FS_FileCategory_SCRIPT,
    /// A temporary or ephemeral file.
    Temp = ffi::CFE_FS_FileCategory_t_CFE_FS_FileCategory_TEMP,
}

// Add this struct at the top
/// Metadata and state for a background file write operation.
///
/// This struct should be stored in a persistent memory location (e.g., a `static`)
/// for the duration of the asynchronous file write.
#[repr(transparent)]
pub struct FileWriteMetaData(pub ffi::CFE_FS_FileWriteMetaData_t);

/// A structure containing metadata about a file.
#[derive(Debug, Clone, Copy)]
pub struct FileStat {
    inner: ffi::os_fstat_t,
}

impl FileStat {
    /// Returns the size of the file in bytes.
    pub fn size(&self) -> usize {
        self.inner.FileSize
    }

    /// Returns `true` if this metadata is for a directory.
    pub fn is_dir(&self) -> bool {
        (self.inner.FileModeBits & ffi::OS_FILESTAT_MODE_DIR) != 0
    }
}

/// A safe, idiomatic wrapper for a cFE file header.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct FsHeader(pub ffi::CFE_FS_Header_t);

/// Statistics about the overall file system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsInfo {
    /// Total number of file descriptors.
    pub max_fds: u32,
    /// Total number that are free.
    pub free_fds: u32,
    /// Maximum number of volumes.
    pub max_volumes: u32,
    /// Total number of volumes free.
    pub free_volumes: u32,
}

impl From<ffi::os_fsinfo_t> for FsInfo {
    fn from(info: ffi::os_fsinfo_t) -> Self {
        Self {
            max_fds: info.MaxFds,
            free_fds: info.FreeFds,
            max_volumes: info.MaxVolumes,
            free_volumes: info.FreeVolumes,
        }
    }
}

impl FsHeader {
    /// Creates and initializes a new cFE file header with the specified description and subtype.
    ///
    /// The description will be truncated if it is longer than the header field allows.
    pub fn new(description: &str, subtype: u32) -> Result<Self> {
        let mut header_uninit = MaybeUninit::<ffi::CFE_FS_Header_t>::uninit();

        // CFE_FS_HDR_DESC_MAX_LEN includes the null terminator.
        let mut c_desc = CString::<{ ffi::CFE_FS_HDR_DESC_MAX_LEN as usize }>::new();
        // This will truncate if the description is too long, which is acceptable.
        c_desc
            .extend_from_bytes(description.as_bytes())
            .map_err(|_| Error::OsFsErrPathTooLong)?;

        unsafe {
            // CFE_FS_InitHeader takes a pointer and initializes the memory it points to.
            ffi::CFE_FS_InitHeader(header_uninit.as_mut_ptr(), c_desc.as_ptr(), subtype);
            // Now that the C function has initialized it, we can safely assume it's initialized.
            Ok(Self(header_uninit.assume_init()))
        }
    }
}

/// Defines the origin for a seek operation, used by `File::seek`.
#[derive(Debug, Clone, Copy)]
pub enum SeekFrom {
    /// Seek from the beginning of the stream.
    Start(u32),
    /// Seek from the end of the stream.
    End(i32),
    /// Seek from the current position.
    Current(i32),
}

/// A structure containing statistics about a filesystem volume.
#[derive(Debug, Clone, Copy)]
pub struct StatVfs {
    inner: ffi::OS_statvfs_t,
}

impl StatVfs {
    /// Returns the fundamental file system block size.
    pub fn block_size(&self) -> usize {
        self.inner.block_size
    }

    /// Returns the total number of blocks in the file system.
    pub fn total_blocks(&self) -> ffi::osal_blockcount_t {
        self.inner.total_blocks
    }

    /// Returns the number of free blocks available.
    pub fn blocks_free(&self) -> ffi::osal_blockcount_t {
        self.inner.blocks_free
    }
}

/// Properties of an open file, returned by `File::info`.
#[derive(Debug, Clone)]
pub struct FileProp {
    /// The full path of the open file.
    pub path: CString<{ ffi::OS_MAX_PATH_LEN as usize }>,
    /// The OSAL ID of the task that opened the file.
    pub user: OsalId,
    /// Indicates if the file descriptor is valid.
    pub is_valid: bool,
}

/// A handle to an open directory.
///
/// This is an iterator that yields the entries within the directory. It is
/// a wrapper around an `osal_id_t` that will automatically call `OS_DirectoryClose` when
/// it goes out of scope, preventing resource leaks.
#[derive(Debug)]
pub struct Directory {
    id: ffi::osal_id_t,
}

impl Directory {
    /// Opens a directory for reading.
    pub fn open(path: &str) -> Result<Self> {
        let c_path = c_path_from_str(path)?;
        let mut dir_id = MaybeUninit::uninit();
        check(unsafe { ffi::OS_DirectoryOpen(dir_id.as_mut_ptr(), c_path.as_ptr()) })?;
        Ok(Self {
            id: unsafe { dir_id.assume_init() },
        })
    }

    /// Rewinds the directory stream to the beginning.
    pub fn rewind(&mut self) -> Result<()> {
        check(unsafe { ffi::OS_DirectoryRewind(self.id) })?;
        Ok(())
    }
}

impl Iterator for Directory {
    type Item = Result<CString<{ ffi::OS_MAX_FILE_NAME as usize }>>;

    /// Reads the next directory entry.
    ///
    /// Returns `Some(Ok(name))` for the next entry, `None` when the directory
    /// has been fully read, or `Some(Err(e))` if an error occurs.
    fn next(&mut self) -> Option<Self::Item> {
        let mut dirent = MaybeUninit::<ffi::os_dirent_t>::uninit();
        let status = check(unsafe { ffi::OS_DirectoryRead(self.id, dirent.as_mut_ptr()) });

        match status {
            Ok(_) => {
                let dirent = unsafe { dirent.assume_init() };
                let c_str = unsafe { CStr::from_ptr(dirent.FileName.as_ptr()) };
                let mut s = CString::new();
                match s.extend_from_bytes(c_str.to_bytes()) {
                    Ok(_) => Some(Ok(s)),
                    Err(_) => Some(Err(Error::OsErrNameTooLong)),
                }
            }
            Err(Error::OsError) => None,
            Err(err) => Some(Err(err)),
        }
    }
}

impl Drop for Directory {
    /// Closes the directory when the `Directory` object goes out of scope.
    fn drop(&mut self) {
        // OS_DirectoryClose returns an osal_status_t, which can be an error.
        // But we ignore it in drop, as with other resources in this crate.
        let _ = unsafe { ffi::OS_DirectoryClose(self.id) };
    }
}

/// A handle to an open file.
///
/// This is a wrapper around an `osal_id_t` that will automatically call
/// `OS_close` when it goes out of scope, preventing resource leaks.
#[derive(Debug)]
pub struct File {
    id: OsalId,
}

/// File access modes for opening or creating a file.
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum AccessMode {
    /// Open for reading only.
    ReadOnly = ffi::OS_READ_ONLY,
    /// Open for writing only.
    WriteOnly = ffi::OS_WRITE_ONLY,
    /// Open for reading and writing.
    ReadWrite = ffi::OS_READ_WRITE,
}

impl File {
    /// Returns the underlying OSAL file ID.
    pub fn id(&self) -> OsalId {
        self.id
    }
    /// Opens a file with the specified access mode.
    ///
    /// # Arguments
    /// * `path`: The virtual path to the file (e.g., "/ram/my_file.txt").
    /// * `access`: The access mode, e.g., `ffi::OS_READ_ONLY`, `ffi::OS_WRITE_ONLY`,
    ///   or `ffi::OS_READ_WRITE`.
    pub fn open(path: &str, access: AccessMode) -> Result<Self> {
        let c_path = c_path_from_str(path)?;
        let mut file_id = MaybeUninit::uninit();
        let status = unsafe {
            ffi::OS_OpenCreate(
                file_id.as_mut_ptr(),
                c_path.as_ptr(),
                ffi::OS_file_flag_t_OS_FILE_FLAG_NONE as i32,
                access as i32,
            )
        };
        check(status)?;
        Ok(Self {
            id: OsalId(unsafe { file_id.assume_init() }),
        })
    }

    /// Creates a new file, or truncates an existing one, with read/write access.
    ///
    /// # Arguments
    /// * `path`: The virtual path to the file to create (e.g., "/ram/new_file.dat").
    pub fn create(path: &str) -> Result<Self> {
        let c_path = c_path_from_str(path)?;
        let mut file_id = MaybeUninit::uninit();
        let flags = ffi::OS_file_flag_t_OS_FILE_FLAG_CREATE as i32
            | ffi::OS_file_flag_t_OS_FILE_FLAG_TRUNCATE as i32;

        let status = unsafe {
            ffi::OS_OpenCreate(
                file_id.as_mut_ptr(),
                c_path.as_ptr(),
                flags,
                ffi::OS_READ_WRITE as i32,
            )
        };
        check(status)?;
        Ok(Self {
            id: OsalId(unsafe { file_id.assume_init() }),
        })
    }

    /// Reads some bytes from the file into the specified buffer.
    ///
    /// Returns the number of bytes read.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let bytes_read = unsafe { ffi::OS_read(self.id.0, buf.as_mut_ptr() as *mut _, buf.len()) };
        if bytes_read < 0 {
            Err(Error::from(bytes_read))
        } else {
            Ok(bytes_read as usize)
        }
    }

    /// Writes a buffer to the file.
    ///
    /// Returns the number of bytes written.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let bytes_written =
            unsafe { ffi::OS_write(self.id.0, buf.as_ptr() as *const _, buf.len()) };
        if bytes_written < 0 {
            Err(Error::from(bytes_written))
        } else {
            Ok(bytes_written as usize)
        }
    }

    /// Seeks to an offset, in bytes, in a stream.
    ///
    /// Returns the new position from the start of the file.
    pub fn seek(&mut self, pos: SeekFrom) -> Result<u32> {
        let (offset, whence) = match pos {
            SeekFrom::Start(offset) => (offset as i32, ffi::OS_SEEK_SET),
            SeekFrom::End(offset) => (offset, ffi::OS_SEEK_END),
            SeekFrom::Current(offset) => (offset, ffi::OS_SEEK_CUR),
        };

        let new_pos = unsafe { ffi::OS_lseek(self.id.0, offset, whence) };
        if new_pos < 0 {
            Err(Error::from(new_pos))
        } else {
            Ok(new_pos as u32)
        }
    }

    /// Reads the standard cFE File Header from the file.
    pub fn read_header(&mut self) -> Result<FsHeader> {
        let mut hdr = MaybeUninit::uninit();
        let status = unsafe { ffi::CFE_FS_ReadHeader(hdr.as_mut_ptr(), self.id.0) };
        check(status)?;
        Ok(FsHeader(unsafe { hdr.assume_init() }))
    }

    /// Writes the standard cFE File Header to the file.
    pub fn write_header(&mut self, hdr: &mut FsHeader) -> Result<()> {
        let status = unsafe { ffi::CFE_FS_WriteHeader(self.id.0, &mut hdr.0) };
        check(status)?;
        Ok(())
    }

    /// Modifies the timestamp field in the cFE File Header.
    pub fn set_timestamp(&mut self, time: SysTime) -> Result<()> {
        let status = unsafe { ffi::CFE_FS_SetTimestamp(self.id.0, time.0) };
        check(status)?;
        Ok(())
    }

    /// Retrieves information about this open file.
    pub fn info(&self) -> Result<FileProp> {
        let mut prop = MaybeUninit::<ffi::OS_file_prop_t>::uninit();
        check(unsafe { ffi::OS_FDGetInfo(self.id.0, prop.as_mut_ptr()) })?;
        let prop = unsafe { prop.assume_init() };

        let mut path_str = CString::new();
        let c_str = unsafe { CStr::from_ptr(prop.Path.as_ptr()) };
        path_str
            .extend_from_bytes(c_str.to_bytes())
            .map_err(|_| Error::OsFsErrPathTooLong)?;

        Ok(FileProp {
            path: path_str,
            user: OsalId(prop.User),
            is_valid: prop.IsValid != 0,
        })
    }

    /// Reads from the file with a relative timeout.
    pub fn timed_read(&mut self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        let bytes_read = unsafe {
            ffi::OS_TimedRead(self.id.0, buf.as_mut_ptr() as *mut _, buf.len(), timeout_ms)
        };
        if bytes_read < 0 {
            Err(Error::from(bytes_read))
        } else {
            Ok(bytes_read as usize)
        }
    }

    /// Reads from the file with an absolute timeout.
    pub fn timed_read_abs(&mut self, buf: &mut [u8], abstime: OsTime) -> Result<usize> {
        let bytes_read = unsafe {
            ffi::OS_TimedReadAbs(self.id.0, buf.as_mut_ptr() as *mut _, buf.len(), abstime.0)
        };
        if bytes_read < 0 {
            Err(Error::from(bytes_read))
        } else {
            Ok(bytes_read as usize)
        }
    }

    /// Writes to the file with a relative timeout.
    pub fn timed_write(&mut self, buf: &[u8], timeout_ms: i32) -> Result<usize> {
        let bytes_written = unsafe {
            ffi::OS_TimedWrite(self.id.0, buf.as_ptr() as *const _, buf.len(), timeout_ms)
        };
        if bytes_written < 0 {
            Err(Error::from(bytes_written))
        } else {
            Ok(bytes_written as usize)
        }
    }

    /// Writes to the file with an absolute timeout.
    pub fn timed_write_abs(&mut self, buf: &[u8], abstime: OsTime) -> Result<usize> {
        let bytes_written = unsafe {
            ffi::OS_TimedWriteAbs(self.id.0, buf.as_ptr() as *const _, buf.len(), abstime.0)
        };
        if bytes_written < 0 {
            Err(Error::from(bytes_written))
        } else {
            Ok(bytes_written as usize)
        }
    }
}

impl Drop for File {
    /// Closes the file when the `File` object goes out of scope.
    fn drop(&mut self) {
        let _ = unsafe { ffi::OS_close(self.id.0) };
    }
}

// --- Standalone Functions ---

/// Retrieves metadata for a file or directory at a given path.
pub fn stat(path: &str) -> Result<FileStat> {
    let c_path = c_path_from_str(path)?;
    let mut filestats = MaybeUninit::uninit();
    check(unsafe { ffi::OS_stat(c_path.as_ptr(), filestats.as_mut_ptr()) })?;
    Ok(FileStat {
        inner: unsafe { filestats.assume_init() },
    })
}

/// Changes the permission mode of a file.
///
/// # Errors
///
/// Returns an error if the path is invalid or the underlying OS call fails.
pub fn chmod(path: &str, mode: FileMode) -> Result<()> {
    let c_path = c_path_from_str(path)?;
    check(unsafe { ffi::OS_chmod(c_path.as_ptr(), mode.bits()) })?;
    Ok(())
}
/// Removes a file from the file system.
pub fn remove(path: &str) -> Result<()> {
    let c_path = c_path_from_str(path)?;
    let status = unsafe { ffi::OS_remove(c_path.as_ptr()) };
    check(status)?;
    Ok(())
}

/// Renames a file.
pub fn rename(old: &str, new: &str) -> Result<()> {
    let c_old = c_path_from_str(old)?;
    let c_new = c_path_from_str(new)?;
    let status = unsafe { ffi::OS_rename(c_old.as_ptr(), c_new.as_ptr()) };
    check(status)?;
    Ok(())
}

/// Creates a new directory.
pub fn mkdir(path: &str) -> Result<()> {
    let c_path = c_path_from_str(path)?;
    // `access` is currently unused by OSAL but we pass a reasonable default.
    let status = unsafe { ffi::OS_mkdir(c_path.as_ptr(), ffi::OS_READ_WRITE) };
    check(status)?;
    Ok(())
}

/// Removes an empty directory.
pub fn rmdir(path: &str) -> Result<()> {
    let c_path = c_path_from_str(path)?;
    let status = unsafe { ffi::OS_rmdir(c_path.as_ptr()) };
    check(status)?;
    Ok(())
}

/// Copies a single file from `src` to `dest`.
pub fn cp(src: &str, dest: &str) -> Result<()> {
    let c_src = c_path_from_str(src)?;
    let c_dest = c_path_from_str(dest)?;
    check(unsafe { ffi::OS_cp(c_src.as_ptr(), c_dest.as_ptr()) })?;
    Ok(())
}

/// Moves a single file from `src` to `dest`.
///
/// This will first attempt a rename, and if that fails (e.g., across different
/// filesystems), it will fall back to a copy-then-delete operation.
pub fn mv(src: &str, dest: &str) -> Result<()> {
    let c_src = c_path_from_str(src)?;
    let c_dest = c_path_from_str(dest)?;
    check(unsafe { ffi::OS_mv(c_src.as_ptr(), c_dest.as_ptr()) })?;
    Ok(())
}

/// Retrieves statistics about a filesystem volume.
pub fn statvfs(path: &str) -> Result<StatVfs> {
    let c_path = c_path_from_str(path)?;
    let mut statbuf = MaybeUninit::uninit();
    check(unsafe { ffi::OS_FileSysStatVolume(c_path.as_ptr(), statbuf.as_mut_ptr()) })?;
    Ok(StatVfs {
        inner: unsafe { statbuf.assume_init() },
    })
}

/// Extracts the filename from a unix style path and filename string.
///
/// Returns a `&str` slice of the valid filename within the provided buffer.
pub fn extract_filename_from_path<'a>(
    original_path: &str,
    filename_buf: &'a mut [u8; ffi::OS_MAX_PATH_LEN as usize],
) -> Result<&'a str> {
    let c_path = c_path_from_str(original_path)?;
    let status = unsafe {
        ffi::CFE_FS_ExtractFilenameFromPath(
            c_path.as_ptr(),
            filename_buf.as_mut_ptr() as *mut libc::c_char,
        )
    };
    check(status)?;

    let c_str = unsafe { CStr::from_ptr(filename_buf.as_ptr() as *const libc::c_char) };
    c_str.to_str().map_err(|_| Error::InvalidString)
}

/// Parses a filename from an input string, applying default path and extension.
///
/// This function uses cFE's logic to construct a complete, platform-correct
/// file path. If the `input_name` omits a path or extension, defaults
/// appropriate for the `category` are applied.
///
/// # Arguments
/// * `output_buf`: A buffer to store the resulting fully-qualified path.
/// * `input_name`: The (potentially partial) filename to parse.
/// * `category`: The `FileCategory` which determines the default path and extension.
///
/// # Returns
/// On success, returns a `&str` slice of the valid, null-terminated path
/// within `output_buf`.
pub fn parse_input_filename<'a>(
    output_buf: &'a mut [u8; ffi::OS_MAX_PATH_LEN as usize],
    input_name: &str,
    category: FileCategory,
) -> Result<&'a str> {
    let c_input = c_path_from_str(input_name)?;
    let status = unsafe {
        ffi::CFE_FS_ParseInputFileName(
            output_buf.as_mut_ptr() as *mut libc::c_char,
            c_input.as_ptr(),
            output_buf.len(),
            category as u32,
        )
    };
    check(status)?;

    // Find the null terminator to determine the actual length of the output string.
    let len = output_buf.iter().position(|&b| b == 0).unwrap_or(0);
    core::str::from_utf8(&output_buf[..len]).map_err(|_| Error::InvalidString)
}

/// Checks if a file with the given name is currently open.
pub fn is_file_open(filename: &str) -> bool {
    if let Ok(c_path) = c_path_from_str(filename) {
        matches!(
            check(unsafe { ffi::OS_FileOpenCheck(c_path.as_ptr()) }),
            Ok(Status::Success)
        )
    } else {
        false
    }
}

/// Closes all files that were opened through OSAL.
pub fn close_all_files() -> Result<()> {
    check(unsafe { ffi::OS_CloseAllFiles() })?;
    Ok(())
}

/// Closes a file by its filename.
pub fn close_file_by_name(filename: &str) -> Result<()> {
    let c_path = c_path_from_str(filename)?;
    check(unsafe { ffi::OS_CloseFileByName(c_path.as_ptr()) })?;
    Ok(())
}

/// Creates a new file system on a block device or in memory.
pub fn make_fs(
    address: *mut u8,
    devname: &str,
    volname: &str,
    blocksize: usize,
    numblocks: usize,
) -> Result<()> {
    let c_dev = c_path_from_str(devname)?;
    let c_vol = c_path_from_str(volname)?;
    check(unsafe {
        ffi::OS_mkfs(
            address as *mut libc::c_char,
            c_dev.as_ptr(),
            c_vol.as_ptr(),
            blocksize,
            numblocks,
        )
    })?;
    Ok(())
}

/// Mounts a file system to a virtual mount point.
pub fn mount(devname: &str, mountpoint: &str) -> Result<()> {
    let c_dev = c_path_from_str(devname)?;
    let c_mount = c_path_from_str(mountpoint)?;
    check(unsafe { ffi::OS_mount(c_dev.as_ptr(), c_mount.as_ptr()) })?;
    Ok(())
}

/// Unmounts a file system.
pub fn unmount(mountpoint: &str) -> Result<()> {
    let c_mount = c_path_from_str(mountpoint)?;
    check(unsafe { ffi::OS_unmount(c_mount.as_ptr()) })?;
    Ok(())
}

/// Gets the physical drive name associated with a virtual mount point.
pub fn get_phys_drive_name(mountpoint: &str) -> Result<CString<{ ffi::OS_MAX_PATH_LEN as usize }>> {
    let c_mount = c_path_from_str(mountpoint)?;
    let mut buffer = [0u8; ffi::OS_MAX_PATH_LEN as usize];
    check(unsafe {
        ffi::OS_FS_GetPhysDriveName(buffer.as_mut_ptr() as *mut libc::c_char, c_mount.as_ptr())
    })?;
    let len = buffer.iter().position(|&b| b == 0).unwrap_or(0);
    let mut s = CString::new();
    s.extend_from_bytes(&buffer[..len])
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}

/// Translates an OSAL virtual path to a host-specific local path.
pub fn translate_path(
    virtual_path: &str,
) -> Result<CString<{ ffi::OS_MAX_LOCAL_PATH_LEN as usize }>> {
    let c_virt = c_path_from_str(virtual_path)?;
    let mut buffer = [0u8; ffi::OS_MAX_LOCAL_PATH_LEN as usize];
    check(unsafe { ffi::OS_TranslatePath(c_virt.as_ptr(), buffer.as_mut_ptr()) })?;
    let len = buffer.iter().position(|&b| b == 0).unwrap_or(0);
    let mut s = CString::new();
    s.extend_from_bytes(&buffer[..len])
        .map_err(|_| Error::OsErrNameTooLong)?;
    Ok(s)
}

/// Retrieves information about the overall file system.
pub fn get_fs_info() -> Result<FsInfo> {
    let mut info = MaybeUninit::uninit();
    check(unsafe { ffi::OS_GetFsInfo(info.as_mut_ptr()) })?;
    Ok(unsafe { info.assume_init() }.into())
}

/// Gets the default virtual mount point for a file category (e.g., "/ram" or "/cf").
pub fn get_default_mount_point(category: FileCategory) -> Option<&'static str> {
    let ptr = unsafe { ffi::CFE_FS_GetDefaultMountPoint(category as u32) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or(""))
    }
}

/// Gets the default filename extension for a file category (e.g., ".so" or ".log").
pub fn get_default_extension(category: FileCategory) -> Option<&'static str> {
    let ptr = unsafe { ffi::CFE_FS_GetDefaultExtension(category as u32) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or(""))
    }
}

/// Registers a background file dump request with Executive Services.
pub fn background_file_dump_request(meta: &mut FileWriteMetaData) -> Result<()> {
    check(unsafe { ffi::CFE_FS_BackgroundFileDumpRequest(&mut meta.0) })?;
    Ok(())
}

/// Checks if a background file dump request is currently pending.
pub fn is_background_file_dump_pending(meta: &FileWriteMetaData) -> bool {
    unsafe { ffi::CFE_FS_BackgroundFileDumpIsPending(&meta.0) }
}

/// Creates a fixed mapping between a physical host path and a virtual OSAL mount point.
///
/// This is typically called by the PSP/BSP before application startup to configure
/// the virtual filesystem.
pub fn add_fixed_map(phys_path: &str, virt_path: &str) -> Result<OsalId> {
    let c_phys = c_path_from_str(phys_path)?;
    let c_virt = c_path_from_str(virt_path)?;
    let mut filesys_id = MaybeUninit::uninit();
    check(unsafe {
        ffi::OS_FileSysAddFixedMap(filesys_id.as_mut_ptr(), c_phys.as_ptr(), c_virt.as_ptr())
    })?;
    Ok(OsalId(unsafe { filesys_id.assume_init() }))
}

/// Initializes an existing file system on the target.
///
/// This is a low-level function for preparing a block device or memory region for use,
/// but without creating it from scratch like `make_fs`.
pub fn init_fs(
    address: *mut u8,
    devname: &str,
    volname: &str,
    blocksize: usize,
    numblocks: usize,
) -> Result<()> {
    let c_dev = c_path_from_str(devname)?;
    let c_vol = c_path_from_str(volname)?;
    check(unsafe {
        ffi::OS_initfs(
            address as *mut libc::c_char,
            c_dev.as_ptr(),
            c_vol.as_ptr(),
            blocksize,
            numblocks,
        )
    })?;
    Ok(())
}

/// Removes a file system mapping from OSAL.
///
/// This does not unmount the filesystem, but rather removes the OSAL entry for it.
pub fn remove_fs(devname: &str) -> Result<()> {
    let c_dev = c_path_from_str(devname)?;
    check(unsafe { ffi::OS_rmfs(c_dev.as_ptr()) })?;
    Ok(())
}

/// Checks the health of a file system and optionally repairs it.
///
/// Note: This functionality may not be implemented on all underlying operating systems.
pub fn check_fs(path: &str, repair: bool) -> Result<()> {
    let c_path = c_path_from_str(path)?;
    check(unsafe { ffi::OS_chkfs(c_path.as_ptr(), repair) })?;
    Ok(())
}
