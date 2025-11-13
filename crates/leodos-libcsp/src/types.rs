use crate::error::{check, Result};
use crate::ffi;
use bitflags::bitflags;
use core::future::Future;
use core::task::Poll;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Priority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Normal
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ConnectOpts: u32 {
        const NONE = ffi::CSP_O_NONE;
        const RDP = ffi::CSP_O_RDP;
        const NO_RDP = ffi::CSP_O_NORDP;
        const HMAC = ffi::CSP_O_HMAC;
        const NO_HMAC = ffi::CSP_O_NOHMAC;
        const CRC32 = ffi::CSP_O_CRC32;
        const NO_CRC32 = ffi::CSP_O_NOCRC32;
    }
}

impl Default for ConnectOpts {
    fn default() -> Self {
        ConnectOpts::NONE
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SocketOpts: u32 {
        const NONE = ffi::CSP_SO_NONE;
        const RDP_REQUIRED = ffi::CSP_SO_RDPREQ;
        const RDP_PROHIBIT = ffi::CSP_SO_RDPPROHIB;
        const HMAC_REQUIRED = ffi::CSP_SO_HMACREQ;
        const HMAC_PROHIBIT = ffi::CSP_SO_HMACPROHIB;
        const CRC32_REQUIRED = ffi::CSP_SO_CRC32REQ;
        const CRC32_PROHIBIT = ffi::CSP_SO_CRC32PROHIB;
        const CONN_LESS = ffi::CSP_SO_CONN_LESS;
    }
}

impl Default for SocketOpts {
    fn default() -> Self {
        SocketOpts::NONE
    }
}

pub const ANY_PORT: u8 = ffi::CSP_ANY as u8;

pub struct Packet {
    ptr: *mut ffi::csp_packet_t,
}

impl Packet {
    pub(crate) fn from_raw(ptr: *mut ffi::csp_packet_t) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    pub fn data(&self) -> &[u8] {
        unsafe {
            let len = (*self.ptr).length as usize;
            core::slice::from_raw_parts((*self.ptr).__bindgen_anon_1.data.as_ptr(), len)
        }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        unsafe {
            let len = (*self.ptr).length as usize;
            core::slice::from_raw_parts_mut((*self.ptr).__bindgen_anon_1.data.as_mut_ptr(), len)
        }
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.ptr).length as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn set_len(&mut self, len: u16) {
        unsafe {
            (*self.ptr).length = len;
        }
    }

    pub fn capacity(&self) -> usize {
        ffi::CSP_BUFFER_SIZE as usize
    }

    pub(crate) fn as_ptr(&self) -> *mut ffi::csp_packet_t {
        self.ptr
    }

    pub(crate) fn into_raw(self) -> *mut ffi::csp_packet_t {
        let ptr = self.ptr;
        core::mem::forget(self);
        ptr
    }

    pub fn try_clone(&self) -> Option<Self> {
        let ptr = unsafe { ffi::csp_buffer_clone(self.ptr) };
        Self::from_raw(ptr)
    }

    pub fn copy_from(&mut self, src: &Packet) {
        unsafe { ffi::csp_buffer_copy(src.ptr, self.ptr) }
    }

    pub fn src_addr(&self) -> u16 {
        unsafe { (*self.ptr).id.src }
    }

    pub fn dst_addr(&self) -> u16 {
        unsafe { (*self.ptr).id.dst }
    }

    pub fn src_port(&self) -> u8 {
        unsafe { (*self.ptr).id.sport }
    }

    pub fn dst_port(&self) -> u8 {
        unsafe { (*self.ptr).id.dport }
    }

    pub fn priority(&self) -> u8 {
        unsafe { (*self.ptr).id.pri }
    }

    pub fn flags(&self) -> u8 {
        unsafe { (*self.ptr).id.flags }
    }
}

impl Drop for Packet {
    fn drop(&mut self) {
        unsafe {
            ffi::csp_buffer_free(self.ptr as *mut libc::c_void);
        }
    }
}

pub struct Connection {
    ptr: *mut ffi::csp_conn_t,
}

impl Connection {
    pub(crate) fn from_raw(ptr: *mut ffi::csp_conn_t) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }

