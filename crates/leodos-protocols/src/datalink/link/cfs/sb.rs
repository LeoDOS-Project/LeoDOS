use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::CfsError as CfsError;

use crate::datalink::{DatalinkRead, DatalinkWrite};

const HEADER_SIZE: usize = 8;

/// A bidirectional data link over the cFS Software Bus.
///
/// Sends packets by publishing to `send_mid` and receives
/// packets by subscribing to `recv_mid` on a private pipe.
pub struct SbDatalink {
    pipe: Pipe,
    send_mid: MsgId,
}

impl SbDatalink {
    /// Creates a new Software Bus data link.
    ///
    /// Opens a pipe named `name` with queue depth `depth`,
    /// subscribes to `recv_mid`, and sends outbound data
    /// to `send_mid`.
    pub fn new(
        name: &str,
        depth: u16,
        recv_mid: MsgId,
        send_mid: MsgId,
    ) -> Result<Self, CfsError> {
        let pipe = Pipe::new(name, depth)?;
        pipe.subscribe(recv_mid)?;
        Ok(Self { pipe, send_mid })
    }
}

impl DatalinkWrite for SbDatalink {
    type Error = CfsError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let total_size = HEADER_SIZE + data.len();
        let mut buf = SendBuffer::new(total_size)?;
        {
            let mut msg = buf.view();
            msg.init(self.send_mid, total_size)?;
            let slice = buf.as_mut_slice();
            slice[HEADER_SIZE..].copy_from_slice(data);
        }
        buf.send(true)?;
        Ok(())
    }
}

impl DatalinkRead for SbDatalink {
    type Error = CfsError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let total_size = HEADER_SIZE + buffer.len();
        let mut recv_buf = heapless::Vec::<u8, 2048>::new();
        recv_buf.resize(total_size, 0).ok();
        let len = self.pipe.recv(&mut recv_buf).await?;
        if len <= HEADER_SIZE {
            return Ok(0);
        }
        let payload_len = len - HEADER_SIZE;
        let copy_len = payload_len.min(buffer.len());
        buffer[..copy_len].copy_from_slice(
            &recv_buf[HEADER_SIZE..HEADER_SIZE + copy_len],
        );
        Ok(copy_len)
    }
}
