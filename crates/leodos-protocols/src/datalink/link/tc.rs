//! Telecommand link channels.
//!
//! Type aliases over the generic [`channel`](super::channel)
//! module, pre-configured with TC framing.

pub use crate::datalink::framing::sdlp::tc::{
    BypassFlag, ControlFlag, TcFrameReader, TcFrameWriter, TcFrameWriterConfig,
};

pub use super::framed::{
    DatalinkError as TcError, DatalinkReader as TcLinkReader, DatalinkWriter as TcLinkWriter,
};

/// Configuration for a Telecommand link channel.
pub type TcConfig = TcFrameWriterConfig;
