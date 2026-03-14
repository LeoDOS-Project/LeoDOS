//! Safe, idiomatic wrappers for OSAL Shell command execution.

use crate::error::{CfsError, OsalError, Result};
use crate::ffi;
use crate::os::fs::File;
use crate::cstring;
use crate::status::check;

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
    let c_command = cstring::<{ ffi::OS_MAX_CMD_LEN as usize }>(command)
        .map_err(|_| CfsError::Osal(OsalError::InvalidArgument))?;

    check(unsafe { ffi::OS_ShellOutputToFile(c_command.as_ptr(), output_file.id().0) })?;
    Ok(())
}
