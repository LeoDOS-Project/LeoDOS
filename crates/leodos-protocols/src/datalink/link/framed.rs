//! Generic frame link channel.
//!
//! Provides [`DatalinkWriter`] and [`DatalinkReader`] that compose a
//! frame format ([`FrameWrite`]/[`FrameRead`]) with optional security
//! ([`SecurityProcessor`]) and a coding pipeline
//! ([`CodingWrite`]/[`CodingRead`]) into a single owned type.

use bon::bon;

use crate::coding::CodingRead;
use crate::coding::CodingWrite;
use crate::datalink::DatalinkRead;
use crate::datalink::DatalinkWrite;
use crate::datalink::framing::FrameRead;
use crate::datalink::framing::FrameWrite;
use crate::datalink::framing::PushError;
use crate::datalink::security::SecurityProcessor;
use crate::datalink::spp::SpacePacket;

/// Errors that can occur during link channel operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DatalinkError<E> {
    /// The underlying coding layer returned an error.
    #[error("link error: {0}")]
    Link(E),
    /// The data exceeds the maximum frame data length.
    #[error("frame too large")]
    FrameTooLarge,
    /// A received frame failed to parse.
    #[error("invalid frame")]
    InvalidFrame,
    /// Failed to construct a transfer frame.
    #[error("frame build error")]
    BuildError,
    /// Security processing failed.
    #[error("security error")]
    Security,
}

// ── DatalinkWriter ──

/// Owns a [`FrameWrite`], [`SecurityProcessor`], and [`CodingWrite`],
/// accumulating packets into frames and flushing them through
/// security → coding.
pub struct DatalinkWriter<F, W, S> {
    frame_writer: F,
    security: S,
    coding_writer: W,
    scratch: [u8; 2048],
}

#[bon]
impl<F, W, S> DatalinkWriter<F, W, S>
where
    F: FrameWrite,
    S: SecurityProcessor,
    W: CodingWrite,
{
    /// Creates a new link writer.
    #[builder]
    pub fn new(frame_writer: F, coding_writer: W, security: S) -> Self {
        Self {
            frame_writer,
            security,
            coding_writer,
            scratch: [0u8; 2048],
        }
    }

    /// Finish the current frame and write it through
    /// security → coding.
    pub async fn flush(&mut self) -> Result<(), DatalinkError<W::Error>> {
        if self.frame_writer.is_empty() {
            return Ok(());
        }

        let frame = self
            .frame_writer
            .finish()
            .map_err(|_| DatalinkError::BuildError)?;

        let len = frame.len().min(2048);
        self.scratch[..len].copy_from_slice(&frame[..len]);

        let secured_len = self
            .security
            .apply(&mut self.scratch[..len])
            .map_err(|_| DatalinkError::Security)?;

        self.coding_writer
            .write(&self.scratch[..secured_len])
            .await
            .map_err(DatalinkError::Link)
    }
}

impl<F, W, S> DatalinkWrite for DatalinkWriter<F, W, S>
where
    F: FrameWrite,
    S: SecurityProcessor,
    W: CodingWrite,
{
    type Error = DatalinkError<W::Error>;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        match self.frame_writer.push(data) {
            Ok(()) => Ok(()),
            Err(PushError::TooLarge) => Err(DatalinkError::FrameTooLarge),
            Err(PushError::Full) => {
                self.flush().await?;
                self.frame_writer
                    .push(data)
                    .map_err(|_| DatalinkError::FrameTooLarge)
            }
        }
    }
}

// ── DatalinkReader ──

/// Owns a [`FrameRead`], [`SecurityProcessor`], and [`CodingRead`],
/// reading frames through coding → security removal → packet
/// extraction.
pub struct DatalinkReader<F, R, S> {
    frame_reader: F,
    security: S,
    coding_reader: R,
    pos: usize,
}

#[bon]
impl<F, R, S> DatalinkReader<F, R, S>
where
    F: FrameRead,
    S: SecurityProcessor,
    R: CodingRead,
{
    /// Creates a new link reader.
    #[builder]
    pub fn new(frame_reader: F, coding_reader: R, security: S) -> Self {
        Self {
            frame_reader,
            security,
            coding_reader,
            pos: 0,
        }
    }

    fn extract_packet(&mut self, buffer: &mut [u8]) -> Option<usize> {
        let data = self.frame_reader.data_field();
        if self.pos >= data.len() {
            return None;
        }
        let remaining = &data[self.pos..];
        let pkt = SpacePacket::parse(remaining).ok()?;
        let len = pkt.primary_header.packet_len();
        let n = len.min(buffer.len());
        buffer[..n].copy_from_slice(&remaining[..n]);
        self.pos += len;
        Some(n)
    }
}

impl<F, R, S> DatalinkRead for DatalinkReader<F, R, S>
where
    F: FrameRead,
    S: SecurityProcessor,
    R: CodingRead,
{
    type Error = DatalinkError<R::Error>;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        if let Some(n) = self.extract_packet(buffer) {
            return Ok(n);
        }

        let frame_buf = self.frame_reader.buffer_mut();
        let len = self
            .coding_reader
            .read(frame_buf)
            .await
            .map_err(DatalinkError::Link)?;

        if len == 0 {
            return Ok(0);
        }

        let _secured_len = self
            .security
            .process(&mut frame_buf[..len])
            .map_err(|_| DatalinkError::Security)?;

        self.frame_reader
            .feed(len)
            .map_err(|_| DatalinkError::InvalidFrame)?;
        self.pos = 0;

        match self.extract_packet(buffer) {
            Some(n) => Ok(n),
            None => Ok(0),
        }
    }
}
