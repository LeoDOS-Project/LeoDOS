//! Core WAMR runtime, module, instance, and execution environment structs.

use super::{
    c_char_to_string_heapless, memory,
    reflect::{self, Export, Frame, Global, Import, Table},
    value::WasmValue,
    wasi::WasiCtxBuilder,
    LogLevel, Result, WamrError, ERROR_BUF_SIZE,
};
use crate::ffi::{self, NativeSymbol};
use crate::memory::ModulePtr;
#[cfg(feature = "thread-support")]
use crate::thread::WasmThread;
use core::ffi::{c_char, c_void, CStr};
use core::marker::PhantomData;
use core::ptr::{null_mut, NonNull};
use heapless::{CString, String, Vec};

/// The execution mode for the Wasm interpreter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RunningMode {
    Interpreter = ffi::RunningMode_Mode_Interp,
    FastJIT = ffi::RunningMode_Mode_Fast_JIT,
    LLVMJIT = ffi::RunningMode_Mode_LLVM_JIT,
    MultiTierJIT = ffi::RunningMode_Mode_Multi_Tier_JIT,
}

/// A builder for advanced runtime initialization.
#[derive(Default)]
pub struct RuntimeBuilder {
    init_args: ffi::RuntimeInitArgs,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a fixed-size memory pool instead of the system allocator.
    ///
    /// # Safety
    ///
    /// The `pool` buffer must have a `'static` lifetime.
    pub unsafe fn with_memory_pool(mut self, pool: &'static mut [u8]) -> Self {
        self.init_args.mem_alloc_type = ffi::mem_alloc_type_t_Alloc_With_Pool;
        self.init_args.mem_alloc_option.pool.heap_buf = pool.as_mut_ptr() as *mut _;
        self.init_args.mem_alloc_option.pool.heap_size = pool.len() as u32;
        self
    }

    /// Set native symbols to be linked with Wasm modules.
    ///
    /// # Safety
    ///
    /// The `native_symbols` slice and the data it points to (module name, symbol names, signatures)
    /// must have a `'static` lifetime, as WAMR does not copy them.
    pub unsafe fn with_native_symbols(
        mut self,
        module_name: &'static CStr,
        native_symbols: &'static mut [NativeSymbol],
    ) -> Self {
        self.init_args.native_module_name = module_name.as_ptr();
        self.init_args.n_native_symbols = native_symbols.len() as u32;
        self.init_args.native_symbols = native_symbols.as_mut_ptr();
        self
    }

    /// Sets the maximum number of threads. Only effective with `thread-support` feature.
    pub fn with_max_threads(mut self, count: u32) -> Self {
        self.init_args.max_thread_num = count;
        self
    }

    /// Sets the default running mode for the runtime.
    pub fn with_running_mode(mut self, mode: RunningMode) -> Self {
        self.init_args.running_mode = mode as u32;
        self
    }

    /// Sets the Fast JIT code cache size.
    pub fn with_fast_jit_cache_size(mut self, size: u32) -> Self {
        self.init_args.fast_jit_code_cache_size = size;
        self
    }

    /// Sets the LLVM JIT optimization level (0-3).
    pub fn with_llvm_jit_opt_level(mut self, level: u32) -> Self {
        self.init_args.llvm_jit_opt_level = level;
        self
    }

    /// Sets the LLVM JIT size level (0-3).
    pub fn with_llvm_jit_size_level(mut self, level: u32) -> Self {
        self.init_args.llvm_jit_size_level = level;
        self
    }

    /// Builds and initializes the WAMR runtime.
    ///
    /// # Safety
    ///
    /// This function must only be called once per process. The underlying
    /// WAMR runtime is a global singleton. Creating multiple `Runtime`
    /// instances will lead to undefined behavior.
    pub unsafe fn build(mut self) -> Result<Runtime> {
        if unsafe { ffi::wasm_runtime_full_init(&mut self.init_args) } {
            Ok(Runtime { _private: () })
        } else {
            Err(WamrError::InitializationFailed)
        }
    }
}

