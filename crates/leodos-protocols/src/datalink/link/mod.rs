/// Asymmetric link combining separate sender and receiver halves.
pub mod asymmetric;
/// Generic frame link channel, generic over FrameWrite/FrameRead.
pub mod channel;
/// CCSDS File Delivery Service link support.
#[cfg(feature = "cfs")]
pub mod cfs;
/// Telecommand link channels (re-exports from channel + TC framing).
pub mod tc;
/// Telemetry link channels (re-exports from channel + TM framing).
pub mod tm;
