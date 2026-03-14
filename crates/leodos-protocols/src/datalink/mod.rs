//! Implements the CCSDS Data Link Protocols (Layer 2).

use core::future::Future;

/// Transfer frame definitions (SDLP, USLP).
pub mod framing;
/// CCSDS Space Packet Protocol (SPP) definitions.
pub mod spp;
/// Async link channels for sending and receiving frames.
pub mod link;
/// Hop-by-hop reliable frame delivery (COP-1).
pub mod reliability;
/// Frame-level encryption and authentication (SDLS).
pub mod security;

// ── Layer boundary traits ──────────────────────────────────────

/// Send direction of the data link layer.
pub trait DatalinkWrite {
    /// Error type for write operations.
    type Error: core::error::Error;

    /// Write data over the link.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Receive direction of the data link layer.
pub trait DatalinkRead {
    /// Error type for read operations.
    type Error: core::error::Error;

    /// Read data from the link into `buffer`.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

/// A bidirectional data link that can be split into
/// independent read and write halves.
pub trait Datalink {
    /// Error type for read operations.
    type ReadError: core::error::Error;
    /// Error type for write operations.
    type WriteError: core::error::Error;
    /// Read half type.
    type Reader<'a>: DatalinkRead<Error = Self::ReadError>
    where
        Self: 'a;
    /// Write half type.
    type Writer<'a>: DatalinkWrite<Error = Self::WriteError>
    where
        Self: 'a;

    /// Split into independent read and write halves.
    fn split(&mut self) -> (Self::Reader<'_>, Self::Writer<'_>);
}

impl<R: DatalinkRead, W: DatalinkWrite> Datalink for (R, W) {
    type ReadError = R::Error;
    type WriteError = W::Error;
    type Reader<'a> = &'a mut R where Self: 'a;
    type Writer<'a> = &'a mut W where Self: 'a;

    fn split(&mut self) -> (&mut R, &mut W) {
        (&mut self.0, &mut self.1)
    }
}

impl<T: DatalinkRead + ?Sized> DatalinkRead for &mut T {
    type Error = T::Error;

    async fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, Self::Error> {
        T::read(self, buffer).await
    }
}

impl<T: DatalinkWrite + ?Sized> DatalinkWrite for &mut T {
    type Error = T::Error;

    async fn write(
        &mut self,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        T::write(self, data).await
    }
}