/// Represents the initialized WAMR runtime.
///
/// This struct manages the global lifecycle of the runtime. It should only be
/// created once. Dropping this struct will destroy the runtime.
pub struct Runtime {
    _private: (),
}

impl Runtime {
    /// Creates a new `RuntimeBuilder` for advanced configuration.
    pub fn builder() -> RuntimeBuilder {
        RuntimeBuilder::new()
    }

    /// Initializes the WAMR runtime with default settings.
    ///
    /// # Safety
    ///
    /// This function must only be called once per process. See `RuntimeBuilder::build`.
    pub unsafe fn new() -> Result<Self> {
        unsafe { RuntimeBuilder::new().build() }
    }

    /// Loads a WebAssembly module from a byte buffer.
    ///
    /// The runtime may modify the buffer for optimization purposes, so it must be mutable.
    pub fn load<'r>(&'r self, wasm_binary: &mut [u8]) -> Result<Module<'r>> {
        let mut error_buf = [0u8; ERROR_BUF_SIZE];
        let module_ptr = unsafe {
            ffi::wasm_runtime_load(
                wasm_binary.as_mut_ptr(),
                wasm_binary.len() as u32,
                error_buf.as_mut_ptr() as *mut c_char,
                error_buf.len() as u32,
            )
        };

        if module_ptr.is_null() {
            let error_msg = c_char_to_string_heapless(&error_buf as *const _ as *const c_char)?;
            Err(WamrError::LoadFailed(error_msg))
        } else {
            Ok(Module {
                ptr: NonNull::new(module_ptr).unwrap(),
                _phantom: PhantomData,
            })
        }
    }

    /// Registers a module with a given name, making it available for other modules to import.
    ///
    /// The module must not have been registered previously.
    pub fn register_module<const N: usize>(&self, name: &str, module: &Module) -> Result<()> {
        let mut c_name = CString::<N>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;
        let mut error_buf = [0u8; ERROR_BUF_SIZE];

        let success = unsafe {
            ffi::wasm_runtime_register_module(
                c_name.as_ptr(),
                module.as_ptr(),
                error_buf.as_mut_ptr() as *mut _,
                error_buf.len() as u32,
            )
        };

        if !success {
            let error_msg = c_char_to_string_heapless(&error_buf as *const _ as *const c_char)?;
            Err(WamrError::RegistrationFailed(error_msg))
        } else {
            Ok(())
        }
    }

    /// Finds a previously registered module by name.
    pub fn find_registered_module<'r, const N: usize>(&'r self, name: &str) -> Result<Module<'r>> {
        let mut c_name = CString::<N>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;

        let module_ptr = unsafe { ffi::wasm_runtime_find_module_registered(c_name.as_ptr()) };

        if module_ptr.is_null() {
            let mut not_found_msg = String::new();
            not_found_msg
                .push_str(name)
                .map_err(|_| WamrError::CapacityExceeded)?;
            Err(WamrError::NotFound(not_found_msg))
        } else {
            Ok(Module {
                ptr: NonNull::new(module_ptr).unwrap(),
                _phantom: PhantomData,
            })
        }
    }

    /// Sets the runtime log level.
    pub fn set_log_level(&self, level: LogLevel) {
        unsafe { ffi::wasm_runtime_set_log_level(level as u32) }
    }

    /// Returns the version of the WAMR runtime.
    pub fn version(&self) -> (u32, u32, u32) {
        let mut major = 0;
        let mut minor = 0;
        let mut patch = 0;
        unsafe { ffi::wasm_runtime_get_version(&mut major, &mut minor, &mut patch) };
        (major, minor, patch)
    }

    /// Creates a shared heap that can be attached to module instances.
    pub fn create_shared_heap(&self, size: u32) -> Result<SharedHeap> {
        let mut args = ffi::SharedHeapInitArgs {
            size,
            pre_allocated_addr: null_mut(),
        };
        let heap_ptr = unsafe { ffi::wasm_runtime_create_shared_heap(&mut args) };
        NonNull::new(heap_ptr)
            .map(|ptr| SharedHeap { ptr })
            .ok_or_else(|| {
                WamrError::MemoryError(String::try_from("Failed to create shared heap").unwrap())
            })
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        unsafe {
            ffi::wasm_runtime_destroy();
        }
    }
}

