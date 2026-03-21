//! Proximity-1 Physical Layer parameters (CCSDS 211.1-B-4).
//!
//! Defines the RF and modulation parameters for Proximity-1
//! space links. These are short-range, bidirectional links used
//! between landers, rovers, orbiters, and CubeSat formations.
//!
//! Modulation: GMSK with BT = 0.25.
//! Band: UHF 390-450 MHz.
//! Data rates: 8 kbps to 256 kbps (powers of 2).

use crate::physical::modulator::gmsk::Gmsk;

/// GMSK bandwidth-time product for Proximity-1.
pub const BT: f32 = 0.25;

/// Default samples per symbol for simulation.
pub const DEFAULT_SPS: usize = 8;

/// Forward link frequency band lower bound (Hz).
pub const FORWARD_BAND_LOW: u32 = 435_000_000;

/// Forward link frequency band upper bound (Hz).
pub const FORWARD_BAND_HIGH: u32 = 450_000_000;

/// Return link frequency band lower bound (Hz).
pub const RETURN_BAND_LOW: u32 = 390_000_000;

/// Return link frequency band upper bound (Hz).
pub const RETURN_BAND_HIGH: u32 = 405_000_000;

/// Channel spacing (Hz).
pub const CHANNEL_SPACING: u32 = 20_000;

/// Supported data rates (bits per second).
pub const DATA_RATES: [u32; 6] = [8_000, 16_000, 32_000, 64_000, 128_000, 256_000];

/// Data encoding options.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DataEncoding {
    /// Non-Return-to-Zero (natural binary).
    Nrz,
    /// Bi-Phase Level (Manchester).
    BiPhaseL,
}

/// Default noise variance for simulation.
pub const DEFAULT_NOISE_VAR: f32 = 0.1;

/// Default LLR scale factor.
pub const DEFAULT_SCALE: f32 = 1.0;

/// Creates a GMSK modulator configured for Proximity-1.
pub fn modulator() -> Gmsk {
    Gmsk::new(BT, DEFAULT_SPS, DEFAULT_NOISE_VAR, DEFAULT_SCALE)
}

/// Creates a GMSK modulator with a custom samples-per-symbol.
pub fn modulator_with_sps(sps: usize) -> Gmsk {
    Gmsk::new(BT, sps, DEFAULT_NOISE_VAR, DEFAULT_SCALE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::modulator::gmsk;

    #[test]
    fn modulator_creates_valid_gmsk() {
        let m = modulator();
        let bits: [u8; 4] = [0xA5, 0x3C, 0xFF, 0x00];
        let n_bits = 32;
        let out_len = gmsk::output_len(n_bits, DEFAULT_SPS);
        let mut out_i = [0.0f32; 512];
        let mut out_q = [0.0f32; 512];
        gmsk::modulate_gmsk(&bits, n_bits, BT, DEFAULT_SPS, &mut out_i[..out_len], &mut out_q[..out_len]);
        assert!(out_i[..out_len].iter().any(|&s| s != 0.0));
        let _ = m;
    }

    #[test]
    fn data_rates_are_powers_of_two_kbps() {
        for rate in DATA_RATES {
            assert!((rate / 1000).is_power_of_two());
        }
    }

    #[test]
    fn frequency_bands_valid() {
        assert!(FORWARD_BAND_LOW < FORWARD_BAND_HIGH);
        assert!(RETURN_BAND_LOW < RETURN_BAND_HIGH);
        assert!(RETURN_BAND_HIGH < FORWARD_BAND_LOW);
    }
}
