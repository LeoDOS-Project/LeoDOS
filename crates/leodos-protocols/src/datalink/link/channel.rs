//! Generic frame link channel.
//!
//! Provides [`LinkWriter`] and [`LinkReader`] that compose a
//! frame format ([`FrameWriter`]/[`FrameReader`]) with a coding
//! pipeline ([`CodingWriter`]/[`CodingReader`]) into a single
//! owned type — no split, no shared state, no extra buffers.

use crate::coding::CodingReader;
use crate::coding::CodingWriter;
use crate::datalink::DatalinkReader;
use crate::datalink::DatalinkWriter;
use crate::datalink::framing::FrameReader;
use crate::datalink::framing::FrameWriter;
use crate::datalink::framing::PushError;

/// Errors that can occur during link channel operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LinkError<E> {
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

// ── LinkWriter ──

/// Owns a [`FrameWriter`] and a [`CodingWriter`], accumulating
/// packets into frames and flushing them to the coding pipeline.
pub struct LinkWriter<F, W> {
    frame_writer: F,
    coding_writer: W,
}

impl<F: FrameWriter, W: CodingWriter> LinkWriter<F, W> {
    /// Creates a new link writer.
    pub fn new(frame_writer: F, coding_writer: W) -> Self {
        Self {
            frame_writer,
            coding_writer,
        }
    }

    /// Push a packet into the current frame. If the frame is
    /// full, flushes it first, then retries.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), LinkError<W::Error>> {
        match self.frame_writer.push(data) {
            Ok(()) => Ok(()),
            Err(PushError::TooLarge) => Err(LinkError::FrameTooLarge),
            Err(PushError::Full) => {
                self.flush().await?;
                self.frame_writer
                    .push(data)
                    .map_err(|_| LinkError::FrameTooLarge)
            }
        }
    }

    /// Finish the current frame and write it to the coding
    /// pipeline.
    pub async fn flush(&mut self) -> Result<(), LinkError<W::Error>> {
        if self.frame_writer.is_empty() {
            return Ok(());
        }

        let frame = self
            .frame_writer
            .finish()
            .map_err(|_| LinkError::BuildError)?;

        self.coding_writer
            .write(frame)
            .await
            .map_err(LinkError::Link)
    }
}

impl<F, W> DatalinkWriter for LinkWriter<F, W>
where
    F: FrameWriter,
    W: CodingWriter,
{
    type Error = LinkError<W::Error>;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.send(data).await
    }
}

// ── LinkReader ──

/// Owns a [`FrameReader`] and a [`CodingReader`], reading
/// frames from the coding pipeline and extracting packets.
pub struct LinkReader<F, R> {
    frame_reader: F,
    coding_reader: R,
}

impl<F: FrameReader, R: CodingReader> LinkReader<F, R> {
    /// Creates a new link reader.
    pub fn new(frame_reader: F, coding_reader: R) -> Self {
        Self {
            frame_reader,
            coding_reader,
        }
    }

    /// Receive the next packet. Reads a new frame from the
    /// coding pipeline when the current frame is exhausted.
    pub async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, LinkError<R::Error>> {
        // Try extracting from current frame first
        if let Some(pkt) = self.frame_reader.next() {
            let len = pkt.len().min(buffer.len());
            buffer[..len].copy_from_slice(&pkt[..len]);
            return Ok(len);
        }

        // Read a new frame directly into frame_reader's buffer
        let len = self
            .coding_reader
            .read(self.frame_reader.buffer_mut())
            .await
            .map_err(LinkError::Link)?;

        if len == 0 {
            return Ok(0);
        }

        self.frame_reader
            .feed(len)
            .map_err(|_| LinkError::InvalidFrame)?;

        match self.frame_reader.next() {
            Some(pkt) => {
                let n = pkt.len().min(buffer.len());
                buffer[..n].copy_from_slice(&pkt[..n]);
                Ok(n)
            }
            None => Ok(0),
        }
    }
}

impl<F, R> DatalinkReader for LinkReader<F, R>
where
    F: FrameReader,
    R: CodingReader,
{
    type Error = LinkError<R::Error>;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.recv(buffer).await
    }
}