/// Arguments for module instantiation.
#[derive(Debug, Default, Clone, Copy)]
pub struct InstantiationArgs {
    pub stack_size: u32,
    pub heap_size: u32,
    pub max_memory_pages: Option<u32>,
}

/// A compiled WebAssembly module.
///
/// It holds a reference to the `Runtime` it was created from.
pub struct Module<'r> {
    pub(crate) ptr: NonNull<ffi::WASMModuleCommon>,
    _phantom: PhantomData<&'r Runtime>,
}

impl<'r> Module<'r> {
    /// Instantiates the module with default arguments.
    pub fn instantiate(&self, stack_size: u32, heap_size: u32) -> Result<Instance<'_>> {
        let args = InstantiationArgs {
            stack_size,
            heap_size,
            max_memory_pages: None,
        };
        self.instantiate_with(&args)
    }

    /// Instantiates the module with extended arguments.
    pub fn instantiate_with(&self, args: &InstantiationArgs) -> Result<Instance<'_>> {
        let ffi_args = ffi::InstantiationArgs {
            default_stack_size: args.stack_size,
            host_managed_heap_size: args.heap_size,
            max_memory_pages: args.max_memory_pages.unwrap_or(0),
        };

        let mut error_buf = [0u8; ERROR_BUF_SIZE];
        let instance_ptr = unsafe {
            ffi::wasm_runtime_instantiate_ex(
                self.ptr.as_ptr(),
                &ffi_args,
                error_buf.as_mut_ptr() as *mut c_char,
                error_buf.len() as u32,
            )
        };

        if instance_ptr.is_null() {
            let error_msg = c_char_to_string_heapless(&error_buf as *const _ as *const c_char)?;
            Err(WamrError::InstantiationFailed(error_msg))
        } else {
            Ok(Instance {
                ptr: NonNull::new(instance_ptr).unwrap(),
                _phantom: PhantomData,
            })
        }
    }

    /// Configures the WASI parameters for this module.
    ///
    /// This must be called *before* `instantiate`.
    pub fn configure_wasi<
        const ARGS_CAP: usize,
        const ENVS_CAP: usize,
        const DIRS_CAP: usize,
        const STR_CAP: usize,
    >(
        &self,
        builder: &WasiCtxBuilder<ARGS_CAP, ENVS_CAP, DIRS_CAP, STR_CAP>,
    ) {
        builder.apply_to_module(self);
    }

    /// Returns an iterator over the imports of this module.
    pub fn imports<const N: usize>(&self) -> impl Iterator<Item = Import<N>> {
        reflect::ImportIterator::new(self.ptr)
    }

    /// Returns an iterator over the exports of this module.
    pub fn exports<const N: usize>(&self) -> impl Iterator<Item = Export<N>> {
        reflect::ExportIterator::new(self.ptr)
    }

    /// Gets a custom section from the module by name.
    pub fn get_custom_section<const N: usize>(&self, name: &str) -> Option<&[u8]> {
        let mut c_name = CString::<N>::new();
        c_name.extend_from_bytes(name.as_bytes()).ok()?;
        let mut len = 0;
        let ptr = unsafe {
            ffi::wasm_runtime_get_custom_section(self.ptr.as_ptr(), c_name.as_ptr(), &mut len)
        };

        if ptr.is_null() {
            None
        } else {
            Some(unsafe { core::slice::from_raw_parts(ptr, len as usize) })
        }
    }

    /// Returns the raw underlying pointer.
    pub fn as_ptr(&self) -> *mut ffi::WASMModuleCommon {
        self.ptr.as_ptr()
    }
}

