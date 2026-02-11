use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::Error as CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;

use super::FrameReceiver;
use super::FrameSender;

#[derive(Debug, Clone)]
pub enum CfsLinkError {
    Cfs(CfsError),
    BufferTooSmall { required: usize, available: usize },
}

impl core::fmt::Display for CfsLinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cfs(e) => write!(f, "CFS error: {e}"),
            Self::BufferTooSmall { required, available } => {
                write!(f, "buffer too small: need {required}, have {available}")
            }
        }
    }
}

impl core::error::Error for CfsLinkError {}

impl From<CfsError> for CfsLinkError {
    fn from(e: CfsError) -> Self {
        Self::Cfs(e)
    }
}

pub struct UdpFrameSender<'a> {
    socket: &'a UdpSocket,
    target: SocketAddr,
}

impl<'a> UdpFrameSender<'a> {
    pub fn new(socket: &'a UdpSocket, target: SocketAddr) -> Self {
        Self { socket, target }
    }
}

impl FrameSender for UdpFrameSender<'_> {
    type Error = CfsLinkError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send(data, &self.target).await?;
        Ok(())
    }
}

pub struct UdpFrameReceiver<'a> {
    socket: &'a UdpSocket,
}

impl<'a> UdpFrameReceiver<'a> {
    pub fn new(socket: &'a UdpSocket) -> Self {
        Self { socket }
    }
}

impl FrameReceiver for UdpFrameReceiver<'_> {
    type Error = CfsLinkError;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _addr) = self.socket.recv(buffer).await?;
        Ok(len)
    }
}

pub struct PipeFrameSender {
    msg_id: MsgId,
}

impl PipeFrameSender {
    pub fn new(msg_id: MsgId) -> Self {
        Self { msg_id }
    }
}

impl FrameSender for PipeFrameSender {
    type Error = CfsLinkError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
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

pub struct PipeFrameReceiver<'a> {
    pipe: &'a mut Pipe,
    header_size: usize,
}

impl<'a> PipeFrameReceiver<'a> {
    pub fn new(pipe: &'a mut Pipe) -> Self {
        Self {
            pipe,
            header_size: 8,
        }
    }

    pub fn with_header_size(mut self, size: usize) -> Self {
        self.header_size = size;
        self
    }
}

impl FrameReceiver for PipeFrameReceiver<'_> {
    type Error = CfsLinkError;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
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
        buffer[..copy_len].copy_from_slice(&recv_buf[header_size..header_size + copy_len]);

        Ok(copy_len)
    }
}

pub struct UdpDataLink {
    socket: UdpSocket,
    remote: SocketAddr,
}

impl UdpDataLink {
    pub fn new(socket: UdpSocket, remote: SocketAddr) -> Self {
        Self { socket, remote }
    }

    pub fn bind(local: SocketAddr, remote: SocketAddr) -> Result<Self, CfsError> {
        let socket = UdpSocket::bind(local)?;
        Ok(Self { socket, remote })
    }
}

impl crate::datalink::DataLink for UdpDataLink {
    type Error = CfsError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send(data, &self.remote).await?;
        Ok(())
    }

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _addr) = self.socket.recv(buffer).await?;
        Ok(len)
    }
}