    pub(crate) fn as_ptr(&self) -> *mut ffi::csp_conn_t {
        self.ptr
    }

    pub fn read(&self, timeout_ms: u32) -> Option<Packet> {
        let ptr = unsafe { ffi::csp_read(self.ptr, timeout_ms) };
        Packet::from_raw(ptr)
    }

    pub fn send(&self, packet: Packet) {
        unsafe {
            ffi::csp_send(self.ptr, packet.into_raw());
        }
    }

    pub fn send_with_prio(&self, prio: Priority, packet: Packet) {
        unsafe {
            ffi::csp_send_prio(prio as u8, self.ptr, packet.into_raw());
        }
    }

    pub fn dst_port(&self) -> i32 {
        unsafe { ffi::csp_conn_dport(self.ptr) }
    }

    pub fn src_port(&self) -> i32 {
        unsafe { ffi::csp_conn_sport(self.ptr) }
    }

    pub fn dst_addr(&self) -> i32 {
        unsafe { ffi::csp_conn_dst(self.ptr) }
    }

    pub fn src_addr(&self) -> i32 {
        unsafe { ffi::csp_conn_src(self.ptr) }
    }

    pub fn is_active(&self) -> bool {
        unsafe { ffi::csp_conn_is_active(self.ptr) }
    }

    pub fn flags(&self) -> i32 {
        unsafe { ffi::csp_conn_flags(self.ptr) }
    }

    pub fn recv_async(&self) -> impl Future<Output = Option<Packet>> + '_ {
        core::future::poll_fn(|_| match self.read(0) {
            Some(packet) => Poll::Ready(Some(packet)),
            None => Poll::Pending,
        })
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            ffi::csp_close(self.ptr);
        }
    }
}

pub struct Socket {
    inner: ffi::csp_socket_t,
}

impl Socket {
    pub fn new(opts: SocketOpts) -> Self {
        Self {
            inner: ffi::csp_socket_t {
                rx_queue: core::ptr::null_mut(),
                rx_queue_static: [0u8; 128],
                rx_queue_static_data: [0; core::mem::size_of::<*mut ffi::csp_packet_t>()
                    * ffi::CSP_CONN_RXQUEUE_LEN as usize],
                opts: opts.bits(),
            },
        }
    }

    pub fn bind(&mut self, port: u8) -> Result<()> {
        check(unsafe { ffi::csp_bind(self.as_mut_ptr(), port) })
    }

    pub fn listen(&mut self, backlog: usize) -> Result<()> {
        check(unsafe { ffi::csp_listen(self.as_mut_ptr(), backlog) })
    }

    pub fn accept(&mut self, timeout_ms: u32) -> Option<Connection> {
        let ptr = unsafe { ffi::csp_accept(self.as_mut_ptr(), timeout_ms) };
        Connection::from_raw(ptr)
    }

    pub fn recvfrom(&mut self, timeout_ms: u32) -> Option<Packet> {
        let ptr = unsafe { ffi::csp_recvfrom(self.as_mut_ptr(), timeout_ms) };
        Packet::from_raw(ptr)
    }

    pub fn accept_async(&mut self) -> impl Future<Output = Option<Connection>> + '_ {
        core::future::poll_fn(|_| match self.accept(0) {
            Some(conn) => Poll::Ready(Some(conn)),
            None => Poll::Pending,
        })
    }

    pub fn recvfrom_async(&mut self) -> impl Future<Output = Option<Packet>> + '_ {
        core::future::poll_fn(|_| match self.recvfrom(0) {
            Some(packet) => Poll::Ready(Some(packet)),
            None => Poll::Pending,
        })
    }

    fn as_mut_ptr(&mut self) -> *mut ffi::csp_socket_t {
        &mut self.inner as *mut ffi::csp_socket_t
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            ffi::csp_socket_close(self.as_mut_ptr());
        }
    }
}

impl Default for Socket {
    fn default() -> Self {
        Self::new(SocketOpts::NONE)
    }
}
