use crate::ffi;

#[derive(Debug, Clone, Copy)]
pub struct RdpOptions {
    pub window_size: u32,
    pub conn_timeout_ms: u32,
    pub packet_timeout_ms: u32,
    pub delayed_acks: u32,
    pub ack_timeout: u32,
    pub ack_delay_count: u32,
}

impl RdpOptions {
    pub fn get() -> Self {
        let mut opts = Self::default();
        unsafe {
            ffi::csp_rdp_get_opt(
                &mut opts.window_size,
                &mut opts.conn_timeout_ms,
                &mut opts.packet_timeout_ms,
                &mut opts.delayed_acks,
                &mut opts.ack_timeout,
                &mut opts.ack_delay_count,
            );
        }
        opts
    }

    pub fn set(&self) {
        unsafe {
            ffi::csp_rdp_set_opt(
                self.window_size,
                self.conn_timeout_ms,
                self.packet_timeout_ms,
                self.delayed_acks,
                self.ack_timeout,
                self.ack_delay_count,
            );
        }
    }
}

impl Default for RdpOptions {
    fn default() -> Self {
        Self {
            window_size: 4,
            conn_timeout_ms: 10000,
            packet_timeout_ms: 5000,
            delayed_acks: 1,
            ack_timeout: 2000,
            ack_delay_count: 4,
        }
    }
}
