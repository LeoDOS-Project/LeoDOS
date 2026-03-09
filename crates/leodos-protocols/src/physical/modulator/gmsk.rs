//! Gaussian Minimum Shift Keying (GMSK) Modulation
//!
//! GMSK is a continuous-phase modulation with Gaussian pulse
//! shaping and modulation index h = 0.5. The Gaussian filter
//! smooths the frequency transitions, yielding excellent spectral
//! containment and a nearly constant envelope.
//!
//! Used in Proximity-1 links (CCSDS 211.0, BT = 0.25) and other
//! near-Earth LEO communication systems.
//!
//! # Modulation
//!
//! The instantaneous frequency deviation is the NRZ data convolved
//! with a Gaussian pulse truncated to 4 symbol periods. The phase
//! is the running integral of the frequency deviation.
//!
//! # Demodulation
//!
//! Soft-decision differential detection: computes the cross-product
//! of consecutive symbol-spaced samples to approximate the phase
//! difference, then scales by 1/σ² for LLR output.

use super::clamp_i16;

/// Maximum supported samples-per-symbol.
const MAX_SPS: usize = 32;

/// Pulse truncation length in symbol periods.
const TRUNC: usize = 4;

/// Maximum pulse filter length.
const MAX_PULSE: usize = TRUNC * MAX_SPS + 1;

/// Returns the number of output samples for a GMSK frame.
///
/// Each input bit produces `sps` I/Q sample pairs, plus a tail
/// of `TRUNC / 2 * sps` samples for the pulse filter to ring
/// down.
pub fn output_len(n_bits: usize, sps: usize) -> usize {
    n_bits * sps + TRUNC / 2 * sps
}

/// Computes the Gaussian frequency pulse shape.
///
/// Returns `(pulse, len)` where `pulse[0..len]` contains the
/// sampled pulse, normalized so the total phase per bit is π/2
/// (for h = 0.5).
fn gaussian_pulse(bt: f32, sps: usize) -> ([f32; MAX_PULSE], usize) {
    let pulse_len = TRUNC * sps + 1;
    assert!(pulse_len <= MAX_PULSE);

    let center = (pulse_len / 2) as f32;
    let c = core::f32::consts::PI * bt
        / libm::sqrtf(2.0 * libm::logf(2.0));

    let mut pulse = [0f32; MAX_PULSE];
    let mut sum = 0f32;

    for m in 0..pulse_len {
        let t = (m as f32 - center) / sps as f32;
        let a1 = c * (2.0 * t - 1.0);
        let a2 = c * (2.0 * t + 1.0);
        pulse[m] = 0.25 * (libm::erff(a2) - libm::erff(a1));
        sum += pulse[m];
    }

    // Normalize so total phase per bit = π/2
    let norm = (sps as f32 * 0.5) / sum;
    for m in 0..pulse_len {
        pulse[m] *= norm;
    }

    (pulse, pulse_len)
}

/// Modulates packed bits using GMSK.
///
/// - `bt`: Gaussian bandwidth-time product (e.g. 0.25, 0.3, 0.5)
/// - `sps`: samples per symbol (must be ≤ 32)
///
/// Writes `output_len(n_bits, sps)` samples to `out_i` and `out_q`.
pub fn modulate_gmsk(
    bits: &[u8],
    n_bits: usize,
    bt: f32,
    sps: usize,
    out_i: &mut [f32],
    out_q: &mut [f32],
) {
    assert!(sps >= 1 && sps <= MAX_SPS);
    assert!(bits.len() * 8 >= n_bits || n_bits == 0);

    let (pulse, pulse_len) = gaussian_pulse(bt, sps);
    let n_out = output_len(n_bits, sps);
    assert!(out_i.len() >= n_out);
    assert!(out_q.len() >= n_out);

    let sps_f = sps as f32;
    let mut phase = 0f32;

    let half = (pulse_len / 2) as isize;

    for n in 0..n_out {
        // Compute filtered frequency deviation at sample n.
        // The pulse for bit k is centered at sample k*sps, so
        // tap = n - k*sps + pulse_len/2 maps the center of the
        // stored pulse to the bit transition.
        let mut freq = 0f32;

        // Only iterate over bits whose pulse overlaps sample n.
        let first_k = if (n as isize) >= half {
            ((n as isize - half) as usize) / sps
        } else {
            0
        };
        let last_k = (((n as isize + half) as usize) / sps + 1)
            .min(n_bits);

        for k in first_k..last_k {
            let tap = n as isize - (k * sps) as isize + half;
            if tap >= 0 && (tap as usize) < pulse_len {
                let bit =
                    (bits[k / 8] >> (7 - (k % 8))) & 1;
                let nrz = 1.0 - 2.0 * bit as f32;
                freq += nrz * pulse[tap as usize];
            }
        }

        phase += core::f32::consts::PI / sps_f * freq;
        out_i[n] = libm::cosf(phase);
        out_q[n] = libm::sinf(phase);
    }
}

