//! Thermal camera driver over SPI.
//!
//! Reads dual-band (MWIR/LWIR) thermal imagery from the
//! NOS3 thermal camera simulator via SPI register access.
//! Each pixel is a brightness temperature in Kelvin as f32.

use crate::error::CfsError;
use crate::nos3::buses::spi::Spi;
use crate::nos3::buses::spi::SpiError;
use crate::nos3::buses::BusError;

const REG_STATUS: u8 = 0x01;
const REG_TRIGGER: u8 = 0x02;
const REG_NUM_BANDS: u8 = 0x0F;
const REG_WIDTH: u8 = 0x10;
const REG_HEIGHT: u8 = 0x11;
const REG_FIFO_SIZE_0: u8 = 0x12;
const REG_FIFO_SIZE_1: u8 = 0x13;
const REG_FIFO_SIZE_2: u8 = 0x14;
const REG_FIFO_READ: u8 = 0x20;

const STATUS_READY: u8 = 0x02;
const TRIGGER_CAPTURE: u8 = 0x01;

/// Thermal camera error.
#[derive(Debug)]
pub enum ThermalCamError {
    /// Camera not ready (status register bit not set).
    NotReady,
    /// SPI bus error.
    Spi(SpiError),
}

impl From<SpiError> for ThermalCamError {
    fn from(e: SpiError) -> Self {
        Self::Spi(e)
    }
}

impl core::fmt::Display for ThermalCamError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotReady => write!(f, "thermal camera not ready"),
            Self::Spi(e) => write!(f, "SPI: {e}"),
        }
    }
}

impl core::error::Error for ThermalCamError {}

/// Capture result borrowing the camera's internal buffers.
pub struct Frame<'a> {
    /// MWIR brightness temperatures (Kelvin).
    pub mwir: &'a [f32],
    /// LWIR brightness temperatures (Kelvin).
    pub lwir: &'a [f32],
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// Thermal camera with owned pixel buffers.
///
/// `N` is the maximum number of pixels per band
/// (width * height). Typical values: `64*64` for
/// low-res, `512*512` for high-res.
pub struct ThermalCamera<const N: usize> {
    spi: Spi,
    mwir: [f32; N],
    lwir: [f32; N],
}

#[bon::bon]
impl<const N: usize> ThermalCamera<N> {
    /// Creates a new thermal camera on the given SPI bus.
    ///
    /// The builder can open the SPI device directly:
    /// ```ignore
    /// Camera::builder()
    ///     .device(c"spi_3")
    ///     .bus(0)
    ///     .cs(3)
    ///     .baudrate(1_000_000)
    ///     .build()?;
    /// ```
    #[builder]
    pub fn new(
        device: &core::ffi::CStr,
        #[builder(default)] bus: u8,
        chip_select_line: u8,
        baudrate: u32,
        #[builder(default)] spi_mode: u8,
        #[builder(default = 8)] bits_per_word: u8,
    ) -> Result<Self, CfsError> {
        let spi = Spi::open(device, bus, chip_select_line, baudrate, spi_mode, bits_per_word)
            .map_err(BusError::from)?;
        Ok(Self {
            spi,
            mwir: [0.0; N],
            lwir: [0.0; N],
        })
    }
}

impl<const N: usize> From<Spi> for ThermalCamera<N> {
    fn from(spi: Spi) -> Self {
        Self {
            spi,
            mwir: [0.0; N],
            lwir: [0.0; N],
        }
    }
}

impl<const N: usize> ThermalCamera<N> {
    fn read_reg(&mut self, reg: u8) -> Result<u8, CfsError> {
        let tx = [reg, 0x00];
        let mut rx = [0u8; 2];
        self.spi.transfer(&tx, &mut rx, 2, 0, 8, true).map_err(BusError::from)?;
        Ok(rx[0])
    }

    const WRITE_BIT: u8 = 0x80;

    fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), CfsError> {
        let tx = [reg | Self::WRITE_BIT, val];
        self.spi.write(&tx).map_err(BusError::from)?;
        Ok(())
    }

    /// Captures a thermal frame into the internal buffers.
    ///
    /// Yields until the camera is ready, then triggers a
    /// capture and reads the FIFO. Returns a [`Frame`] that
    /// borrows the MWIR/LWIR data. If only one band is
    /// available, LWIR is a copy of MWIR.
    pub async fn capture(&mut self) -> Result<Frame<'_>, CfsError> {
        core::future::poll_fn(|_| match self.read_reg(REG_STATUS) {
            Ok(s) if s & STATUS_READY != 0 => core::task::Poll::Ready(Ok(())),
            Ok(_) => core::task::Poll::Pending,
            Err(e) => core::task::Poll::Ready(Err(e)),
        })
        .await?;

        self.write_reg(REG_TRIGGER, TRIGGER_CAPTURE)?;

        let num_bands = self.read_reg(REG_NUM_BANDS)?.max(1);
        let width = self.read_reg(REG_WIDTH)? as u32;
        let height = self.read_reg(REG_HEIGHT)? as u32;

        let s0 = self.read_reg(REG_FIFO_SIZE_0)? as u32;
        let s1 = self.read_reg(REG_FIFO_SIZE_1)? as u32;
        let s2 = self.read_reg(REG_FIFO_SIZE_2)? as u32;
        let fifo_bytes = s0 | (s1 << 8) | (s2 << 16);
        let pixels_per_band = (width * height) as usize;
        let total_pixels = (fifo_bytes / 4) as usize;

        let mut bytes = [0u8; 4];

        let n_mwir = pixels_per_band.min(N).min(total_pixels);
        for i in 0..n_mwir {
            for b in &mut bytes {
                *b = self.read_reg(REG_FIFO_READ)?;
            }
            self.mwir[i] = f32::from_le_bytes(bytes);
        }

        if num_bands >= 2 {
            let remaining = total_pixels.saturating_sub(pixels_per_band);
            let n_lwir = pixels_per_band.min(N).min(remaining);
            for i in 0..n_lwir {
                for b in &mut bytes {
                    *b = self.read_reg(REG_FIFO_READ)?;
                }
                self.lwir[i] = f32::from_le_bytes(bytes);
            }
        } else {
            for i in 0..n_mwir {
                self.lwir[i] = self.mwir[i];
            }
        }

        Ok(Frame {
            mwir: &self.mwir[..n_mwir],
            lwir: &self.lwir[..n_mwir],
            width,
            height,
        })
    }
}
