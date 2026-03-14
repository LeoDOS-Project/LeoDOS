//! Safe, idiomatic wrappers for OSAL networking APIs (sockets).

use crate::error::{Error, OsalError, Result};
use crate::ffi;
use crate::os::id::OsalId;
use crate::os::time::OsTime;
use crate::status::check;
use core::ffi::CStr;
use core::fmt::Write;
use core::future::Future;
use core::mem::MaybeUninit;
use core::task::Poll;
use heapless::{CString, String};

/// A wrapper for a CFE/OSAL socket address.
#[derive(Clone)]
#[repr(transparent)]
pub struct SocketAddr(ffi::OS_SockAddr_t);

/// Defines how to shut down a TCP stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SocketShutdownMode {
    /// Disable future reading.
    Read = ffi::OS_SocketShutdownMode_t_OS_SocketShutdownMode_SHUT_READ,
    /// Disable future writing.
    Write = ffi::OS_SocketShutdownMode_t_OS_SocketShutdownMode_SHUT_WRITE,
    /// Disable future reading and writing.
    ReadWrite = ffi::OS_SocketShutdownMode_t_OS_SocketShutdownMode_SHUT_READWRITE,
}

impl SocketAddr {
    /// Creates a new socket address.
    pub fn new_ipv4(ip_addr: &str, port: u16) -> Result<Self> {
        let mut addr_uninit = MaybeUninit::uninit();

        // 1. Initialize for the correct domain
        check(unsafe {
            ffi::OS_SocketAddrInit(addr_uninit.as_mut_ptr(), SocketDomain::IPv4.into())
        })?;

        // 2. Set the IP address from a string
        let mut c_ip: String<{ ffi::OS_MAX_PATH_LEN as usize }> = String::new();
        c_ip.push_str(ip_addr)
            .map_err(|_| Error::OsErrNameTooLong)?;
        c_ip.push('\0').map_err(|_| Error::OsErrNameTooLong)?;
        check(unsafe {
            ffi::OS_SocketAddrFromString(
                addr_uninit.as_mut_ptr(),
                c_ip.as_ptr() as *const libc::c_char,
            )
        })?;

        // 3. Set the port
        check(unsafe { ffi::OS_SocketAddrSetPort(addr_uninit.as_mut_ptr(), port) })?;

        Ok(Self(unsafe { addr_uninit.assume_init() }))
    }

    /// Gets the port number from a socket address.
    pub fn port(&self) -> Result<u16> {
        let mut port = MaybeUninit::uninit();
        check(unsafe { ffi::OS_SocketAddrGetPort(port.as_mut_ptr(), &self.0) })?;
        Ok(unsafe { port.assume_init() })
    }

    /// Gets a string representation of the host address (e.g., "127.0.0.1").
    pub fn to_string(&self) -> Result<String<{ ffi::OS_MAX_PATH_LEN as usize }>> {
        let mut buffer = [0u8; { ffi::OS_MAX_PATH_LEN as usize }];
        check(unsafe {
            ffi::OS_SocketAddrToString(
                buffer.as_mut_ptr() as *mut libc::c_char,
                buffer.len(),
                &self.0,
            )
        })?;
        let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
        let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| Error::OsErrNameTooLong)?;
        let s = String::from_utf8(vec).map_err(|_| Error::InvalidString)?;
        Ok(s)
    }
}

impl TryFrom<core::net::SocketAddr> for SocketAddr {
    type Error = Error;

    fn try_from(addr: core::net::SocketAddr) -> Result<Self> {
        let mut addr_uninit = MaybeUninit::uninit();

        let domain = match addr {
            core::net::SocketAddr::V4(_) => SocketDomain::IPv4,
            core::net::SocketAddr::V6(_) => SocketDomain::IPv6,
        };

        check(unsafe { ffi::OS_SocketAddrInit(addr_uninit.as_mut_ptr(), domain.into()) })?;

        // Format IP to C-String for OSAL
        let mut c_ip: String<{ ffi::OS_MAX_PATH_LEN as usize }> = String::new();
        write!(c_ip, "{}", addr.ip()).map_err(|_| Error::OsErrNameTooLong)?;
        c_ip.push('\0').map_err(|_| Error::OsErrNameTooLong)?;

        check(unsafe {
            ffi::OS_SocketAddrFromString(
                addr_uninit.as_mut_ptr(),
                c_ip.as_ptr() as *const libc::c_char,
            )
        })?;

        check(unsafe { ffi::OS_SocketAddrSetPort(addr_uninit.as_mut_ptr(), addr.port()) })?;

        Ok(Self(unsafe { addr_uninit.assume_init() }))
    }
}