/// Demodulates GMSK using soft-decision differential detection.
///
/// Computes the cross-product of samples spaced `sps` apart to
/// approximate `sin(Δφ)`, which is proportional to the transmitted
/// bit. The result is scaled by `scale / σ²` and quantized to i16.
///
/// Produces one LLR per bit. Bits at the beginning (before the
/// first full symbol delay) use the initial reference (1, 0).
pub fn demodulate_gmsk(
    in_i: &[f32],
    in_q: &[f32],
    n_bits: usize,
    sps: usize,
    noise_var: f32,
    scale: f32,
    llr: &mut [i16],
) {
    assert!(sps >= 1 && sps <= MAX_SPS);
    assert!(llr.len() >= n_bits);

    let n_out = output_len(n_bits, sps);
    assert!(in_i.len() >= n_out);
    assert!(in_q.len() >= n_out);

    let factor = scale / noise_var;
    let half_sps = sps / 2;

    for k in 0..n_bits {
        // Sample at the center of bit k
        let n = k * sps + half_sps;

        // Previous sample, one symbol period earlier
        let (prev_i, prev_q) = if n >= sps {
            (in_i[n - sps], in_q[n - sps])
        } else {
            (1.0, 0.0) // initial reference
        };

        // Cross-product ≈ sin(Δφ): I_prev · Q_curr - Q_prev · I_curr
        let cross = prev_i * in_q[n] - prev_q * in_i[n];
        llr[k] = clamp_i16(factor * cross);
    }
}

/// GMSK modulator/demodulator with configurable parameters.
pub struct Gmsk {
    bt: f32,
    sps: usize,
    noise_var: f32,
    scale: f32,
}

impl Gmsk {
    /// Creates a GMSK modem.
    ///
    /// - `bt`: Gaussian bandwidth-time product (e.g. 0.25)
    /// - `sps`: samples per symbol (≤ 32)
    /// - `noise_var`: noise variance σ²
    /// - `scale`: LLR quantization scale factor
    pub fn new(bt: f32, sps: usize, noise_var: f32, scale: f32) -> Self {
        Self { bt, sps, noise_var, scale }
    }
}

impl super::Modulator for Gmsk {
    fn modulate(
        &self,
        bits: &[u8],
        n_bits: usize,
        symbols: &mut [f32],
    ) -> usize {
        let n_out = output_len(n_bits, self.sps);
        let (oi, oq) = symbols.split_at_mut(n_out);
        modulate_gmsk(bits, n_bits, self.bt, self.sps, oi, oq);
        n_out * 2
    }
}