impl Drop for Module<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::wasm_runtime_unload(self.ptr.as_ptr());
        }
    }
}

/// An instantiated WebAssembly module.
pub struct Instance<'m> {
    pub(crate) ptr: NonNull<ffi::WASMModuleInstanceCommon>,
    _phantom: PhantomData<&'m Module<'m>>,
}

impl<'m> Instance<'m> {
    /// Creates a temporary, non-owning `Instance` wrapper from a raw pointer.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is a valid, non-null pointer to a
    /// `WASMModuleInstanceCommon` and that the lifetime of the pointer exceeds
    /// the lifetime of the returned `Instance` object. The returned object
    /// must not be dropped; use `core::mem::forget` to prevent this.
    pub unsafe fn from_raw(ptr: *mut ffi::WASMModuleInstanceCommon) -> Self {
        Instance {
            ptr: unsafe { NonNull::new_unchecked(ptr) },
            _phantom: PhantomData,
        }
    }

    /// Reads a null-terminated string from the instance's memory.
    pub fn read_string(&self, offset: u32) -> Result<String<256>> {
        let memory = self.default_memory()?;
        let native_ptr = memory.offset_to_native(offset as u64)?;
        c_char_to_string_heapless(native_ptr as *const c_char)
    }

    /// Looks up an exported function by name.
    pub fn lookup_function<'i, const N: usize>(&'i self, name: &str) -> Result<Function<'i>> {
        let mut c_name = CString::<N>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;

        let func_ptr =
            unsafe { ffi::wasm_runtime_lookup_function(self.ptr.as_ptr(), c_name.as_ptr()) };

        if func_ptr.is_null() {
            let mut not_found_msg = String::new();
            not_found_msg
                .push_str(name)
                .map_err(|_| WamrError::CapacityExceeded)?;
            Err(WamrError::NotFound(not_found_msg))
        } else {
            Ok(Function {
                ptr: NonNull::new(func_ptr).unwrap(),
                instance_ptr: self.ptr,
                _phantom: PhantomData,
            })
        }
    }

    /// Looks up an exported global by name.
    pub fn lookup_global<const N: usize>(&self, name: &str) -> Result<Global> {
        let mut c_name = CString::<N>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;
        let mut global_inst = ffi::wasm_global_inst_t::default();

        let success = unsafe {
            ffi::wasm_runtime_get_export_global_inst(
                self.ptr.as_ptr(),
                c_name.as_ptr(),
                &mut global_inst,
            )
        };

        if !success {
            let mut not_found_msg = String::new();
            not_found_msg
                .push_str(name)
                .map_err(|_| WamrError::CapacityExceeded)?;
            Err(WamrError::NotFound(not_found_msg))
        } else {
            Ok(Global::new(global_inst))
        }
    }

    /// Looks up an exported table by name.
    pub fn lookup_table<'i, const N: usize>(&'i self, name: &str) -> Result<Table<'i>> {
        let mut c_name = CString::<N>::new();
        c_name
            .extend_from_bytes(name.as_bytes())
            .map_err(|_| WamrError::InvalidCString)?;
        let mut table_inst = ffi::wasm_table_inst_t::default();

        let success = unsafe {
            ffi::wasm_runtime_get_export_table_inst(
                self.ptr.as_ptr(),
                c_name.as_ptr(),
                &mut table_inst,
            )
        };

        if !success {
            let mut not_found_msg = String::new();
            not_found_msg
                .push_str(name)
                .map_err(|_| WamrError::CapacityExceeded)?;
            Err(WamrError::NotFound(not_found_msg))
        } else {
            Ok(Table::new(table_inst, self))
        }
    }

    /// Creates an execution environment for this instance.
    pub fn create_exec_env(&self, stack_size: u32) -> Result<ExecEnv<'_>> {
        let exec_env_ptr =
            unsafe { ffi::wasm_runtime_create_exec_env(self.ptr.as_ptr(), stack_size) };

        if exec_env_ptr.is_null() {
            let err_msg = String::try_from("Failed to create exec env").unwrap();
            Err(WamrError::InstantiationFailed(err_msg))
        } else {
            Ok(ExecEnv {
                ptr: NonNull::new(exec_env_ptr).unwrap(),
                _phantom: PhantomData,
            })
        }
    }

    /// Gets the default linear memory of the instance.
    pub fn default_memory(&self) -> Result<memory::Memory<'_>> {
        let mem_ptr = unsafe { ffi::wasm_runtime_get_default_memory(self.ptr.as_ptr()) };
        if mem_ptr.is_null() {
            Err(WamrError::NotFound(
                String::try_from("default memory").unwrap(),
            ))
        } else {
            Ok(memory::Memory::new(NonNull::new(mem_ptr).unwrap(), self))
        }
    }

    /// Get the exception string, if any.
    pub fn get_exception(&self) -> Option<String<ERROR_BUF_SIZE>> {
        let exception_ptr = unsafe { ffi::wasm_runtime_get_exception(self.ptr.as_ptr()) };
        if exception_ptr.is_null() {
            None
        } else {
            c_char_to_string_heapless(exception_ptr).ok()
        }
    }

    /// Clears any pending exception on the instance.
    pub fn clear_exception(&self) {
        unsafe { ffi::wasm_runtime_clear_exception(self.ptr.as_ptr()) }
    }

    /// Returns the raw underlying pointer.
    pub fn as_ptr(&self) -> *mut ffi::WASMModuleInstanceCommon {
        self.ptr.as_ptr()
    }

    /// Associates a raw pointer with this instance, which can be retrieved later.
    /// This is useful for storing host-specific state.
    ///
    /// # Safety
    ///
    /// The caller is responsible for the lifetime of the data pointed to by `custom_data`.
    pub unsafe fn set_custom_data(&self, custom_data: *mut c_void) {
        unsafe { ffi::wasm_runtime_set_custom_data(self.ptr.as_ptr(), custom_data) }
    }

    /// Retrieves the custom data pointer associated with this instance.
    pub fn get_custom_data(&self) -> *mut c_void {
        unsafe { ffi::wasm_runtime_get_custom_data(self.ptr.as_ptr()) }
    }

    /// Maps a host object pointer to a Wasm `externref` index.
    /// This allows Wasm code to hold opaque references to host data.
    ///
    /// # Safety
    ///
    /// The caller is responsible for the lifetime of `extern_obj`.
    pub unsafe fn obj_to_externref(&self, extern_obj: *mut c_void) -> Result<u32> {
        let mut p_externref_idx = 0;
        if unsafe {
            ffi::wasm_externref_obj2ref(self.ptr.as_ptr(), extern_obj, &mut p_externref_idx)
        } {
            Ok(p_externref_idx)
        } else {
            Err(WamrError::ExecutionError(
                String::try_from("Failed to create externref").unwrap(),
            ))
        }
    }

    /// Retrieves a host object pointer from a Wasm `externref` index.
    ///
    /// # Safety
    ///
    /// The returned pointer is only valid if the `externref` is still valid.
    pub unsafe fn externref_to_obj(&self, externref_idx: u32) -> Result<*mut c_void> {
        let mut p_extern_obj = null_mut();
        if unsafe { ffi::wasm_externref_ref2obj(externref_idx, &mut p_extern_obj) } {
            Ok(p_extern_obj)
        } else {
            Err(WamrError::ExecutionError(
                String::try_from("Failed to resolve externref").unwrap(),
            ))
        }
    }

    /// Allocates memory within the Wasm instance's own heap (the one used by malloc/free in C).
    /// Returns an RAII guard that frees the memory on drop.
    pub fn module_malloc(&self, size: u64) -> Result<ModulePtr<'_, [u8]>> {
        let mut native_addr = null_mut();
        let offset =
            unsafe { ffi::wasm_runtime_module_malloc(self.ptr.as_ptr(), size, &mut native_addr) };

        if offset == 0 {
            Err(WamrError::MemoryError(
                String::try_from("Module malloc failed").unwrap(),
            ))
        } else {
            Ok(ModulePtr::new(self, offset, size))
        }
    }

    /// Frees memory previously allocated with `module_malloc`.
    /// Note: This is called automatically when `ModulePtr` is dropped.
    pub(crate) fn module_free(&self, offset: u64) {
        unsafe { ffi::wasm_runtime_module_free(self.ptr.as_ptr(), offset) }
    }

    /// Executes the main entry point of a WASI application (`_start` or `_initialize`).
    pub fn execute_main<const ARGS_CAP: usize, const ARG_LEN: usize>(
        &self,
        args: &[&str],
    ) -> Result<()> {
        let c_args: Result<Vec<CString<ARG_LEN>, ARGS_CAP>> = args
            .iter()
            .map(|&arg| {
                let mut cs = CString::new();
                cs.extend_from_bytes(arg.as_bytes())
                    .map_err(|_| WamrError::InvalidCString)?;
                Ok(cs)
            })
            .collect();

        let c_args = c_args?;
        let mut argv: Vec<*mut c_char, ARGS_CAP> =
            c_args.iter().map(|cs| cs.as_ptr() as *mut _).collect();

        if unsafe {
            ffi::wasm_application_execute_main(
                self.ptr.as_ptr(),
                argv.len() as i32,
                argv.as_mut_ptr(),
            )
        } {
            Ok(())
        } else {
            Err(WamrError::WasiError)
        }
    }

    /// Gets the exit code of a WASI application after it has run.
    pub fn wasi_exit_code(&self) -> u32 {
        unsafe { ffi::wasm_runtime_get_wasi_exit_code(self.ptr.as_ptr()) }
    }

    /// Attaches a shared heap to this instance.
    pub fn attach_shared_heap(&self, heap: &SharedHeap) -> Result<()> {
        if unsafe { ffi::wasm_runtime_attach_shared_heap(self.ptr.as_ptr(), heap.ptr.as_ptr()) } {
            Ok(())
        } else {
            Err(WamrError::MemoryError(
                String::try_from("Failed to attach shared heap").unwrap(),
            ))
        }
    }

    /// Asynchronously terminate the execution of this instance.
    pub fn terminate(&self) {
        unsafe { ffi::wasm_runtime_terminate(self.ptr.as_ptr()) }
    }

    /// Dump memory consumption statistics for this instance to the console.
    pub fn dump_memory_consumption(&self, exec_env: &ExecEnv) {
        unsafe { ffi::wasm_runtime_dump_mem_consumption(exec_env.as_ptr()) }
    }

    /// Dump performance profiling statistics to the console.
    pub fn dump_performance_profiling(&self) {
        unsafe { ffi::wasm_runtime_dump_perf_profiling(self.ptr.as_ptr()) }
    }
}

