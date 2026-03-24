//! Thermal camera driver over SPI.
//!
//! Reads dual-band (MWIR/LWIR) thermal imagery from the
//! NOS3 thermal camera simulator via SPI register access.
//! Each pixel is a brightness temperature in Kelvin as f32.

use crate::error::CfsError;
use crate::nos3::buses::spi::Spi;

const REG_STATUS: u8 = 0x01;
const REG_TRIGGER: u8 = 0x02;
const REG_NUM_BANDS: u8 = 0x0F;
const REG_WIDTH: u8 = 0x10;
const REG_HEIGHT: u8 = 0x11;
const REG_FIFO_SIZE_0: u8 = 0x12;
const REG_FIFO_SIZE_1: u8 = 0x13;
const REG_FIFO_SIZE_2: u8 = 0x14;
const REG_FIFO_READ: u8 = 0x20;

/// Thermal camera handle.
pub struct ThermalCamera {
    spi: Spi,
}

/// Capture result metadata.
pub struct CaptureInfo {
    pub width: u32,
    pub height: u32,
    pub num_bands: u8,
}

impl ThermalCamera {
    /// Opens the thermal camera on the given SPI bus.
    pub fn open(spi: Spi) -> Self {
        Self { spi }
    }

    fn read_reg(&mut self, reg: u8) -> u8 {
        let tx = [reg, 0x00];
        let mut rx = [0u8; 2];
        self.spi.transfer(&tx, &mut rx, 2, 0, 8, true).ok();
        rx[0]
    }

    fn write_reg(&mut self, reg: u8, val: u8) {
        let tx = [reg | 0x80, val];
        self.spi.write(&tx).ok();
    }

    /// Captures a thermal frame into the provided MWIR/LWIR buffers.
    ///
    /// Returns capture metadata (width, height, band count).
    /// If only one band is available, LWIR is filled with a copy of MWIR.
    pub fn capture(
        &mut self,
        mwir: &mut [f32],
        lwir: &mut [f32],
    ) -> Result<CaptureInfo, CfsError> {
        let status = self.read_reg(REG_STATUS);
        if status & 0x02 == 0 {
            return Err(CfsError::IncorrectState);
        }

        self.write_reg(REG_TRIGGER, 0x01);

        let num_bands = self.read_reg(REG_NUM_BANDS).max(1);
        let width = self.read_reg(REG_WIDTH) as u32;
        let height = self.read_reg(REG_HEIGHT) as u32;

        let s0 = self.read_reg(REG_FIFO_SIZE_0) as u32;
        let s1 = self.read_reg(REG_FIFO_SIZE_1) as u32;
        let s2 = self.read_reg(REG_FIFO_SIZE_2) as u32;
        let fifo_bytes = s0 | (s1 << 8) | (s2 << 16);
        let pixels_per_band = (width * height) as usize;
        let total_pixels = (fifo_bytes / 4) as usize;

        let mut bytes = [0u8; 4];

        let n_mwir = pixels_per_band.min(mwir.len()).min(total_pixels);
        for pixel in mwir.iter_mut().take(n_mwir) {
            for b in &mut bytes {
                *b = self.read_reg(REG_FIFO_READ);
            }
            *pixel = f32::from_le_bytes(bytes);
        }

        if num_bands >= 2 {
            let remaining = total_pixels.saturating_sub(pixels_per_band);
            let n_lwir = pixels_per_band.min(lwir.len()).min(remaining);
            for pixel in lwir.iter_mut().take(n_lwir) {
                for b in &mut bytes {
                    *b = self.read_reg(REG_FIFO_READ);
                }
                *pixel = f32::from_le_bytes(bytes);
            }
        } else {
            let n = n_mwir.min(lwir.len());
            lwir[..n].copy_from_slice(&mwir[..n]);
        }

        Ok(CaptureInfo {
            width,
            height,
            num_bands,
        })
    }
}
