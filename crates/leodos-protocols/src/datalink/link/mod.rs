/// Asymmetric link combining separate sender and receiver halves.
pub mod asymmetric;
/// CCSDS File Delivery Service link support.
#[cfg(feature = "cfs")]
pub mod cfs;
/// Telecommand link channels for sending and receiving TC frames.
pub mod tc;
/// Telemetry link channels for sending and receiving TM frames.
pub mod tm;
