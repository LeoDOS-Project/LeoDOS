//! Space Data Link Protocols (SDLP).
//!
//! Spec: https://ccsds.org/Pubs/130x2g3.pdf

/// Advanced Orbiting Systems (AOS) Transfer Frame protocol.
pub mod aos;
/// CCSDS Proximity-1 protocol.
pub mod proximity1;
/// Telecommand (TC) Transfer Frame protocol.
pub mod tc;
/// Telemetry (TM) Transfer Frame protocol.
pub mod tm;

/// Re-export of the Proximity-1 Version-3 Transfer Frame.
pub use proximity1::Proximity1TransferFrame;
/// Re-export of the Telecommand Transfer Frame.
pub use tc::TelecommandTransferFrame;
/// Re-export of the Telemetry Transfer Frame.
pub use tm::TelemetryTransferFrame;
