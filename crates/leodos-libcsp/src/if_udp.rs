use crate::ffi;
use crate::iface::Interface;

pub struct UdpConfig {
    inner: ffi::csp_if_udp_conf_t,
    host_buf: [u8; 64],
}

impl UdpConfig {
    pub fn new(host: &str, local_port: i32, remote_port: i32) -> Self {
        let mut host_buf = [0u8; 64];
        let len = host.len().min(host_buf.len() - 1);
        host_buf[..len].copy_from_slice(&host.as_bytes()[..len]);

        let mut inner = ffi::csp_if_udp_conf_t::default();
        inner.lport = local_port;
        inner.rport = remote_port;

        Self { inner, host_buf }
    }

    pub fn local_port(&self) -> i32 {
        self.inner.lport
    }

    pub fn remote_port(&self) -> i32 {
        self.inner.rport
    }
}

pub fn init(iface: &mut Interface, config: &mut UdpConfig) {
    config.inner.host = config.host_buf.as_mut_ptr() as *mut libc::c_char;
    unsafe { ffi::csp_if_udp_init(iface.as_ptr(), &mut config.inner) }
}
