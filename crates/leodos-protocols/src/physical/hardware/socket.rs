use leodos_libcfs::nos3::SocketError;
use leodos_libcfs::nos3::buses::socket::Socket;

use crate::physical::{PhysicalReader, PhysicalWriter};

/// A [`Socket`] bound to a fixed remote address for writing.
///
/// Stores the remote IP and port so that [`PhysicalWriter::write`]
/// can call [`Socket::send`] without per-call addressing.
/// [`PhysicalReader::read`] delegates to [`Socket::recv`] which
/// already needs no address.
pub struct BoundSocket {
    socket: Socket,
    remote_ip: &'static core::ffi::CStr,
    remote_port: i32,
}

impl BoundSocket {
    /// Wraps a socket with a fixed remote destination.
    pub fn new(
        socket: Socket,
        remote_ip: &'static core::ffi::CStr,
        remote_port: i32,
    ) -> Self {
        Self { socket, remote_ip, remote_port }
    }

    /// Returns a reference to the inner socket.
    pub fn socket(&self) -> &Socket {
        &self.socket
    }

    /// Returns a mutable reference to the inner socket.
    pub fn socket_mut(&mut self) -> &mut Socket {
        &mut self.socket
    }

    /// Consumes this wrapper, returning the inner socket.
    pub fn into_inner(self) -> Socket {
        self.socket
    }
}

impl PhysicalWriter for BoundSocket {
    type Error = SocketError;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let mut offset = 0;
        while offset < data.len() {
            let n = self.socket.send(
                &data[offset..],
                self.remote_ip,
                self.remote_port,
            )?;
            offset += n;
        }
        Ok(())
    }
}

impl PhysicalReader for BoundSocket {
    type Error = SocketError;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.socket.recv(buffer)
    }
}