impl Drop for Instance<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::wasm_runtime_deinstantiate(self.ptr.as_ptr());
        }
    }
}

/// A handle to an exported WebAssembly function.
pub struct Function<'i> {
    pub(crate) ptr: NonNull<ffi::WASMFunctionInstanceCommon>,
    pub(crate) instance_ptr: NonNull<ffi::WASMModuleInstanceCommon>,
    pub(crate) _phantom: PhantomData<&'i Instance<'i>>,
}

impl<'i> Function<'i> {
    pub fn param_count(&self) -> u32 {
        unsafe { ffi::wasm_func_get_param_count(self.ptr.as_ptr(), self.instance_ptr.as_ptr()) }
    }
    pub fn result_count(&self) -> u32 {
        unsafe { ffi::wasm_func_get_result_count(self.ptr.as_ptr(), self.instance_ptr.as_ptr()) }
    }
}

/// An execution environment for calling Wasm functions.
pub struct ExecEnv<'i> {
    pub(crate) ptr: NonNull<ffi::WASMExecEnv>,
    _phantom: PhantomData<&'i Instance<'i>>,
}

impl<'i> ExecEnv<'i> {
    /// Calls a WebAssembly function with the given arguments.
    pub fn call<const ARGS_CAP: usize, const RESULTS_CAP: usize>(
        &mut self,
        func: &Function,
        args: &Vec<WasmValue, ARGS_CAP>,
    ) -> Result<Vec<WasmValue, RESULTS_CAP>> {
        let mut c_args: Vec<ffi::wasm_val_t, ARGS_CAP> = Vec::new();
        for arg in args {
            c_args
                .push((*arg).into())
                .map_err(|_| WamrError::CapacityExceeded)?;
        }

        let result_count = func.result_count();
        if result_count as usize > RESULTS_CAP {
            return Err(WamrError::CapacityExceeded);
        }

        let mut c_results: Vec<ffi::wasm_val_t, RESULTS_CAP> = Vec::new();
        c_results
            .resize_default(result_count as usize)
            .map_err(|_| WamrError::CapacityExceeded)?;

        let success = unsafe {
            ffi::wasm_runtime_call_wasm_a(
                self.ptr.as_ptr(),
                func.ptr.as_ptr(),
                result_count,
                c_results.as_mut_ptr(),
                args.len() as u32,
                c_args.as_mut_ptr(),
            )
        };

        if !success {
            let instance_ptr = unsafe { ffi::wasm_runtime_get_module_inst(self.ptr.as_ptr()) };
            let error_msg = c_char_to_string_heapless(unsafe {
                ffi::wasm_runtime_get_exception(instance_ptr)
            })?;
            return Err(WamrError::ExecutionError(error_msg));
        }

        let mut results: Vec<WasmValue, RESULTS_CAP> = Vec::new();
        for val in c_results.iter().take(result_count as usize) {
            results
                .push(WasmValue::try_from(*val)?)
                .map_err(|_| WamrError::CapacityExceeded)?;
        }

        Ok(results)
    }

