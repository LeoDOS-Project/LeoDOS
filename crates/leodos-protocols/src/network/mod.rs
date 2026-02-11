use core::future::Future;

pub mod cfe;
pub mod isl;
pub mod passthrough;
pub mod spp;

pub trait NetworkLayer {
    type Error: core::error::Error;

    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;

    fn recv(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
