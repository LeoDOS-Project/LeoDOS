//! Telemetry link channels.
//!
//! Type aliases over the generic [`channel`](super::channel)
//! module, pre-configured with TM framing.

pub use crate::datalink::framing::sdlp::tm::{
    TmFrameReader, TmFrameWriter, TmFrameWriterConfig,
};

pub use super::framed::{
    DatalinkError as TmError, DatalinkReader as TmLinkReader,
    DatalinkWriter as TmLinkWriter,
};

/// Configuration for a Telemetry link channel.
pub type TmConfig = TmFrameWriterConfig;
