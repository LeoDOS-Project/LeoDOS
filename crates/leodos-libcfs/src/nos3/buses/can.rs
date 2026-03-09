//! CAN (Controller Area Network) bus.
//!
//! CAN is a multi-master serial bus originally designed for
//! automotive use, adopted in spacecraft for robust,
//! prioritised messaging between subsystems such as IMUs.
//! The device is closed on drop.

use crate::ffi;
use core::mem::MaybeUninit;

/// Errors from CAN bus operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum CanError {
    /// Generic OS/driver error (`CAN_ERROR`).
    #[error("CAN: OS error")]
    OsError,
    /// Interface up error (`CAN_UP_ERR`).
    #[error("CAN: interface up error")]
    UpError,
    /// Interface down error (`CAN_DOWN_ERR`).
    #[error("CAN: interface down error")]
    DownError,
    /// Set modes error (`CAN_SET_MODES_ERR`).
    #[error("CAN: set modes error")]
    SetModes,
    /// Set bitrate error (`CAN_SET_BITRATE_ERR`).
    #[error("CAN: set bitrate error")]
    SetBitrate,
    /// Socket open error (`CAN_SOCK_OPEN_ERR`).
    #[error("CAN: socket open error")]
    SocketOpen,
    /// Socket flag set error (`CAN_SOCK_FLAGSET_ERR`).
    #[error("CAN: socket flag set error")]
    SocketFlagSet,
    /// Socket bind error (`CAN_SOCK_BIND_ERR`).
    #[error("CAN: socket bind error")]
    SocketBind,
    /// Write error (`CAN_WRITE_ERR`).
    #[error("CAN: write error")]
    Write,
    /// Read error (`CAN_READ_ERR`).
    #[error("CAN: read error")]
    Read,
    /// Read timeout (`CAN_READ_TIMEOUT_ERR`).
    #[error("CAN: read timeout")]
    ReadTimeout,
    /// Socket set option error (`CAN_SOCK_SETOPT_ERR`).
    #[error("CAN: socket set option error")]
    SocketSetOpt,
    /// Unhandled error code.
    #[error("CAN: unhandled error ({0})")]
    Unhandled(i32),
}

pub(crate) fn check(rc: i32) -> Result<(), CanError> {
    match rc {
        0 => Ok(()),
        _ if rc == ffi::CAN_ERROR => Err(CanError::OsError),
        _ if rc == ffi::CAN_UP_ERR => Err(CanError::UpError),
        _ if rc == ffi::CAN_DOWN_ERR => Err(CanError::DownError),
        _ if rc == ffi::CAN_SET_MODES_ERR => Err(CanError::SetModes),
        _ if rc == ffi::CAN_SET_BITRATE_ERR => Err(CanError::SetBitrate),
        _ if rc == ffi::CAN_SOCK_OPEN_ERR => Err(CanError::SocketOpen),
        _ if rc == ffi::CAN_SOCK_FLAGSET_ERR => Err(CanError::SocketFlagSet),
        _ if rc == ffi::CAN_SOCK_BIND_ERR => Err(CanError::SocketBind),
        _ if rc == ffi::CAN_WRITE_ERR => Err(CanError::Write),
        _ if rc == ffi::CAN_READ_ERR => Err(CanError::Read),
        _ if rc == ffi::CAN_READ_TIMEOUT_ERR => Err(CanError::ReadTimeout),
        _ if rc == ffi::CAN_SOCK_SETOPT_ERR => Err(CanError::SocketSetOpt),
        other => Err(CanError::Unhandled(other)),
    }
}

/// An open CAN bus device.
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
        check(unsafe { ffi::can_init_dev(&mut info) })?;
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
        check(unsafe { ffi::can_set_modes(&mut self.inner) })
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
        check(unsafe { ffi::can_write(&mut self.inner) })
    }

    /// Reads a CAN frame (non-blocking).
    ///
    /// Returns `(can_id, data_length)`. The payload is copied
    /// into `buf` (up to 8 bytes).
    pub fn read(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(u32, usize), CanError> {
        check(unsafe { ffi::can_read(&mut self.inner) })?;
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
        check(unsafe {
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
