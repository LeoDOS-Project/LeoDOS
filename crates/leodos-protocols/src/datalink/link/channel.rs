//! Generic frame link channel.
//!
//! Provides [`DatalinkWriter`] and [`DatalinkReader`] that compose a
//! frame format ([`FrameWrite`]/[`FrameRead`]) with a coding
//! pipeline ([`CodingWrite`]/[`CodingRead`]) into a single
//! owned type — no split, no shared state, no extra buffers.

use crate::coding::CodingRead;
use crate::coding::CodingWrite;
use crate::datalink::DatalinkRead;
use crate::datalink::DatalinkWrite;
use crate::datalink::framing::FrameRead;
use crate::datalink::framing::FrameWrite;
use crate::datalink::framing::PushError;
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
}

// ── DatalinkWriter ──

/// Owns a [`FrameWrite`] and a [`CodingWrite`], accumulating
/// packets into frames and flushing them to the coding pipeline.
pub struct DatalinkWriter<F, W> {
    frame_writer: F,
    coding_writer: W,
}

impl<F: FrameWrite, W: CodingWrite> DatalinkWriter<F, W> {
    /// Creates a new link writer.
    pub fn new(frame_writer: F, coding_writer: W) -> Self {
        Self {
            frame_writer,
            coding_writer,
        }
    }

    /// Finish the current frame and write it to the coding
    /// pipeline.
    pub async fn flush(&mut self) -> Result<(), DatalinkError<W::Error>> {
        if self.frame_writer.is_empty() {
            return Ok(());
        }

        let frame = self
            .frame_writer
            .finish()
            .map_err(|_| DatalinkError::BuildError)?;

        self.coding_writer
            .write(frame)
            .await
            .map_err(DatalinkError::Link)
    }
}

impl<F, W> DatalinkWrite for DatalinkWriter<F, W>
where
    F: FrameWrite,
    W: CodingWrite,
{
    type Error = DatalinkError<W::Error>;

    /// Push a packet into the current frame. If the frame is
    /// full, flushes it first, then retries.
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

/// Owns a [`FrameRead`] and a [`CodingRead`], reading
/// frames from the coding pipeline and extracting packets.
///
/// Packet extraction (previously in each FrameRead impl) is
/// handled here using [`SpacePacket::parse`] over the raw data
/// field returned by [`FrameRead::data_field`].
pub struct DatalinkReader<F, R> {
    frame_reader: F,
    coding_reader: R,
    /// Current read position within the data field.
    pos: usize,
}

impl<F: FrameRead, R: CodingRead> DatalinkReader<F, R> {
    /// Creates a new link reader.
    pub fn new(frame_reader: F, coding_reader: R) -> Self {
        Self {
            frame_reader,
            coding_reader,
            pos: 0,
        }
    }

    /// Try to extract the next packet from the current data
    /// field, returning its byte length or `None` if exhausted.
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

impl<F, R> DatalinkRead for DatalinkReader<F, R>
where
    F: FrameRead,
    R: CodingRead,
{
    type Error = DatalinkError<R::Error>;

    /// Reads the next packet. Fetches a new frame from the
    /// coding pipeline when the current frame is exhausted.
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        // Try extracting from current frame first.
        if let Some(n) = self.extract_packet(buffer) {
            return Ok(n);
        }

        // Read a new frame directly into frame_reader's buffer.
        let len = self
            .coding_reader
            .read(self.frame_reader.buffer_mut())
            .await
            .map_err(DatalinkError::Link)?;

        if len == 0 {
            return Ok(0);
        }

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
