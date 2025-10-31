//! Safe, idiomatic wrappers for OSAL Shell command execution.

use crate::error::{Error, Result};
use crate::ffi;
use crate::os::fs::File;
use crate::status::check;
use heapless::CString;

/// Executes a shell command and redirects its standard output to a file.
///
/// This function provides a safe wrapper around `OS_ShellOutputToFile`.
///
/// # Arguments
/// * `command`: The shell command string to execute.
/// * `output_file`: An open, writable `libcfs::fs::File` handle where the
///   command's output will be written. The file must remain open for the
///   duration of this call.
pub fn command_to_file(command: &str, output_file: &File) -> Result<()> {
    let mut c_command = CString::<{ ffi::OS_MAX_CMD_LEN as usize }>::new();
    c_command
        .extend_from_bytes(command.as_bytes())
        .map_err(|_| Error::OsErrInvalidArgument)?;

    check(unsafe { ffi::OS_ShellOutputToFile(c_command.as_ptr(), output_file.id().0) })?;
    Ok(())
}