/// A raw socket handle that ensures `OS_close` is called on drop.
#[derive(Debug)]
#[repr(transparent)]
pub struct Socket(ffi::osal_id_t);

impl Drop for Socket {
    fn drop(&mut self) {
        if self.0 != 0 {
            let _ = unsafe { ffi::OS_close(self.0) };
        }
    }
}

impl Socket {
    /// Binds a socket to a given local address.
    pub fn bind_address(&self, addr: &SocketAddr) -> Result<()> {
        check(unsafe { ffi::OS_SocketBindAddress(self.0, &addr.0) })?;
        Ok(())
    }
}

/// A UDP socket.
#[derive(Debug)]
#[repr(transparent)]
pub struct UdpSocket(Socket);

/// Timeout options for socket operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timeout {
    /// Wait indefinitely.
    Pend,
    /// Do not wait at all.
    Poll,
    /// Wait for the specified number of milliseconds.
    Milliseconds(i32),
}

impl From<Timeout> for i32 {
    fn from(timeout: Timeout) -> Self {
        match timeout {
            Timeout::Pend => ffi::OS_PEND,
            Timeout::Poll => ffi::OS_CHECK as i32,
            Timeout::Milliseconds(ms) => ms,
        }
    }
}

impl UdpSocket {
    /// Creates a new UDP socket bound to the specified address.
    pub fn bind(addr: SocketAddr) -> Result<UdpSocket> {
        let mut sock_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_SocketOpen(
                sock_id.as_mut_ptr(),
                SocketDomain::IPv4.into(),
                ffi::OS_SocketType_t_OS_SocketType_DATAGRAM,
            )
        })?;
        let sock_id = unsafe { sock_id.assume_init() };

        check(unsafe { ffi::OS_SocketBind(sock_id, &addr.0) })?;

        Ok(UdpSocket(Socket(sock_id)))
    }

    /// Receives a single datagram message on the socket.
    pub fn recv_from<'a>(
        &self,
        buf: &'a mut [u8],
        timeout: Timeout,
    ) -> Result<(usize, SocketAddr)> {
        let mut remote_addr_uninit = MaybeUninit::<ffi::OS_SockAddr_t>::uninit();

        let num_bytes = unsafe {
            ffi::OS_SocketRecvFrom(
                (self.0).0,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                remote_addr_uninit.as_mut_ptr(),
                timeout.into(),
            )
        };

        if num_bytes < 0 {
            Err(Error::from(num_bytes))
        } else {
            let remote_addr = unsafe { remote_addr_uninit.assume_init() };
            Ok((num_bytes as usize, SocketAddr(remote_addr)))
        }
    }

    /// Sends data on the socket to the given address.
    pub fn send_to(&self, buf: &[u8], target: &SocketAddr) -> Result<usize> {
        let num_bytes = unsafe {
            ffi::OS_SocketSendTo((self.0).0, buf.as_ptr() as *const _, buf.len(), &target.0)
        };
        if num_bytes < 0 {
            Err(Error::from(num_bytes))
        } else {
            Ok(num_bytes as usize)
        }
    }

    /// Receives a datagram with an absolute timeout.
    pub fn recv_from_abs<'a>(
        &self,
        buf: &'a mut [u8],
        abstime: OsTime,
    ) -> Result<(usize, SocketAddr)> {
        let mut remote_addr_uninit = MaybeUninit::<ffi::OS_SockAddr_t>::uninit();
        let num_bytes = unsafe {
            ffi::OS_SocketRecvFromAbs(
                (self.0).0,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                remote_addr_uninit.as_mut_ptr(),
                abstime.0,
            )
        };

        if num_bytes < 0 {
            Err(Error::from(num_bytes))
        } else {
            let remote_addr = SocketAddr(unsafe { remote_addr_uninit.assume_init() });
            Ok((num_bytes as usize, remote_addr))
        }
    }
}

