use core::future::Future;

use super::cltu::{encode_cltu, encoded_cltu_len, CltuError};

pub trait AsyncPhysicalWriter {
    type Error;

    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

pub trait AsyncPhysicalReader {
    type Error;

    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

#[derive(Debug, Clone)]
pub enum CltuWriterError<E> {
    Cltu(CltuError),
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

pub struct CltuWriter<W, const BUF: usize> {
    writer: W,
    buffer: [u8; BUF],
}

impl<W, const BUF: usize> CltuWriter<W, BUF> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: [0u8; BUF],
        }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: AsyncPhysicalWriter, const BUF: usize> CltuWriter<W, BUF> {
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
