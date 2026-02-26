use core::fmt;
use core::str;
use leodos_protocols::application::colonies::executor::ColoniesHandler;
use leodos_protocols::application::colonies::messages::ArgIterator;

/// The logic handler.
pub struct Handler {
    pub counter: u32,
}

impl Handler {
    pub fn new() -> Self {
        Self { counter: 0 }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Missing argument")]
    MissingArgument,
    #[error("Unknown function")]
    UnknownFunction,
    #[error("Output buffer full")]
    OutputBufferFull,
}

impl ColoniesHandler for Handler {
    type Error = Error;

    async fn handle(
        &mut self,
        func_name: &str,
        mut args: ArgIterator<'_>,
        output_buffer: &mut [u8],
    ) -> Result<usize, Self::Error> {
        match func_name {
            "echo" => {
                let Some(arg_bytes) = args.next() else {
                    return Err(Error::MissingArgument);
                };

                if output_buffer.len() < arg_bytes.len() {
                    return Err(Error::OutputBufferFull);
                }

                output_buffer[..arg_bytes.len()].copy_from_slice(arg_bytes);
                Ok(arg_bytes.len())
            }

            "count" => {
                self.counter += 1;

                let mut writer = BufWriter::new(output_buffer);
                use core::fmt::Write;

                write!(writer, "Count: {}", self.counter).map_err(|_| Error::OutputBufferFull)?;

                Ok(writer.len())
            }

            _ => Err(Error::UnknownFunction),
        }
    }
}

struct BufWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> BufWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn len(&self) -> usize {
        self.pos
    }
}

impl<'a> fmt::Write for BufWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let rem = self.buf.len() - self.pos;
        if rem < bytes.len() {
            return Err(fmt::Error);
        }
        self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}
