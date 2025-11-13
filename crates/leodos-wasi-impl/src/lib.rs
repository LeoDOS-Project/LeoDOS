//! A bridge for hosting WAMR inside a cFS application and exposing cFS services.

#![no_std]

use core::marker::PhantomData;
use heapless::Vec;

use leodos_libcfs as cfs;
use leodos_libwamr as wamr;

mod api; // Private module containing the unsafe extern "C" bridge functions
pub mod error;

use error::{Result, WamrHostError};

const MAX_WASM_SOCKETS: usize = 8;

/// Manages the WAMR runtime and exposed cFS resources.
pub struct WamrHost<'r> {
    runtime: wamr::Runtime,
    // State for exposed services is managed here
    pub(crate) sockets: Vec<Option<cfs::os::net::UdpSocket>, MAX_WASM_SOCKETS>,
    _phantom: PhantomData<&'r ()>,
}

impl<'r> WamrHost<'r> {
    /// Creates a new `WamrHostBuilder` to configure the host environment.
    pub fn builder() -> WamrHostBuilder<'r> {
        WamrHostBuilder::new()
    }

    /// Loads a WebAssembly module from a byte buffer.
    pub fn load_module(&self, wasm_binary: &mut [u8]) -> wamr::Result<wamr::Module<'_>> {
        self.runtime.load(wasm_binary)
    }

    /// After instantiating a module, this function must be called to link
    /// the host's state with the instance.
    pub fn bind_instance(&mut self, instance: &wamr::Instance) {
        // Store a pointer to this host state within the WAMR instance.
        // This is how our bridge functions will get access to the socket pool.
        unsafe {
            instance.set_custom_data(self as *mut Self as *mut _);
        }
    }
}

/// A builder for configuring and creating a `WamrHost`.
pub struct WamrHostBuilder<'r> {
    native_symbols: Vec<wamr::NativeSymbol, 16>, // Pre-allocate for common symbols
    _phantom: PhantomData<&'r ()>,
}

impl<'r> WamrHostBuilder<'r> {
    pub fn new() -> Self {
        Self {
            native_symbols: Vec::new(),
            _phantom: PhantomData,
        }
    }

    /// Exposes a stateful UDP socket API to the Wasm guest.
    pub fn with_udp_sockets(mut self) -> Result<Self> {
        self.native_symbols
            .push(api::udp::socket_open())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::udp::socket_close())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::udp::socket_sendto())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        // Add recvfrom etc. here
        Ok(self)
    }

    /// Builds and initializes the `WamrHost`.
    ///
    /// # Safety
    /// This function must only be called once per cFS application, as it
    /// initializes the global WAMR runtime.
    pub unsafe fn build(mut self) -> wamr::Result<WamrHost<'r>> {
        let runtime = wamr::RuntimeBuilder::new()
            .with_native_symbols("env", &mut self.native_symbols)
            .build()?;

        Ok(WamrHost {
            runtime,
            sockets: Vec::new(),
            _phantom: PhantomData,
        })
    }
}