impl super::Demodulator for Gmsk {
    fn demodulate_soft(
        &self,
        symbols: &[f32],
        n_bits: usize,
        llr: &mut [i16],
    ) {
        let n_out = output_len(n_bits, self.sps);
        let (ii, iq) = symbols.split_at(n_out);
        demodulate_gmsk(
            ii, iq, n_bits, self.sps,
            self.noise_var, self.scale, llr,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_shape_normalized() {
        for &bt in &[0.25f32, 0.3, 0.5] {
            let sps = 8;
            let (pulse, len) = gaussian_pulse(bt, sps);
            let sum: f32 = pulse[..len].iter().sum();
            let expected = sps as f32 * 0.5;
            assert!(
                (sum - expected).abs() < 0.01,
                "bt={bt}: sum={sum}, expected={expected}"
            );
        }
    }

    #[test]
    fn pulse_shape_symmetric() {
        let (pulse, len) = gaussian_pulse(0.25, 8);
        for i in 0..len / 2 {
            let diff = (pulse[i] - pulse[len - 1 - i]).abs();
            assert!(diff < 1e-6, "asymmetry at {i}: {diff}");
        }
    }

    #[test]
    fn constant_envelope() {
        let bits = [0xA5u8, 0x5A];
        let n_bits = 16;
        let sps = 8;
        let n_out = output_len(n_bits, sps);
        let mut oi = [0f32; 256];
        let mut oq = [0f32; 256];
        modulate_gmsk(&bits, n_bits, 0.25, sps, &mut oi, &mut oq);

        for n in 0..n_out {
            let env = libm::sqrtf(oi[n] * oi[n] + oq[n] * oq[n]);
            assert!(
                (env - 1.0).abs() < 1e-5,
                "envelope at {n} = {env}"
            );
        }
    }

    #[test]
    fn roundtrip_no_noise() {
        let bits = [0b10110100u8];
        let n_bits = 8;
        let sps = 8;
        let n_out = output_len(n_bits, sps);
        let mut oi = [0f32; 256];
        let mut oq = [0f32; 256];
        modulate_gmsk(&bits, n_bits, 0.5, sps, &mut oi, &mut oq);

        let mut llr = [0i16; 8];
        demodulate_gmsk(&oi, &oq, n_bits, sps, 0.01, 100.0, &mut llr);

        let mut recovered = [0u8; 1];
        for i in 0..8 {
            if llr[i] < 0 {
                recovered[0] |= 1 << (7 - i);
            }
        }
        assert_eq!(recovered[0], bits[0]);
    }

    #[test]
    fn roundtrip_multiple_bytes() {
        let bits = [0xDE, 0xAD, 0xBE, 0xEF];
        let n_bits = 32;
        let sps = 8;
        let n_out = output_len(n_bits, sps);
        let mut oi = [0f32; 512];
        let mut oq = [0f32; 512];
        modulate_gmsk(&bits, n_bits, 0.3, sps, &mut oi, &mut oq);

        let mut llr = [0i16; 32];
        demodulate_gmsk(&oi, &oq, n_bits, sps, 0.01, 100.0, &mut llr);

        let mut recovered = [0u8; 4];
        for i in 0..32 {
            if llr[i] < 0 {
                recovered[i / 8] |= 1 << (7 - (i % 8));
            }
        }
        assert_eq!(recovered, bits);
    }

    #[test]
    fn all_zeros_positive_llr() {
        let bits = [0x00u8];
        let n_bits = 8;
        let sps = 8;
        let n_out = output_len(n_bits, sps);
        let mut oi = [0f32; 256];
        let mut oq = [0f32; 256];
        modulate_gmsk(&bits, n_bits, 0.5, sps, &mut oi, &mut oq);

        let mut llr = [0i16; 8];
        demodulate_gmsk(&oi, &oq, n_bits, sps, 0.01, 100.0, &mut llr);

        // All bits are 0, so LLRs should all be positive
        for (i, &l) in llr.iter().enumerate() {
            assert!(l > 0, "llr[{i}] = {l}, expected positive");
        }
    }

    #[test]
    fn output_len_calculation() {
        assert_eq!(output_len(10, 8), 10 * 8 + 2 * 8);
        assert_eq!(output_len(0, 8), 16);
    }

    #[test]
    fn different_bt_products() {
        // All BT values should produce valid roundtrips
        for &bt in &[0.25f32, 0.3, 0.5] {
            let bits = [0xC3u8];
            let n_bits = 8;
            let sps = 8;
            let n_out = output_len(n_bits, sps);
            let mut oi = [0f32; 256];
            let mut oq = [0f32; 256];
            modulate_gmsk(&bits, n_bits, bt, sps, &mut oi, &mut oq);

            let mut llr = [0i16; 8];
            demodulate_gmsk(
                &oi, &oq, n_bits, sps, 0.01, 100.0, &mut llr,
            );

            let mut recovered = [0u8; 1];
            for i in 0..8 {
                if llr[i] < 0 {
                    recovered[0] |= 1 << (7 - i);
                }
            }
            assert_eq!(
                recovered[0], bits[0],
                "roundtrip failed for BT={bt}"
            );
        }
    }
}
