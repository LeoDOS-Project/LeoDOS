//! A builder for configuring WASI parameters.

use crate::{Module, Result, WamrError, ffi};
use heapless::{CString, String, Vec};

/// A builder for setting up the WASI environment for a module instance.
///
/// The const generics define the capacity of the builder's internal storage:
/// - `ARGS_CAP`: Max number of command-line arguments.
/// - `ENVS_CAP`: Max number of environment variables.
/// - `DIRS_CAP`: Max number of pre-opened directories.
/// - `STR_CAP`: Max length of any individual argument, variable, or path string.
///
/// This should be created and configured, then applied to a `Module`
/// using `Module::configure_wasi` *before* the module is instantiated.
pub struct WasiCtxBuilder<
    const ARGS_CAP: usize,
    const ENVS_CAP: usize,
    const DIRS_CAP: usize,
    const STR_CAP: usize,
> {
    args: Vec<CString<STR_CAP>, ARGS_CAP>,
    envs: Vec<CString<STR_CAP>, ENVS_CAP>,
    dirs: Vec<CString<STR_CAP>, DIRS_CAP>,
    map_dirs: Vec<CString<STR_CAP>, DIRS_CAP>,
}

impl<const ARGS_CAP: usize, const ENVS_CAP: usize, const DIRS_CAP: usize, const STR_CAP: usize>
    Default for WasiCtxBuilder<ARGS_CAP, ENVS_CAP, DIRS_CAP, STR_CAP>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const ARGS_CAP: usize, const ENVS_CAP: usize, const DIRS_CAP: usize, const STR_CAP: usize>
    WasiCtxBuilder<ARGS_CAP, ENVS_CAP, DIRS_CAP, STR_CAP>
{
    /// Creates a new, empty WASI context builder.
    pub fn new() -> Self {
        Self {
            args: Vec::new(),
            envs: Vec::new(),
            dirs: Vec::new(),
            map_dirs: Vec::new(),
        }
    }

    /// Adds a command-line argument.
    pub fn arg(mut self, arg: &str) -> Result<Self> {
        let mut cstr = CString::<STR_CAP>::new();
        cstr.extend_from_bytes(arg.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;
        self.args
            .push(cstr)
            .map_err(|_| WamrError::CapacityExceeded)?;
        Ok(self)
    }

    /// Adds an environment variable in `"KEY=VALUE"` format.
    pub fn env(mut self, env: &str) -> Result<Self> {
        let mut cstr = CString::<STR_CAP>::new();
        cstr.extend_from_bytes(env.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;
        self.envs
            .push(cstr)
            .map_err(|_| WamrError::CapacityExceeded)?;
        Ok(self)
    }

    /// Pre-opens a directory on the host for the guest to access.
    /// `host_path` is the real path on the host system.
    /// `guest_path` is the path inside the Wasm module.
    pub fn preopened_dir(mut self, host_path: &str, guest_path: &str) -> Result<Self> {
        let mut map_dir_str: String<STR_CAP> = String::new();
        map_dir_str
            .push_str(guest_path)
            .map_err(|_| WamrError::CapacityExceeded)?;
        map_dir_str
            .push_str("::")
            .map_err(|_| WamrError::CapacityExceeded)?;
        map_dir_str
            .push_str(host_path)
            .map_err(|_| WamrError::CapacityExceeded)?;

        let mut c_host_path = CString::<STR_CAP>::new();
        c_host_path
            .extend_from_bytes(host_path.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;

        let mut c_map_dir = CString::<STR_CAP>::new();
        c_map_dir
            .extend_from_bytes(map_dir_str.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;

        self.dirs
            .push(c_host_path)
            .map_err(|_| WamrError::CapacityExceeded)?;
        self.map_dirs
            .push(c_map_dir)
            .map_err(|_| WamrError::CapacityExceeded)?;
        Ok(self)
    }

    /// Applies the configured WASI context to a `Module`.
    pub(crate) fn apply_to_module(&self, module: &Module) {
        let args_ptr: Vec<*mut i8, ARGS_CAP> =
            self.args.iter().map(|s| s.as_ptr() as *mut _).collect();
        let envs_ptr: Vec<*const i8, ENVS_CAP> =
            self.envs.iter().map(|s| s.as_ptr() as *const _).collect();
        let dirs_ptr: Vec<*const i8, DIRS_CAP> =
            self.dirs.iter().map(|s| s.as_ptr() as *const _).collect();
        let map_dirs_ptr: Vec<*const i8, DIRS_CAP> = self
            .map_dirs
            .iter()
            .map(|s| s.as_ptr() as *const _)
            .collect();

        unsafe {
            ffi::wasm_runtime_set_wasi_args(
                module.as_ptr(),
                dirs_ptr.as_ptr() as *mut _,
                dirs_ptr.len() as u32,
                map_dirs_ptr.as_ptr() as *mut _,
                map_dirs_ptr.len() as u32,
                envs_ptr.as_ptr() as *mut _,
                envs_ptr.len() as u32,
                args_ptr.as_ptr() as *mut _,
                args_ptr.len() as i32,
            );
        }
    }
}
