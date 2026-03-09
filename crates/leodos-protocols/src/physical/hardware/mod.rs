//! Hardware-backed physical channel implementations.

/// UART-based physical channel (NOS Engine / real hardware).
#[cfg(feature = "cfs")]
pub mod uart;
