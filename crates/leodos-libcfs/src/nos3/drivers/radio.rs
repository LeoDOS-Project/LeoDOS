//! Radio transceiver (ground and inter-satellite links).
//!
//! Simulates an RF front-end for uplink/downlink to a ground
//! station and proximity links between satellites.
//! Communicates over a socket bus.

use crate::ffi;
use crate::nos3::{check_socket, SocketError};
use crate::nos3::buses::socket::Socket;

/// Radio housekeeping telemetry.
#[derive(Debug, Clone, Default)]
pub struct RadioHk {
    /// Device command counter.
    pub device_counter: u32,
    /// Current device configuration.
    pub device_config: u32,
    /// Proximity signal strength.
    pub prox_signal: u32,
}

/// Sets the radio configuration.
pub fn set_configuration(
    device: &mut Socket,
    config: u32,
) -> Result<(), SocketError> {
    check_socket(unsafe {
        ffi::GENERIC_RADIO_SetConfiguration(
            &mut device.inner,
            config,
        )
    })
}

/// Forwards data via proximity link.
pub fn proximity_forward(
    device: &mut Socket,
    scid: u16,
    data: &[u8],
) -> Result<(), SocketError> {
    check_socket(unsafe {
        ffi::GENERIC_RADIO_ProximityForward(
            &mut device.inner,
            scid,
            data.as_ptr() as *mut _,
            data.len() as u16,
        )
    })
}

/// Requests housekeeping telemetry from the radio.
pub fn request_hk(
    device: &mut Socket,
) -> Result<RadioHk, SocketError> {
    let mut raw = ffi::GENERIC_RADIO_Device_HK_tlm_t::default();
    check_socket(unsafe {
        ffi::GENERIC_RADIO_RequestHK(&mut device.inner, &mut raw)
    })?;
    Ok(RadioHk {
        device_counter: raw.DeviceCounter,
        device_config: raw.DeviceConfig,
        prox_signal: raw.ProxSignal,
    })
}