/// A TCP socket server, listening for connections.
#[derive(Debug)]
#[repr(transparent)]
pub struct TcpListener(Socket);

/// The domain of a socket.
pub enum SocketDomain {
    /// IPv4 (Inet)
    IPv4,
    /// IPv6 (Inet6)
    IPv6,
}

impl Into<ffi::OS_SocketDomain_t> for SocketDomain {
    fn into(self) -> ffi::OS_SocketDomain_t {
        match self {
            SocketDomain::IPv4 => ffi::OS_SocketDomain_t_OS_SocketDomain_INET,
            SocketDomain::IPv6 => ffi::OS_SocketDomain_t_OS_SocketDomain_INET6,
        }
    }
}

impl TcpListener {
    /// Creates a new `TcpListener` which will be bound to the specified address.
    pub fn bind(addr: SocketAddr) -> Result<Self> {
        let mut sock_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_SocketOpen(
                sock_id.as_mut_ptr(),
                SocketDomain::IPv4.into(),
                ffi::OS_SocketType_t_OS_SocketType_STREAM,
            )
        })?;
        let sock_id = unsafe { sock_id.assume_init() };

        check(unsafe { ffi::OS_SocketBind(sock_id, &addr.0) })?;

        Ok(TcpListener(Socket(sock_id)))
    }

    /// Accepts a new incoming connection from this listener.
    /// This function will block until a new connection is established.
    pub fn accept(&self, timeout: Timeout) -> Result<(TcpStream, SocketAddr)> {
        let mut remote_addr_uninit = MaybeUninit::<ffi::OS_SockAddr_t>::uninit();
        let mut conn_sock_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_SocketAccept(
                (self.0).0,
                conn_sock_id.as_mut_ptr(),
                remote_addr_uninit.as_mut_ptr(),
                timeout.into(),
            )
        })?;

        let stream = TcpStream(Socket(unsafe { conn_sock_id.assume_init() }));
        let remote_addr = SocketAddr(unsafe { remote_addr_uninit.assume_init() });
        Ok((stream, remote_addr))
    }

    /// Places the socket into a listening state for incoming connections.
    ///
    /// This is typically called after `bind`.
    pub fn listen(&self) -> Result<()> {
        check(unsafe { ffi::OS_SocketListen(self.0 .0) })?;
        Ok(())
    }
}

/// A TCP stream between a local and a remote socket.
#[derive(Debug)]
#[repr(transparent)]
pub struct TcpStream(Socket);

impl TcpStream {
    /// Opens a TCP connection to a remote host.
    pub fn connect(addr: SocketAddr, domain: SocketDomain) -> Result<Self> {
        let mut sock_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_SocketOpen(
                sock_id.as_mut_ptr(),
                domain.into(),
                ffi::OS_SocketType_t_OS_SocketType_STREAM,
            )
        })?;
        let sock_id = unsafe { sock_id.assume_init() };

        check(unsafe { ffi::OS_SocketConnect(sock_id, &addr.0, ffi::OS_PEND as i32) })?;

        Ok(TcpStream(Socket(sock_id)))
    }

    /// Reads some bytes from the stream into the specified buffer.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let bytes_read = unsafe { ffi::OS_read(self.0 .0, buf.as_mut_ptr() as *mut _, buf.len()) };
        if bytes_read < 0 {
            Err(Error::from(bytes_read))
        } else {
            Ok(bytes_read as usize)
        }
    }

    /// Writes a buffer to the stream.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let bytes_written =
            unsafe { ffi::OS_write(self.0 .0, buf.as_ptr() as *const _, buf.len()) };
        if bytes_written < 0 {
            Err(Error::from(bytes_written))
        } else {
            Ok(bytes_written as usize)
        }
    }

    /// Accepts a new connection with an absolute timeout.
    pub fn accept_abs(&self, abstime: OsTime) -> Result<(TcpStream, SocketAddr)> {
        let mut remote_addr_uninit = MaybeUninit::<ffi::OS_SockAddr_t>::uninit();
        let mut conn_sock_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_SocketAcceptAbs(
                self.0 .0,
                conn_sock_id.as_mut_ptr(),
                remote_addr_uninit.as_mut_ptr(),
                abstime.0,
            )
        })?;

        let stream = TcpStream(Socket(unsafe { conn_sock_id.assume_init() }));
        let remote_addr = SocketAddr(unsafe { remote_addr_uninit.assume_init() });
        Ok((stream, remote_addr))
    }

    /// Opens a TCP connection to a remote host with an absolute timeout.
    pub fn connect_abs(addr: SocketAddr, domain: SocketDomain, abstime: OsTime) -> Result<Self> {
        let mut sock_id = MaybeUninit::uninit();
        check(unsafe {
            ffi::OS_SocketOpen(
                sock_id.as_mut_ptr(),
                domain.into(),
                ffi::OS_SocketType_t_OS_SocketType_STREAM,
            )
        })?;
        let sock_id = unsafe { sock_id.assume_init() };

        check(unsafe { ffi::OS_SocketConnectAbs(sock_id, &addr.0, abstime.0) })?;

        Ok(TcpStream(Socket(sock_id)))
    }

    /// Gracefully shuts down the read, write, or both halves of the connection.
    pub fn shutdown(&self, how: SocketShutdownMode) -> Result<()> {
        check(unsafe { ffi::OS_SocketShutdown(self.0 .0, how as ffi::OS_SocketShutdownMode_t) })?;
        Ok(())
    }
}