    /// Calls a function in a table by its index.
    /// This uses the legacy `u32` array ABI for arguments and results.
    pub fn call_indirect<const ARGS_CAP: usize>(
        &mut self,
        element_index: u32,
        args: &[WasmValue],
    ) -> Result<Vec<WasmValue, ARGS_CAP>> {
        let mut argv: Vec<u32, ARGS_CAP> = Vec::new();
        // This is complex because we need to know the return types to reserve space.
        // WAMR's C API for this is not ideal. This wrapper assumes no return values for simplicity.
        // A more complete implementation would need to look up the function type.

        for arg in args {
            match arg {
                WasmValue::I32(v) => argv
                    .push(*v as u32)
                    .map_err(|_| WamrError::CapacityExceeded)?,
                WasmValue::F32(v) => argv
                    .push(v.to_bits())
                    .map_err(|_| WamrError::CapacityExceeded)?,
                WasmValue::I64(v) => {
                    let bytes = v.to_le_bytes();
                    let (p1, p2) = bytes.split_at(4);
                    argv.push(u32::from_le_bytes(p1.try_into().unwrap()))
                        .map_err(|_| WamrError::CapacityExceeded)?;
                    argv.push(u32::from_le_bytes(p2.try_into().unwrap()))
                        .map_err(|_| WamrError::CapacityExceeded)?;
                }
                WasmValue::F64(v) => {
                    let bytes = v.to_le_bytes();
                    let (p1, p2) = bytes.split_at(4);
                    argv.push(u32::from_le_bytes(p1.try_into().unwrap()))
                        .map_err(|_| WamrError::CapacityExceeded)?;
                    argv.push(u32::from_le_bytes(p2.try_into().unwrap()))
                        .map_err(|_| WamrError::CapacityExceeded)?;
                }
            }
        }

        let success = unsafe {
            ffi::wasm_runtime_call_indirect(
                self.ptr.as_ptr(),
                element_index,
                argv.len() as u32,
                argv.as_mut_ptr(),
            )
        };

        if !success {
            let instance_ptr = unsafe { ffi::wasm_runtime_get_module_inst(self.ptr.as_ptr()) };
            let error_msg = c_char_to_string_heapless(unsafe {
                ffi::wasm_runtime_get_exception(instance_ptr)
            })?;
            return Err(WamrError::ExecutionError(error_msg));
        }

        // As noted, this simplified version doesn't handle return values from call_indirect.
        Ok(Vec::new())
    }

