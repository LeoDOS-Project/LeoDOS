//! Safe socket wrapper.
//!
//! Wraps the hwlib `socket_*` functions with RAII lifetime
//! management. The socket is closed automatically on drop.

use super::{check_socket, SocketError};
use crate::ffi;
use core::mem::MaybeUninit;

/// IP address family.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AddrFamily {
    /// IPv4.
    V4,
    /// IPv6.
    V6,
}

/// Socket type.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SockType {
    /// Stream (TCP).
    Stream,
    /// Datagram (UDP).
    Dgram,
}

/// Socket role.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Category {
    /// Server socket (binds and listens).
    Server,
    /// Client socket (connects).
    Client,
}

/// An open network socket.
///
/// Created via [`Socket::create`]. Automatically closed on drop.
pub struct Socket {
    pub(crate) inner: ffi::socket_info_t,
}

impl Socket {
    /// Creates a new socket.
    ///
    /// - `ip`: local IP address (as C string, e.g. `"0.0.0.0"`)
    /// - `port`: local port number
    /// - `family`: IPv4 or IPv6
    /// - `sock_type`: stream or datagram
    /// - `category`: server or client
    /// - `blocking`: whether I/O blocks
    pub fn create(
        ip: &core::ffi::CStr,
        port: i32,
        family: AddrFamily,
        sock_type: SockType,
        category: Category,
        blocking: bool,
    ) -> Result<Self, SocketError> {
        let mut info: ffi::socket_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.ip_address = ip.as_ptr() as *mut _;
        info.port_num = port;
        info.address_family = match family {
            AddrFamily::V4 => ffi::addr_fam_e_ip_ver_4,
            AddrFamily::V6 => ffi::addr_fam_e_ip_ver_6,
        };
        info.type_ = match sock_type {
            SockType::Stream => ffi::type_e_stream,
            SockType::Dgram => ffi::type_e_dgram,
        };
        info.category = match category {
            Category::Server => ffi::category_e_server,
            Category::Client => ffi::category_e_client,
        };
        info.block = blocking;
        info.created = false;
        check_socket(unsafe { ffi::socket_create(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Starts listening for connections (server, stream only).
    pub fn listen(&mut self) -> Result<(), SocketError> {
        check_socket(unsafe { ffi::socket_listen(&mut self.inner) })
    }

    /// Accepts an incoming connection (server, stream only).
    pub fn accept(&mut self) -> Result<(), SocketError> {
        check_socket(unsafe { ffi::socket_accept(&mut self.inner) })
    }

    /// Connects to a remote address (client).
    pub fn connect(
        &mut self,
        remote_ip: &core::ffi::CStr,
        remote_port: i32,
    ) -> Result<(), SocketError> {
        check_socket(unsafe {
            ffi::socket_connect(
                &mut self.inner,
                remote_ip.as_ptr() as *mut _,
                remote_port,
            )
        })
    }

    /// Sends data.
    ///
    /// For datagrams, specify `remote_ip` and `remote_port`.
    /// For streams, these are ignored (pass empty CStr and 0).
    /// Returns the number of bytes sent.
    pub fn send(
        &mut self,
        data: &[u8],
        remote_ip: &core::ffi::CStr,
        remote_port: i32,
    ) -> Result<usize, SocketError> {
        let mut bytes_sent: usize = 0;
        check_socket(unsafe {
            ffi::socket_send(
                &mut self.inner,
                data.as_ptr() as *mut _,
                data.len(),
                &mut bytes_sent,
                remote_ip.as_ptr() as *mut _,
                remote_port,
            )
        })?;
        Ok(bytes_sent)
    }

    /// Receives data into `buf`.
    ///
    /// Returns the number of bytes received.
    pub fn recv(
        &mut self,
        buf: &mut [u8],
    ) -> Result<usize, SocketError> {
        let mut bytes_recvd: usize = 0;
        check_socket(unsafe {
            ffi::socket_recv(
                &mut self.inner,
                buf.as_mut_ptr(),
                buf.len(),
                &mut bytes_recvd,
            )
        })?;
        Ok(bytes_recvd)
    }

    /// Enables or disables keep-alive.
    pub fn set_keep_alive(&mut self, keep_alive: bool) {
        self.inner.keep_alive = keep_alive;
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe { ffi::socket_close(&mut self.inner); }
    }
}
