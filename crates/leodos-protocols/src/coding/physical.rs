use core::future::Future;

use super::cltu::{encode_cltu, encoded_cltu_len, CltuError};

/// Async trait for writing raw bytes to a physical channel.
pub trait AsyncPhysicalWriter {
    /// Error type for write operations.
    type Error;

    /// Writes the given data bytes to the physical channel.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Async trait for reading raw bytes from a physical channel.
pub trait AsyncPhysicalReader {
    /// Error type for read operations.
    type Error;

    /// Reads bytes into the buffer, returning the number of bytes read.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

/// Errors that can occur when writing CLTU-encoded frames.
#[derive(Debug, Clone)]
pub enum CltuWriterError<E> {
    /// A CLTU encoding error occurred.
    Cltu(CltuError),
    /// The underlying writer returned an error.
    Writer(E),
}

impl<E: core::fmt::Display> core::fmt::Display for CltuWriterError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cltu(e) => write!(f, "CLTU encoding error: {e:?}"),
            Self::Writer(e) => write!(f, "writer error: {e}"),
        }
    }
}

impl<E: core::error::Error> core::error::Error for CltuWriterError<E> {}

/// Wraps an `AsyncPhysicalWriter` to CLTU-encode TC frames before writing.
pub struct CltuWriter<W, const BUF: usize> {
    writer: W,
    buffer: [u8; BUF],
}

impl<W, const BUF: usize> CltuWriter<W, BUF> {
    /// Creates a new CLTU writer wrapping the given physical writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: [0u8; BUF],
        }
    }

    /// Consumes this wrapper, returning the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: AsyncPhysicalWriter, const BUF: usize> CltuWriter<W, BUF> {
    /// Encodes a TC frame as a CLTU and writes it to the physical channel.
    pub async fn write_frame(&mut self, tc_frame: &[u8]) -> Result<(), CltuWriterError<W::Error>> {
        let required = encoded_cltu_len(tc_frame.len());
        if required > BUF {
            return Err(CltuWriterError::Cltu(CltuError::OutputBufferTooSmall {
                required,
                provided: BUF,
            }));
        }

        let len = encode_cltu(tc_frame, &mut self.buffer).map_err(CltuWriterError::Cltu)?;
        self.writer
            .write(&self.buffer[..len])
            .await
            .map_err(CltuWriterError::Writer)
    }
}

impl<W: AsyncPhysicalWriter, const BUF: usize> AsyncPhysicalWriter for CltuWriter<W, BUF> {
    type Error = CltuWriterError<W::Error>;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.write_frame(data).await
    }
}
