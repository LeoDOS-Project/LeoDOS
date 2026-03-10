//! Coding pipeline that composes randomizer, FEC, and framer
//! into a single `CodingWriter` / `CodingReader`.

use crate::coding::randomizer::Randomizer;
use crate::coding::{CodingReader, CodingWriter, Deframer, FecDecoder, FecEncoder, Framer};
use crate::physical::{PhysicalReader, PhysicalWriter};

// ── Write pipeline ──────────────────────────────────────────

/// Composes randomizer → FEC → framer → physical writer into a
/// single [`CodingWriter`].
pub struct CodingWritePipeline<R, F, M, W, const BUF: usize> {
    /// Randomizer applied to the transfer frame.
    pub randomizer: R,
    /// Forward error-correction encoder.
    pub fec: F,
    /// Framer (e.g. ASM, CLTU).
    pub framer: M,
    /// Physical layer writer.
    pub writer: W,
    buf_a: [u8; BUF],
    buf_b: [u8; BUF],
}

impl<R, F, M, W, const BUF: usize> CodingWritePipeline<R, F, M, W, BUF> {
    /// Creates a new write pipeline.
    pub fn new(randomizer: R, fec: F, framer: M, writer: W) -> Self {
        Self {
            randomizer,
            fec,
            framer,
            writer,
            buf_a: [0u8; BUF],
            buf_b: [0u8; BUF],
        }
    }
}

/// Error from a coding write pipeline.
#[derive(Debug)]
pub enum CodingWriteError<F, M, W> {
    /// FEC encoding failed.
    Fec(F),
    /// Framing failed.
    Framer(M),
    /// Physical writer failed.
    Writer(W),
    /// Internal buffer too small.
    BufferTooSmall,
}

impl<F: core::fmt::Display, M: core::fmt::Display, W: core::fmt::Display> core::fmt::Display
    for CodingWriteError<F, M, W>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Fec(e) => write!(f, "FEC encode: {e}"),
            Self::Framer(e) => write!(f, "framer: {e}"),
            Self::Writer(e) => write!(f, "writer: {e}"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
        }
    }
}

impl<F: core::error::Error, M: core::error::Error, W: core::error::Error> core::error::Error
    for CodingWriteError<F, M, W>
{
}

impl<R, F, M, W, const BUF: usize> CodingWriter for CodingWritePipeline<R, F, M, W, BUF>
where
    R: Randomizer,
    F: FecEncoder,
    F::Error: core::error::Error,
    M: Framer,
    M::Error: core::error::Error,
    W: PhysicalWriter,
    W::Error: core::error::Error,
{
    type Error = CodingWriteError<F::Error, M::Error, W::Error>;

    async fn write(&mut self, frame: &[u8]) -> Result<(), Self::Error> {
        if frame.len() > BUF {
            return Err(CodingWriteError::BufferTooSmall);
        }

        // 1. Copy frame → buf_a, randomize
        self.buf_a[..frame.len()].copy_from_slice(frame);
        self.randomizer.apply(&mut self.buf_a[..frame.len()]);

        // 2. FEC encode: buf_a → buf_b
        let fec_len = self
            .fec
            .encode(&self.buf_a[..frame.len()], &mut self.buf_b)
            .map_err(CodingWriteError::Fec)?;

        // 3. Frame: buf_b → buf_a
        let framed_len = self
            .framer
            .frame(&self.buf_b[..fec_len], &mut self.buf_a)
            .map_err(CodingWriteError::Framer)?;

        // 4. Write to physical layer
        self.writer
            .write(&self.buf_a[..framed_len])
            .await
            .map_err(CodingWriteError::Writer)
    }
}

// ── Read pipeline ───────────────────────────────────────────

/// Composes physical reader → deframer → FEC → derandomizer into
/// a single [`CodingReader`].
pub struct CodingReadPipeline<R, D, F, P, const BUF: usize> {
    /// Derandomizer (same as randomizer — XOR is self-inverse).
    pub randomizer: R,
    /// Deframer (e.g. ASM sync, CLTU decode).
    pub deframer: D,
    /// Forward error-correction decoder.
    pub fec: F,
    /// Physical layer reader.
    pub reader: P,
    buf_a: [u8; BUF],
    buf_b: [u8; BUF],
}

impl<R, D, F, P, const BUF: usize> CodingReadPipeline<R, D, F, P, BUF> {
    /// Creates a new read pipeline.
    pub fn new(randomizer: R, deframer: D, fec: F, reader: P) -> Self {
        Self {
            randomizer,
            deframer,
            fec,
            reader,
            buf_a: [0u8; BUF],
            buf_b: [0u8; BUF],
        }
    }
}

/// Error from a coding read pipeline.
#[derive(Debug)]
pub enum CodingReadError<D, F, P> {
    /// Deframing failed.
    Deframer(D),
    /// FEC decoding failed.
    Fec(F),
    /// Physical reader failed.
    Reader(P),
    /// Internal buffer too small.
    BufferTooSmall,
}

impl<D: core::fmt::Display, F: core::fmt::Display, P: core::fmt::Display> core::fmt::Display
    for CodingReadError<D, F, P>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Deframer(e) => write!(f, "deframer: {e}"),
            Self::Fec(e) => write!(f, "FEC decode: {e}"),
            Self::Reader(e) => write!(f, "reader: {e}"),
            Self::BufferTooSmall => write!(f, "buffer too small"),
        }
    }
}

impl<D: core::error::Error, F: core::error::Error, P: core::error::Error> core::error::Error
    for CodingReadError<D, F, P>
{
}

