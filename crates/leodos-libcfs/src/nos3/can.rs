//! Safe CAN bus wrapper.
//!
//! Wraps the hwlib `can_*` functions with RAII lifetime
//! management. The device is closed automatically on drop.

use super::{check_can, CanError};
use crate::ffi;
use core::mem::MaybeUninit;

/// An open CAN bus device.
///
/// Created via [`Can::open`]. Automatically closes the device
/// when dropped.
pub struct Can {
    pub(crate) inner: ffi::can_info_t,
}

impl Can {
    /// Opens a CAN bus device.
    ///
    /// - `handle`: network interface index (e.g. 0 for `can0`)
    /// - `bitrate`: bus bit rate in bps
    pub fn open(handle: i32, bitrate: u32) -> Result<Self, CanError> {
        let mut info: ffi::can_info_t = unsafe {
            MaybeUninit::zeroed().assume_init()
        };
        info.handle = handle;
        info.bitrate = bitrate;
        check_can(unsafe { ffi::can_init_dev(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Sets all CAN controller modes at once.
    pub fn set_modes(
        &mut self,
        loopback: bool,
        listen_only: bool,
        triple_sampling: bool,
        one_shot: bool,
        berr_reporting: bool,
        fd: bool,
        presume_ack: bool,
    ) -> Result<(), CanError> {
        self.inner.loopback = loopback;
        self.inner.listenOnly = listen_only;
        self.inner.tripleSampling = triple_sampling;
        self.inner.oneShot = one_shot;
        self.inner.berrReporting = berr_reporting;
        self.inner.fd = fd;
        self.inner.presumeAck = presume_ack;
        check_can(unsafe { ffi::can_set_modes(&mut self.inner) })
    }

    /// Writes a CAN frame.
    ///
    /// `can_id` is the CAN identifier. `data` is the payload
    /// (up to 8 bytes).
    pub fn write(
        &mut self,
        can_id: u32,
        data: &[u8],
    ) -> Result<(), CanError> {
        let len = data.len().min(8);
        self.inner.tx_frame.can_id = can_id;
        self.inner.tx_frame.can_dlc = len as u8;
        self.inner.tx_frame.data[..len]
            .copy_from_slice(&data[..len]);
        check_can(unsafe { ffi::can_write(&mut self.inner) })
    }

    /// Reads a CAN frame (non-blocking).
    ///
    /// Returns `(can_id, data_length)`. The payload is copied
    /// into `buf` (up to 8 bytes).
    pub fn read(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(u32, usize), CanError> {
        check_can(unsafe { ffi::can_read(&mut self.inner) })?;
        let dlc = self.inner.rx_frame.can_dlc as usize;
        let len = dlc.min(buf.len()).min(8);
        buf[..len].copy_from_slice(
            &self.inner.rx_frame.data[..len],
        );
        Ok((self.inner.rx_frame.can_id, dlc))
    }

    /// Performs a master write-then-read transaction.
    ///
    /// Writes `tx` then reads into `rx`. Returns `(can_id, dlc)`
    /// of the received frame.
    pub fn transaction(
        &mut self,
        tx_id: u32,
        tx: &[u8],
        rx: &mut [u8],
    ) -> Result<(u32, usize), CanError> {
        let tx_len = tx.len().min(8);
        self.inner.tx_frame.can_id = tx_id;
        self.inner.tx_frame.can_dlc = tx_len as u8;
        self.inner.tx_frame.data[..tx_len]
            .copy_from_slice(&tx[..tx_len]);
        check_can(unsafe {
            ffi::can_master_transaction(&mut self.inner)
        })?;
        let dlc = self.inner.rx_frame.can_dlc as usize;
        let len = dlc.min(rx.len()).min(8);
        rx[..len].copy_from_slice(
            &self.inner.rx_frame.data[..len],
        );
        Ok((self.inner.rx_frame.can_id, dlc))
    }

    /// Sets the read timeout.
    pub fn set_timeout(&mut self, seconds: u32, microseconds: u32) {
        self.inner.second_timeout = seconds;
        self.inner.microsecond_timeout = microseconds;
    }
}

impl Drop for Can {
    fn drop(&mut self) {
        unsafe { ffi::can_close_device(&mut self.inner); }
    }
}
