use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;

use crate::datalink::{DatalinkReader, DatalinkWriter, link::cfs::CfsLinkError};

/// Sends frames over a CFS software bus pipe.
pub struct PipeFrameWriter {
    msg_id: MsgId,
}

impl PipeFrameWriter {
    /// Creates a new sender with the given message ID.
    pub fn new(msg_id: MsgId) -> Self {
        Self { msg_id }
    }
}

impl DatalinkWriter for PipeFrameWriter {
    type Error = CfsLinkError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let header_size = 8;
        let total_size = header_size + data.len();

        let mut buf = SendBuffer::new(total_size)?;

        {
            let mut msg = buf.view();
            msg.init(self.msg_id, total_size)?;
            let slice = buf.as_mut_slice();
            slice[header_size..].copy_from_slice(data);
        }

        buf.send(true)?;
        Ok(())
    }
}

/// Receives frames from a CFS software bus pipe.
pub struct PipeFrameReader<'a> {
    pipe: &'a mut Pipe,
    header_size: usize,
}

impl<'a> PipeFrameReader<'a> {
    /// Creates a new receiver on the given pipe.
    pub fn new(pipe: &'a mut Pipe) -> Self {
        Self {
            pipe,
            header_size: 8,
        }
    }

    /// Sets a custom header size to skip when receiving.
    pub fn with_header_size(mut self, size: usize) -> Self {
        self.header_size = size;
        self
    }
}

impl DatalinkReader for PipeFrameReader<'_> {
    type Error = CfsLinkError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let header_size = self.header_size;
        let total_size = header_size + buffer.len();

        let mut recv_buf = heapless::Vec::<u8, 2048>::new();
        recv_buf.resize(total_size, 0).ok();

        let len = self.pipe.recv(&mut recv_buf).await?;

        if len <= header_size {
            return Ok(0);
        }

        let payload_len = len - header_size;
        let copy_len = payload_len.min(buffer.len());
        buffer[..copy_len]
            .copy_from_slice(&recv_buf[header_size..header_size + copy_len]);

        Ok(copy_len)
    }
}