/// Gets the OSAL-specific network ID of the local machine.
pub fn get_network_id() -> i32 {
    unsafe { ffi::OS_NetworkGetID() }
}

/// Gets the local machine's network host name.
pub fn get_host_name() -> Result<String<{ ffi::OS_MAX_PATH_LEN as usize }>> {
    let mut buffer = [0u8; { ffi::OS_MAX_PATH_LEN as usize }];
    check(unsafe {
        ffi::OS_NetworkGetHostName(buffer.as_mut_ptr() as *mut libc::c_char, buffer.len())
    })?;
    let len = buffer.iter().position(|&b| b == 0).unwrap_or(buffer.len());
    let vec = heapless::Vec::from_slice(&buffer[..len]).map_err(|_| Error::OsErrNameTooLong)?;
    let s = String::from_utf8(vec).map_err(|_| Error::InvalidString)?;
    Ok(s)
}

/// A set of file descriptors, for use with `select`.
#[derive(Debug, Clone, Copy)]
pub struct FdSet(ffi::OS_FdSet);

impl FdSet {
    /// Creates a new, empty file descriptor set.
    pub fn new() -> Self {
        let mut set = MaybeUninit::uninit();
        // Should not fail.
        unsafe { ffi::OS_SelectFdZero(set.as_mut_ptr()) };
        Self(unsafe { set.assume_init() })
    }

    /// Clears all file descriptors from the set.
    pub fn zero(&mut self) {
        let _ = unsafe { ffi::OS_SelectFdZero(&mut self.0) };
    }

    /// Adds a file descriptor to the set.
    pub fn add(&mut self, id: OsalId) {
        let _ = unsafe { ffi::OS_SelectFdAdd(&mut self.0, id.0) };
    }

    /// Removes a file descriptor from the set.
    pub fn clear(&mut self, id: OsalId) {
        let _ = unsafe { ffi::OS_SelectFdClear(&mut self.0, id.0) };
    }

    /// Checks if a file descriptor is a member of the set.
    pub fn is_set(&self, id: OsalId) -> bool {
        unsafe { ffi::OS_SelectFdIsSet(&self.0, id.0) }
    }
}

impl Default for FdSet {
    fn default() -> Self {
        Self::new()
    }
}

/// Waits for events on a single file handle with a relative timeout.
pub fn select_single(id: OsalId, state: &mut u32, timeout_ms: i32) -> Result<()> {
    check(unsafe { ffi::OS_SelectSingle(id.0, state, timeout_ms) })?;
    Ok(())
}

/// Waits for events on a single file handle with an absolute timeout.
pub fn select_single_abs(id: OsalId, state: &mut u32, abstime: OsTime) -> Result<()> {
    check(unsafe { ffi::OS_SelectSingleAbs(id.0, state, abstime.0) })?;
    Ok(())
}

