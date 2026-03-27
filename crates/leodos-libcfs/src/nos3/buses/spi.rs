//! SPI (Serial Peripheral Interface) bus.
//!
//! SPI is a high-speed synchronous serial bus used by
//! spacecraft subsystems such as fine sun sensors (FSS),
//! magnetometers, and cameras. The device is closed on drop.

use crate::ffi;
use core::mem::MaybeUninit;

/// Errors from SPI operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum SpiError {
    /// Generic OS/driver error (`SPI_ERROR`).
    #[error("SPI: OS error")]
    OsError,
    /// File open error (`SPI_ERR_FILE_OPEN`).
    #[error("SPI: file open error")]
    FileOpen,
    /// File handle error (`SPI_ERR_FILE_HANDLE`).
    #[error("SPI: file handle error")]
    FileHandle,
    /// File close error (`SPI_ERR_FILE_CLOSE`).
    #[error("SPI: file close error")]
    FileClose,
    /// Invalid SPI mode (`SPI_ERR_INVAL_MD`).
    #[error("SPI: invalid mode")]
    InvalidMode,
    /// IOC message error (`SPI_ERR_IOC_MSG`).
    #[error("SPI: IOC message error")]
    IocMsg,
    /// Write mode error (`SPI_ERR_WR_MODE`).
    #[error("SPI: write mode error")]
    WriteMode,
    /// Read mode error (`SPI_ERR_RD_MODE`).
    #[error("SPI: read mode error")]
    ReadMode,
    /// Write bits-per-word error (`SPI_ERR_WR_BPW`).
    #[error("SPI: write bits-per-word error")]
    WriteBpw,
    /// Read bits-per-word error (`SPI_ERR_RD_BPW`).
    #[error("SPI: read bits-per-word error")]
    ReadBpw,
    /// Write speed error (`SPI_ERR_WR_SD_HZ`).
    #[error("SPI: write speed error")]
    WriteSpeed,
    /// Read speed error (`SPI_ERR_RD_SD_HZ`).
    #[error("SPI: read speed error")]
    ReadSpeed,
    /// Mutex create error (`SPI_ERR_MUTEX_CREATE`).
    #[error("SPI: mutex create error")]
    MutexCreate,
    /// Unhandled error code.
    #[error("SPI: unhandled error ({0})")]
    Unhandled(i32),
}

pub(crate) fn check(rc: i32) -> Result<(), SpiError> {
    match rc {
        0 => Ok(()),
        ffi::SPI_ERROR => Err(SpiError::OsError),
        ffi::SPI_ERR_FILE_OPEN => Err(SpiError::FileOpen),
        ffi::SPI_ERR_FILE_HANDLE => Err(SpiError::FileHandle),
        ffi::SPI_ERR_FILE_CLOSE => Err(SpiError::FileClose),
        ffi::SPI_ERR_INVAL_MD => Err(SpiError::InvalidMode),
        ffi::SPI_ERR_IOC_MSG => Err(SpiError::IocMsg),
        ffi::SPI_ERR_WR_MODE => Err(SpiError::WriteMode),
        ffi::SPI_ERR_RD_MODE => Err(SpiError::ReadMode),
        ffi::SPI_ERR_WR_BPW => Err(SpiError::WriteBpw),
        ffi::SPI_ERR_RD_BPW => Err(SpiError::ReadBpw),
        ffi::SPI_ERR_WR_SD_HZ => Err(SpiError::WriteSpeed),
        ffi::SPI_ERR_RD_SD_HZ => Err(SpiError::ReadSpeed),
        ffi::SPI_ERR_MUTEX_CREATE => Err(SpiError::MutexCreate),
        other => Err(SpiError::Unhandled(other)),
    }
}

/// An open SPI device.
pub struct Spi {
    pub(crate) inner: ffi::spi_info_t,
}

impl Spi {
    /// Opens an SPI device.
    ///
    /// - `device`: OS device path (e.g. `/dev/spidev0.0`)
    /// - `bus`: bus number (for mutex selection)
    /// - `cs`: chip-select line
    /// - `baudrate`: clock speed in Hz
    /// - `spi_mode`: SPI mode (0-3)
    /// - `bits_per_word`: typically 8
    pub fn open(
        device: &core::ffi::CStr,
        bus: u8,
        cs: u8,
        baudrate: u32,
        spi_mode: u8,
        bits_per_word: u8,
    ) -> Result<Self, SpiError> {
        let mut info: ffi::spi_info_t = unsafe { MaybeUninit::zeroed().assume_init() };
        info.deviceString = device.as_ptr();
        info.bus = bus;
        info.cs = cs;
        info.baudrate = baudrate;
        info.spi_mode = spi_mode;
        info.bits_per_word = bits_per_word;
        info.isOpen = 0;
        check(unsafe { ffi::spi_init_dev(&mut info) })?;
        Ok(Self { inner: info })
    }

    /// Writes bytes to the SPI device.
    pub fn write(&mut self, data: &[u8]) -> Result<(), SpiError> {
        check(unsafe {
            ffi::spi_write(&mut self.inner, data.as_ptr() as *mut u8, data.len() as u32)
        })
    }

    /// Reads bytes from the SPI device.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<(), SpiError> {
        check(unsafe { ffi::spi_read(&mut self.inner, buf.as_mut_ptr(), buf.len() as u32) })
    }

    /// Performs a full-duplex SPI transaction.
    ///
    /// - `tx`: data to transmit (or empty to shift out zeros)
    /// - `rx`: buffer for received data (or empty to discard)
    /// - `len`: number of bytes to transfer
    /// - `delay`: post-transfer delay (µs)
    /// - `bits`: bits per word for this transfer
    /// - `deselect`: if true, deassert CS after transfer
    pub fn transfer(
        &mut self,
        tx: &[u8],
        rx: &mut [u8],
        len: u32,
        delay: u16,
        bits: u8,
        deselect: bool,
    ) -> Result<(), SpiError> {
        let tx_ptr = if tx.is_empty() {
            core::ptr::null_mut()
        } else {
            tx.as_ptr() as *mut u8
        };
        let rx_ptr = if rx.is_empty() {
            core::ptr::null_mut()
        } else {
            rx.as_mut_ptr()
        };
        check(unsafe {
            ffi::spi_transaction(
                &mut self.inner,
                tx_ptr,
                rx_ptr,
                len,
                delay,
                bits,
                deselect as u8,
            )
        })
    }

    /// Manually asserts chip-select.
    pub fn select(&mut self) -> Result<(), SpiError> {
        check(unsafe { ffi::spi_select_chip(&mut self.inner) })
    }

    /// Manually deasserts chip-select.
    pub fn deselect(&mut self) -> Result<(), SpiError> {
        check(unsafe { ffi::spi_unselect_chip(&mut self.inner) })
    }
}

impl Drop for Spi {
    fn drop(&mut self) {
        unsafe {
            ffi::spi_close_device(&mut self.inner);
        }
    }
}
