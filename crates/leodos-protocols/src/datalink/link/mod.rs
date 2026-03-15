/// Asymmetric link combining separate sender and receiver halves.
pub mod asymmetric;
/// CCSDS File Delivery Service link support.
#[cfg(feature = "cfs")]
pub mod cfs;
/// Frame + coding pipeline (DatalinkWriter/DatalinkReader).
pub mod framed;
/// In-process bidirectional channel for testing.
pub mod local;
/// Telecommand link channels (re-exports from framed + TC framing).
pub mod tc;
/// Telemetry link channels (re-exports from framed + TM framing).
pub mod tm;