impl<R, D, F, P, const BUF: usize> CodingReader for CodingReadPipeline<R, D, F, P, BUF>
where
    R: Randomizer,
    D: Deframer,
    D::Error: core::error::Error,
    F: FecDecoder,
    F::Error: core::error::Error,
    P: PhysicalReader,
    P::Error: core::error::Error,
{
    type Error = CodingReadError<D::Error, F::Error, P::Error>;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        // 1. Read raw bytes from physical layer
        let raw_len = self
            .reader
            .read(&mut self.buf_a)
            .await
            .map_err(CodingReadError::Reader)?;

        if raw_len == 0 {
            return Ok(0);
        }

        // 2. Deframe: buf_a → buf_b
        let deframed_len = self
            .deframer
            .deframe(&self.buf_a[..raw_len], &mut self.buf_b)
            .map_err(CodingReadError::Deframer)?;

        // 3. FEC decode: copy buf_b → buf_a, decode in-place
        self.buf_a[..deframed_len].copy_from_slice(&self.buf_b[..deframed_len]);
        let data_len = self
            .fec
            .decode(&mut self.buf_a[..deframed_len])
            .map_err(CodingReadError::Fec)?;

        // 4. Derandomize in-place
        self.randomizer.apply(&mut self.buf_a[..data_len]);

        // 5. Copy to caller's buffer
        if buffer.len() < data_len {
            return Err(CodingReadError::BufferTooSmall);
        }
        buffer[..data_len].copy_from_slice(&self.buf_a[..data_len]);
        Ok(data_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coding::{NoFec, NoFramer, NoRandomizer};

    struct MemWriter {
        data: [u8; 1024],
        len: usize,
    }

    impl MemWriter {
        fn new() -> Self {
            Self {
                data: [0u8; 1024],
                len: 0,
            }
        }
    }

    #[derive(Debug)]
    struct MemError;
    impl core::fmt::Display for MemError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "mem error")
        }
    }
    impl core::error::Error for MemError {}

    impl PhysicalWriter for MemWriter {
        type Error = MemError;
        async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            self.data[..data.len()].copy_from_slice(data);
            self.len = data.len();
            Ok(())
        }
    }

    struct MemReader {
        data: [u8; 1024],
        len: usize,
    }

    impl MemReader {
        fn new(data: &[u8]) -> Self {
            let mut buf = [0u8; 1024];
            buf[..data.len()].copy_from_slice(data);
            Self {
                data: buf,
                len: data.len(),
            }
        }
    }

    impl PhysicalReader for MemReader {
        type Error = MemError;
        async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
            let len = self.len.min(buffer.len());
            buffer[..len].copy_from_slice(&self.data[..len]);
            Ok(len)
        }
    }

    #[test]
    fn no_op_pipeline_roundtrip() {
        futures::executor::block_on(async {
            let writer = MemWriter::new();
            let mut write_pipe: CodingWritePipeline<_, _, _, _, 1024> =
                CodingWritePipeline::new(NoRandomizer, NoFec, NoFramer, writer);

            let original = b"Hello, pipeline!";
            write_pipe.write(original).await.unwrap();

            let written = &write_pipe.writer.data[..write_pipe.writer.len];
            assert_eq!(written, original);

            // Read pipeline: D=NoFramer (deframer), F=NoFec (decoder)
            let reader = MemReader::new(written);
            let mut read_pipe: CodingReadPipeline<_, _, _, _, 1024> =
                CodingReadPipeline::new(NoRandomizer, NoFramer, NoFec, reader);

            let mut buf = [0u8; 256];
            let len = read_pipe.read(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], original);
        });
    }

    #[test]
    fn pipeline_with_randomizer() {
        use crate::coding::randomizer::Tm255Randomizer;

        futures::executor::block_on(async {
            let writer = MemWriter::new();
            let mut write_pipe: CodingWritePipeline<_, _, _, _, 1024> =
                CodingWritePipeline::new(Tm255Randomizer::new(), NoFec, NoFramer, writer);

            let original = b"Randomized data!";
            write_pipe.write(original).await.unwrap();

            let written = &write_pipe.writer.data[..write_pipe.writer.len];
            assert_ne!(written, original);

            let reader = MemReader::new(written);
            let mut read_pipe: CodingReadPipeline<_, _, _, _, 1024> =
                CodingReadPipeline::new(Tm255Randomizer::new(), NoFramer, NoFec, reader);

            let mut buf = [0u8; 256];
            let len = read_pipe.read(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], original);
        });
    }

    #[test]
    fn pipeline_with_asm_framing() {
        use crate::coding::framing::cadu::{AsmDeframer, AsmFramer};

        futures::executor::block_on(async {
            let writer = MemWriter::new();
            let mut write_pipe: CodingWritePipeline<_, _, _, _, 1024> =
                CodingWritePipeline::new(NoRandomizer, NoFec, AsmFramer::tm(), writer);

            let original = [0xAAu8; 32];
            write_pipe.write(&original).await.unwrap();

            let written_len = write_pipe.writer.len;
            assert_eq!(written_len, 36);

            let reader = MemReader::new(&write_pipe.writer.data[..written_len]);
            let mut read_pipe: CodingReadPipeline<_, _, _, _, 1024> =
                CodingReadPipeline::new(NoRandomizer, AsmDeframer::tm(32), NoFec, reader);

            let mut buf = [0u8; 256];
            let len = read_pipe.read(&mut buf).await.unwrap();
            assert_eq!(&buf[..len], &original);
        });
    }
}
