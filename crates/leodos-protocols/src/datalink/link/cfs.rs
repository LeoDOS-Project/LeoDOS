use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::Error as CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;

use crate::datalink::{DatalinkReader, DatalinkWriter};

/// Errors from CFS data link operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CfsLinkError {
    /// An error from the CFS runtime.
    #[error("CFS error: {0}")]
    Cfs(#[from] CfsError),
    /// The provided buffer is too small.
    #[error("buffer too small: need {required}, have {available}")]
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual buffer size available.
        available: usize,
    },
}

/// Sends frames over UDP.
pub struct UdpFrameSender<'a> {
    socket: &'a UdpSocket,
    target: SocketAddr,
}

impl<'a> UdpFrameSender<'a> {
    /// Creates a new sender targeting the given address.
    pub fn new(socket: &'a UdpSocket, target: SocketAddr) -> Self {
        Self { socket, target }
    }
}

impl DatalinkWriter for UdpFrameSender<'_> {
    type Error = CfsLinkError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send(data, &self.target).await?;
        Ok(())
    }
}

/// Receives frames over UDP.
pub struct UdpFrameReceiver<'a> {
    socket: &'a UdpSocket,
}

impl<'a> UdpFrameReceiver<'a> {
    /// Creates a new receiver on the given socket.
    pub fn new(socket: &'a UdpSocket) -> Self {
        Self { socket }
    }
}

impl DatalinkReader for UdpFrameReceiver<'_> {
    type Error = CfsLinkError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _addr) = self.socket.recv(buffer).await?;
        Ok(len)
    }
}

/// Sends frames over a CFS software bus pipe.
pub struct PipeFrameSender {
    msg_id: MsgId,
}

impl PipeFrameSender {
    /// Creates a new sender with the given message ID.
    pub fn new(msg_id: MsgId) -> Self {
        Self { msg_id }
    }
}

impl DatalinkWriter for PipeFrameSender {
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
pub struct PipeFrameReceiver<'a> {
    pipe: &'a mut Pipe,
    header_size: usize,
}

impl<'a> PipeFrameReceiver<'a> {
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

impl DatalinkReader for PipeFrameReceiver<'_> {
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

/// A bidirectional UDP data link.
pub struct UdpDataLink {
    socket: UdpSocket,
    remote: SocketAddr,
}

impl UdpDataLink {
    /// Creates a new data link from an existing socket and remote address.
    pub fn new(socket: UdpSocket, remote: SocketAddr) -> Self {
        Self { socket, remote }
    }

    /// Binds a local socket and creates a data link to the remote address.
    pub fn bind(
        local: SocketAddr,
        remote: SocketAddr,
    ) -> Result<Self, CfsError> {
        let socket = UdpSocket::bind(local)?;
        Ok(Self { socket, remote })
    }
}

impl DatalinkWriter for UdpDataLink {
    type Error = CfsError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send(data, &self.remote).await?;
        Ok(())
    }
}

impl DatalinkReader for UdpDataLink {
    type Error = CfsError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _addr) = self.socket.recv(buffer).await?;
        Ok(len)
    }
}
