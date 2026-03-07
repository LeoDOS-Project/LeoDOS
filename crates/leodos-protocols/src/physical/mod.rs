//! Physical layer: modulation, demodulation, and channel models.
//!
//! Connects the coding layer (LDPC, RS) to a simulated or real RF
//! link. The modulator maps coded bits to baseband symbols, the
//! demodulator converts noisy received symbols to soft-decision
//! LLRs that feed into the LDPC decoder.

/// BPSK and QPSK modulation/demodulation.
pub mod modulation;
/// Offset QPSK modulation/demodulation (Proximity-1).
pub mod oqpsk;
/// Gray-coded 8PSK modulation/demodulation.
pub mod eight_psk;
/// Gaussian Minimum Shift Keying (GMSK) modulation.
pub mod gmsk;
