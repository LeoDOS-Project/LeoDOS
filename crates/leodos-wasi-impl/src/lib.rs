//! A bridge for hosting WAMR inside a cFS application and exposing cFS services.

#![no_std]

use core::marker::PhantomData;
use heapless::Vec;

use leodos_libcfs as cfs;
use leodos_libwamr as wamr;

mod api;
pub mod error;

use error::{Result, WamrHostError};

pub const MAX_UDP_SOCKETS: usize = 8;
pub const MAX_TCP_STREAMS: usize = 16;
pub const MAX_TCP_LISTENERS: usize = 4;
pub const MAX_FILES: usize = 16;
pub const MAX_DIRECTORIES: usize = 4;

pub struct WamrHost<'r> {
    runtime: wamr::Runtime,
    pub(crate) udp_sockets: Vec<Option<cfs::os::net::UdpSocket>, MAX_UDP_SOCKETS>,
    pub(crate) tcp_streams: Vec<Option<cfs::os::net::TcpStream>, MAX_TCP_STREAMS>,
    pub(crate) tcp_listeners: Vec<Option<cfs::os::net::TcpListener>, MAX_TCP_LISTENERS>,
    pub(crate) files: Vec<Option<cfs::os::fs::File>, MAX_FILES>,
    pub(crate) directories: Vec<Option<cfs::os::fs::Directory>, MAX_DIRECTORIES>,
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

pub struct WamrHostBuilder<'r> {
    native_symbols: Vec<wamr::NativeSymbol, 64>,
    _phantom: PhantomData<&'r ()>,
}

impl<'r> WamrHostBuilder<'r> {
    pub fn new() -> Self {
        Self {
            native_symbols: Vec::new(),
            _phantom: PhantomData,
        }
    }

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
        self.native_symbols
            .push(api::udp::socket_recvfrom())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        Ok(self)
    }

    pub fn with_tcp_sockets(mut self) -> Result<Self> {
        self.native_symbols
            .push(api::tcp::tcp_listen())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::tcp::tcp_accept())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::tcp::tcp_listener_close())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::tcp::tcp_connect())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::tcp::tcp_read())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::tcp::tcp_write())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::tcp::tcp_close())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        Ok(self)
    }

    pub fn with_filesystem(mut self) -> Result<Self> {
        self.native_symbols
            .push(api::fs::file_open())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::file_create())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::file_close())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::file_read())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::file_write())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::file_seek())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::file_stat())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::fs_stat())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::fs_mkdir())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::fs_rmdir())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::fs_remove())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::fs::fs_rename())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        Ok(self)
    }

    pub fn with_directories(mut self) -> Result<Self> {
        self.native_symbols
            .push(api::dir::dir_open())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::dir::dir_close())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::dir::dir_read())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        self.native_symbols
            .push(api::dir::dir_rewind())
            .map_err(|_| WamrHostError::BuilderCapacityExceeded)?;
        Ok(self)
    }

    pub fn with_all(self) -> Result<Self> {
        self.with_udp_sockets()?
            .with_tcp_sockets()?
            .with_filesystem()?
            .with_directories()
    }

    pub unsafe fn build(mut self) -> wamr::Result<WamrHost<'r>> {
        let runtime = wamr::RuntimeBuilder::new()
            .with_native_symbols("env", &mut self.native_symbols)
            .build()?;

        Ok(WamrHost {
            runtime,
            udp_sockets: Vec::new(),
            tcp_streams: Vec::new(),
            tcp_listeners: Vec::new(),
            files: Vec::new(),
            directories: Vec::new(),
            _phantom: PhantomData,
        })
    }
}
