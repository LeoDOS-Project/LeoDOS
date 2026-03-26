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

pub use leodos_analysis::frame::Frame;

/// Thermal camera driver over SPI.
pub struct ThermalCamera {
    spi: Spi,
}

#[bon::bon]
impl ThermalCamera {
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
        let spi = Spi::open(
            device,
            bus,
            chip_select_line,
            baudrate,
            spi_mode,
            bits_per_word,
        )
        .map_err(BusError::from)?;
        Ok(Self { spi })
    }
}

impl From<Spi> for ThermalCamera {
    fn from(spi: Spi) -> Self {
        Self { spi }
    }
}

impl ThermalCamera {
    fn read_reg(&mut self, reg: u8) -> Result<u8, CfsError> {
        let tx = [reg, 0x00];
        let mut rx = [0u8; 2];
        self.spi
            .transfer(&tx, &mut rx, 2, 0, 8, true)
            .map_err(BusError::from)?;
        Ok(rx[0])
    }

    const WRITE_BIT: u8 = 0x80;

    fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), CfsError> {
        let tx = [reg | Self::WRITE_BIT, val];
        self.spi.write(&tx).map_err(BusError::from)?;
        Ok(())
    }

    /// Captures a thermal frame into caller-provided buffers.
    ///
    /// Yields until the camera is ready, then triggers a
    /// capture and reads the FIFO. Returns a [`Frame`] that
    /// borrows the MWIR/LWIR data. If only one band is
    /// available, LWIR is a copy of MWIR.
    pub async fn capture<'a>(
        &mut self,
        mwir: &'a mut [f32],
        lwir: &'a mut [f32],
    ) -> Result<Frame<'a>, CfsError> {
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
        let n = mwir.len().min(lwir.len());

        let mut bytes = [0u8; 4];

        let n_mwir = pixels_per_band.min(n).min(total_pixels);
        for i in 0..n_mwir {
            for b in &mut bytes {
                *b = self.read_reg(REG_FIFO_READ)?;
            }
            mwir[i] = f32::from_le_bytes(bytes);
        }

        if num_bands >= 2 {
            let remaining = total_pixels.saturating_sub(pixels_per_band);
            let n_lwir = pixels_per_band.min(n).min(remaining);
            for i in 0..n_lwir {
                for b in &mut bytes {
                    *b = self.read_reg(REG_FIFO_READ)?;
                }
                lwir[i] = f32::from_le_bytes(bytes);
            }
        } else {
            for i in 0..n_mwir {
                lwir[i] = mwir[i];
            }
        }

        Ok(Frame {
            mwir: &mwir[..n_mwir],
            lwir: &lwir[..n_mwir],
            width,
            height,
        })
    }
}
