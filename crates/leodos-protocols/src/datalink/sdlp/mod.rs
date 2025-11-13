//! Space Data Link Protocols (SDLP).
//!
//! Spec: https://ccsds.org/Pubs/130x2g3.pdf

pub mod aos;
pub mod proximity1;
pub mod tc;
pub mod tm;

pub use tc::TelecommandTransferFrame;
pub use tm::TelemetryTransferFrame;