    pub fn as_ptr(&self) -> *mut ffi::WASMExecEnv {
        self.ptr.as_ptr()
    }

    /// Spawns a new WAMR thread.
    ///
    /// This is only available with the `thread-support` feature.
    ///
    /// # Safety
    ///
    /// The `arg` pointer must be valid for the duration of the thread's execution.
    /// The callback function is an `extern "C"` function pointer.
    #[cfg(feature = "thread-support")]
    pub unsafe fn spawn_thread<T>(
        &self,
        callback: unsafe extern "C" fn(
            exec_env: *mut ffi::WASMExecEnv,
            arg: *mut c_void,
        ) -> *mut c_void,
        arg: *mut T,
    ) -> Result<WasmThread> {
        let mut tid: ffi::wasm_thread_t = 0;
        let result = unsafe {
            ffi::wasm_runtime_spawn_thread(
                self.ptr.as_ptr(),
                &mut tid,
                Some(callback),
                arg as *mut c_void,
            )
        };

        if result == 0 {
            Ok(WasmThread { tid })
        } else {
            Err(WamrError::ThreadError(result))
        }
    }

    /// Sets user data on the execution environment, retrievable by native functions.
    ///
    /// # Safety
    /// The caller is responsible for the lifetime of `user_data`.
    pub unsafe fn set_user_data(&mut self, user_data: *mut c_void) {
        unsafe { ffi::wasm_runtime_set_user_data(self.ptr.as_ptr(), user_data) }
    }