/// Waits for events across multiple file handles with a relative timeout.
pub fn select_multiple(
    read_set: Option<&mut FdSet>,
    write_set: Option<&mut FdSet>,
    timeout_ms: i32,
) -> Result<()> {
    let read_ptr = read_set.map_or(core::ptr::null_mut(), |s| &mut s.0);
    let write_ptr = write_set.map_or(core::ptr::null_mut(), |s| &mut s.0);
    check(unsafe { ffi::OS_SelectMultiple(read_ptr, write_ptr, timeout_ms) })?;
    Ok(())
}

/// Waits for events across multiple file handles with an absolute timeout.
pub fn select_multiple_abs(
    read_set: Option<&mut FdSet>,
    write_set: Option<&mut FdSet>,
    abstime: OsTime,
) -> Result<()> {
    let read_ptr = read_set.map_or(core::ptr::null_mut(), |s| &mut s.0);
    let write_ptr = write_set.map_or(core::ptr::null_mut(), |s| &mut s.0);
    check(unsafe { ffi::OS_SelectMultipleAbs(read_ptr, write_ptr, abstime.0) })?;
    Ok(())
}

/// Properties of an OSAL socket.
#[derive(Debug, Clone)]
pub struct SocketProp {
    /// The registered name of the socket.
    pub name: String<{ ffi::OS_MAX_API_NAME as usize }>,
    /// The OSAL ID of the task that created the socket.
    pub creator: OsalId,
}

/// Finds an existing socket ID by its name.
pub fn get_socket_id_by_name(name: &str) -> Result<OsalId> {
    let mut c_name = CString::<{ ffi::OS_MAX_API_NAME as usize }>::new();
    c_name
        .extend_from_bytes(name.as_bytes())
        .map_err(|_| Error::OsErrNameTooLong)?;
    let mut sock_id = MaybeUninit::uninit();
    check(unsafe { ffi::OS_SocketGetIdByName(sock_id.as_mut_ptr(), c_name.as_ptr()) })?;
    Ok(OsalId(unsafe { sock_id.assume_init() }))
}

/// Retrieves information about a socket.
pub fn get_socket_info(sock_id: OsalId) -> Result<SocketProp> {
    let mut prop = MaybeUninit::<ffi::OS_socket_prop_t>::uninit();
    check(unsafe { ffi::OS_SocketGetInfo(sock_id.0, prop.as_mut_ptr()) })?;
    let prop = unsafe { prop.assume_init() };

    let name_cstr = unsafe { CStr::from_ptr(prop.name.as_ptr()) };
    let name_str = name_cstr.to_str().map_err(|_| Error::InvalidString)?;
    let name = String::try_from(name_str).map_err(|_| Error::OsErrNameTooLong)?;

    Ok(SocketProp {
        name,
        creator: OsalId(prop.creator),
    })
}

impl UdpSocket {
    /// Asynchronously receives a single datagram message on the socket.
    pub fn recv<'a>(
        &'a self,
        buf: &'a mut [u8],
    ) -> impl Future<Output = Result<(usize, SocketAddr)>> + use<'a> {
        core::future::poll_fn(|_| {
            let recv_future = self.recv_from(buf, Timeout::Poll);
            match recv_future {
                Err(Error::Osal(OsalError::Timeout | OsalError::QueueEmpty)) => Poll::Pending,
                Ok(result) => Poll::Ready(Ok(result)),
                Err(e) => Poll::Ready(Err(e)),
            }
        })
    }

    /// Asynchronously sends data on the socket to the given address.
    pub fn send<'a>(
        &'a self,
        buf: &'a [u8],
        target: &'a SocketAddr,
    ) -> impl Future<Output = Result<usize>> + use<'a> {
        core::future::poll_fn(|_| {
            // send_to is typically non-blocking for UDP, but wrap it anyway
            match self.send_to(buf, target) {
                Err(Error::Osal(OsalError::Timeout | OsalError::QueueEmpty)) => Poll::Pending,
                Ok(result) => Poll::Ready(Ok(result)),
                Err(e) => Poll::Ready(Err(e)),
            }
        })
    }
}
