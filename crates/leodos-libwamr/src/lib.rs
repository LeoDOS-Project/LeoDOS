//! A safe, `no_std` Rust wrapper for the WAMR C API, using `heapless` for collections.

#![no_std]
#![cfg_attr(feature = "thread-support", feature(c_variadic))]

// FFI bindings remain private to the crate root
pub(crate) mod ffi {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    
    // Use the generated NativeSymbol, not a redefinition
    pub use self::generated::NativeSymbol;

    // Put the generated bindings inside a submodule to avoid polluting the `ffi` namespace
    mod generated {
        include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
    }
    // Selectively re-export what's needed, or just use generated::*
    pub use self::generated::*;
}

// --- New Module Structure ---
pub mod memory;
pub mod reflect;
pub mod runtime;
pub mod value;
pub mod wasi;

#[cfg(feature = "thread-support")]
pub mod thread;

// --- Public API Re-exports ---

// Core runtime components
pub use runtime::{
    ExecEnv, Function, Instance, InstantiationArgs, Module, Runtime, RuntimeBuilder, RunningMode, SharedHeap,
};

// Memory management
pub use memory::{Memory, ModulePtr};

// Reflection types
pub use reflect::{
    Export, Frame, FuncType, Global, GlobalType, Import, ImportExportKind, ImportExportType,
    MemoryType, Table, TableType,
};

// Wasm value types
pub use value::{WasmValue, WasmValueKind};

// Threading (conditional)
#[cfg(feature = "thread-support")]
pub use thread::WasmThread;

// Errors and top-level items
use core::ffi::{c_char, CStr};
use core::fmt;
use core::str::Utf8Error;
use heapless::String;

/// The maximum size of an error message string returned by the runtime.
pub const ERROR_BUF_SIZE: usize = 128;

/// Log levels for the WAMR runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum LogLevel {
    Fatal = ffi::log_level_t_WASM_LOG_LEVEL_FATAL,
    Error = ffi::log_level_t_WASM_LOG_LEVEL_ERROR,
    Warning = ffi::log_level_t_WASM_LOG_LEVEL_WARNING,
    Debug = ffi::log_level_t_WASM_LOG_LEVEL_DEBUG,
    Verbose = ffi::log_level_t_WASM_LOG_LEVEL_VERBOSE,
}

/// Errors that can occur when interacting with the WAMR runtime.
#[derive(Debug)]
pub enum WamrError {
    /// The WAMR runtime failed to initialize.
    InitializationFailed,
    /// A Rust string could not be converted to a C string (too long or contains null bytes).
    InvalidCString,
    /// A C-style string from the runtime was not valid UTF-8.
    InvalidUtf8(Utf8Error),
    /// Failed to load the module. The string contains the runtime's error message.
    LoadFailed(String<ERROR_BUF_SIZE>),
    /// Failed to instantiate the module. The string contains the runtime's error message.
    InstantiationFailed(String<ERROR_BUF_SIZE>),
    /// The requested item (function, global, etc.) could not be found.
    NotFound(String<64>),
    /// A Wasm function execution resulted in a trap or error.
    ExecutionError(String<ERROR_BUF_SIZE>),
    /// An error occurred during a memory operation.
    MemoryError(String<ERROR_BUF_SIZE>),
    /// The returned Wasm value had an unexpected type.
    InvalidWasmValue,
    /// A `heapless` collection has exceeded its capacity.
    CapacityExceeded,
    /// A null pointer was encountered where it was not expected.
    NullPointer,
    /// An error occurred during module registration.
    RegistrationFailed(String<ERROR_BUF_SIZE>),
    /// An error occurred during threading operations.
    ThreadError(i32),
    /// An error occurred in a WASI entry point function.
    WasiError,
}

impl fmt::Display for WamrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WamrError::InitializationFailed => write!(f, "WAMR runtime initialization failed"),
            WamrError::InvalidCString => {
                write!(
                    f,
                    "Invalid C-style string (too long or contains null bytes)"
                )
            }
            WamrError::InvalidUtf8(e) => write!(f, "Invalid UTF-8 from runtime: {}", e),
            WamrError::LoadFailed(e) => write!(f, "Failed to load module: {}", e),
            WamrError::InstantiationFailed(e) => write!(f, "Failed to instantiate module: {}", e),
            WamrError::NotFound(name) => write!(f, "Item '{}' not found", name),
            WamrError::ExecutionError(e) => write!(f, "WASM execution error: {}", e),
            WamrError::MemoryError(e) => write!(f, "WASM memory error: {}", e),
            WamrError::InvalidWasmValue => write!(f, "Invalid Wasm value type"),
            WamrError::CapacityExceeded => {
                write!(f, "Capacity of a heapless collection was exceeded")
            }
            WamrError::NullPointer => write!(f, "Encountered a null pointer"),
            WamrError::RegistrationFailed(e) => write!(f, "Module registration failed: {}", e),
            WamrError::ThreadError(code) => write!(f, "Thread operation failed with code {}", code),
            WamrError::WasiError => write!(f, "WASI main function execution failed"),
        }
    }
}

pub type Result<T> = core::result::Result<T, WamrError>;

/// Safely converts a C string (`*const c_char`) to a `heapless::String`.
pub(crate) fn c_char_to_string_heapless<const N: usize>(ptr: *const c_char) -> Result<String<N>> {
    if ptr.is_null() {
        return Err(WamrError::NullPointer);
    }
    let cstr = unsafe { CStr::from_ptr(ptr) };
    let rust_str = cstr.to_str().map_err(WamrError::InvalidUtf8)?;
    String::try_from(rust_str).map_err(|_| WamrError::CapacityExceeded)
}
