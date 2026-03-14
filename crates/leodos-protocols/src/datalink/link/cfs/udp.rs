use leodos_libcfs::error::Error as CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;

use crate::datalink::{Datalink, DatalinkRead, DatalinkWrite};
/// Sends frames over UDP.
pub struct UdpFrameWriter<'a> {
    socket: &'a UdpSocket,
    target: &'a SocketAddr,
}

impl<'a> UdpFrameWriter<'a> {
    /// Creates a new sender targeting the given address.
    pub fn new(socket: &'a UdpSocket, target: &'a SocketAddr) -> Self {
        Self { socket, target }
    }
}

impl DatalinkWrite for UdpFrameWriter<'_> {
    type Error = CfsError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send(data, self.target).await?;
        Ok(())
    }
}

/// Receives frames over UDP.
pub struct UdpFrameReader<'a> {
    socket: &'a UdpSocket,
}

impl<'a> UdpFrameReader<'a> {
    /// Creates a new receiver on the given socket.
    pub fn new(socket: &'a UdpSocket) -> Self {
        Self { socket }
    }
}

impl DatalinkRead for UdpFrameReader<'_> {
    type Error = CfsError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _addr) = self.socket.recv(buffer).await?;
        Ok(len)
    }
}

/// A bidirectional UDP data link.
pub struct UdpDatalink {
    socket: UdpSocket,
    remote: SocketAddr,
}

impl UdpDatalink {
    /// Creates a new data link from an existing socket and remote address.
    pub fn new(socket: UdpSocket, remote: SocketAddr) -> Self {
        Self { socket, remote }
    }

    /// Binds a local socket and creates a data link to the remote address.
    pub fn bind(local: SocketAddr, remote: SocketAddr) -> Result<Self, CfsError> {
        let socket = UdpSocket::bind(local)?;
        Ok(Self { socket, remote })
    }

    /// Splits into independent read and write halves.
    ///
    /// Both halves borrow the underlying socket. `send()` and
    /// `recv()` on `UdpSocket` take `&self`, so concurrent
    /// use is safe.
    pub fn split(&self) -> (UdpFrameReader<'_>, UdpFrameWriter<'_>) {
        (
            UdpFrameReader::new(&self.socket),
            UdpFrameWriter::new(&self.socket, &self.remote),
        )
    }
}


impl Datalink for UdpDatalink {
    type ReadError = CfsError;
    type WriteError = CfsError;
    type Reader<'a> = UdpFrameReader<'a>;
    type Writer<'a> = UdpFrameWriter<'a>;

    fn split(&mut self) -> (UdpFrameReader<'_>, UdpFrameWriter<'_>) {
        (
            UdpFrameReader::new(&self.socket),
            UdpFrameWriter::new(&self.socket, &self.remote),
        )
    }
}

impl DatalinkWrite for UdpDatalink {
    type Error = CfsError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.socket.send(data, &self.remote).await?;
        Ok(())
    }
}

impl DatalinkRead for UdpDatalink {
    type Error = CfsError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let (len, _addr) = self.socket.recv(buffer).await?;
        Ok(len)
    }
}
