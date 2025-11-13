use crate::error::{check, Result};
use crate::ffi;
use crate::types::Packet;

pub fn memory(data: &[u8]) -> u32 {
    unsafe { ffi::csp_crc32_memory(data.as_ptr() as *const libc::c_void, data.len() as u32) }
}

pub struct Crc32 {
    state: ffi::csp_crc32_t,
}

impl Crc32 {
    pub fn new() -> Self {
        let mut state: ffi::csp_crc32_t = 0;
        unsafe { ffi::csp_crc32_init(&mut state) };
        Self { state }
    }

    pub fn update(&mut self, data: &[u8]) {
        unsafe {
            ffi::csp_crc32_update(
                &mut self.state,
                data.as_ptr() as *const libc::c_void,
                data.len() as u32,
            )
        };
    }

    pub fn finalize(mut self) -> u32 {
        unsafe { ffi::csp_crc32_final(&mut self.state) }
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

pub fn append_to_packet(packet: &mut Packet) -> Result<()> {
    check(unsafe { ffi::csp_crc32_append(packet.as_ptr()) })
}

pub fn verify_packet(packet: &mut Packet) -> Result<()> {
    check(unsafe { ffi::csp_crc32_verify(packet.as_ptr()) })
}
