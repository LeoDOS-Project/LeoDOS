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
const SPI_WRITE_BIT: u8 = 0x80;

const fn check_bits(value: u8, mask: u8) -> bool {
    value & mask != 0
}

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
    const SPI_PADDING: u8 = 0;

    fn read_reg(&mut self, reg: u8) -> Result<u8, CfsError> {
        let tx = [reg, Self::SPI_PADDING];
        let mut rx = [0u8; 2];
        self.spi
            .transfer(&tx, &mut rx, 2, 0, 8, true)
            .map_err(BusError::from)?;
        Ok(rx[0])
    }

    fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), CfsError> {
        let tx = [reg | SPI_WRITE_BIT, val];
        self.spi.write(&tx).map_err(BusError::from)?;
        Ok(())
    }

    fn is_ready(&mut self) -> Result<bool, CfsError> {
        Ok(check_bits(self.read_reg(REG_STATUS)?, STATUS_READY))
    }

    async fn wait_ready(&mut self) -> Result<(), CfsError> {
        core::future::poll_fn(|_| match self.is_ready() {
            Ok(true) => core::task::Poll::Ready(Ok(())),
            Ok(false) => core::task::Poll::Pending,
            Err(e) => core::task::Poll::Ready(Err(e)),
        })
        .await
    }

    /// Reads the FIFO size in total pixel count (24-bit register, 3 reads).
    fn fifo_pixel_count(&mut self) -> Result<usize, CfsError> {
        let b0 = self.read_reg(REG_FIFO_SIZE_0)?;
        let b1 = self.read_reg(REG_FIFO_SIZE_1)?;
        let b2 = self.read_reg(REG_FIFO_SIZE_2)?;
        let fifo_bytes = u32::from_le_bytes([b0, b1, b2, 0]);
        Ok((fifo_bytes / 4) as usize)
    }

    /// Reads one f32 pixel from the FIFO (4 byte reads, little-endian).
    fn read_pixel(&mut self) -> Result<f32, CfsError> {
        let mut bytes = [0u8; 4];
        for b in &mut bytes {
            *b = self.read_reg(REG_FIFO_READ)?;
        }
        Ok(f32::from_le_bytes(bytes))
    }

    /// Reads `count` pixels from the FIFO into `buf`.
    fn read_band(&mut self, buf: &mut [f32], count: usize) -> Result<(), CfsError> {
        for i in 0..count {
            buf[i] = self.read_pixel()?;
        }
        Ok(())
    }

    /// Captures a thermal frame into caller-provided buffers.
    ///
    /// Yields until the camera is ready, then triggers a
    /// capture and reads the FIFO. If only one band is
    /// available, LWIR is a copy of MWIR.
    pub async fn capture<'a>(
        &mut self,
        mwir: &'a mut [f32],
        lwir: &'a mut [f32],
    ) -> Result<Frame<'a>, CfsError> {
        self.wait_ready().await?;
        self.write_reg(REG_TRIGGER, TRIGGER_CAPTURE)?;

        let num_bands = self.read_reg(REG_NUM_BANDS)?.max(1);
        let width = self.read_reg(REG_WIDTH)? as u32;
        let height = self.read_reg(REG_HEIGHT)? as u32;
        let pixels_per_band = (width * height) as usize;
        let fifo_pixels = self.fifo_pixel_count()?;
        let buf_capacity = mwir.len().min(lwir.len());

        let n = pixels_per_band.min(buf_capacity).min(fifo_pixels);
        self.read_band(mwir, n)?;

        if num_bands >= 2 {
            let remaining = fifo_pixels.saturating_sub(pixels_per_band);
            let n_lwir = pixels_per_band.min(buf_capacity).min(remaining);
            self.read_band(lwir, n_lwir)?;
        } else {
            lwir[..n].copy_from_slice(&mwir[..n]);
        }

        Ok(Frame {
            mwir: &mwir[..n],
            lwir: &lwir[..n],
            width,
            height,
        })
    }
}