    /// Gets the user data from the execution environment.
    pub fn get_user_data(&self) -> *mut c_void {
        unsafe { ffi::wasm_runtime_get_user_data(self.ptr.as_ptr()) }
    }

    /// Captures the current call stack.
    ///
    /// The maximum number of frames is determined by `const N`.
    pub fn call_stack<const N: usize>(&self) -> Result<Vec<Frame, N>> {
        let mut frames_buf: Vec<ffi::WASMCApiFrame, N> = Vec::new();
        frames_buf
            .resize_default(N)
            .map_err(|_| WamrError::CapacityExceeded)?;

        let mut error_buf = [0u8; ERROR_BUF_SIZE];

        let count = unsafe {
            ffi::wasm_copy_callstack(
                self.ptr.as_ptr(),
                frames_buf.as_mut_ptr(),
                N as u32,
                0,
                error_buf.as_mut_ptr() as *mut _,
                error_buf.len() as u32,
            )
        };

        frames_buf.truncate(count as usize);
        Ok(frames_buf.into_iter().map(Frame::new).collect())
    }

    /// Sets an instruction count limit for the next Wasm call on this environment.
    ///
    /// This acts as "fuel" for the execution. If the Wasm code runs for more than
    /// `limit` instructions, the call will trap and return an `ExecutionError`.
    /// This prevents the Wasm code from blocking the host thread indefinitely.
    ///
    /// A limit of 0 or a large number effectively disables the limit.
    ///
    /// **NOTE:** This feature requires WAMR to be compiled with `WAMR_BUILD_OPCODE_COUNTER=1`.
    pub fn set_instruction_limit(&mut self, limit: u32) {
        unsafe {
            ffi::wasm_runtime_set_instruction_count_limit(self.ptr.as_ptr(), limit as i32);
        }
    }
}

impl Drop for ExecEnv<'_> {
    fn drop(&mut self) {
        unsafe {
            ffi::wasm_runtime_destroy_exec_env(self.ptr.as_ptr());
        }
    }
}

/// A handle to a shared heap, which can be attached to multiple instances.
pub struct SharedHeap {
    pub(crate) ptr: NonNull<ffi::WASMSharedHeap>,
}
