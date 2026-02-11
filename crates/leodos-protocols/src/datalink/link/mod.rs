use core::future::Future;

pub mod asymmetric;
#[cfg(feature = "cfs")]
pub mod cfs;
pub mod tc;
pub mod tm;

pub trait FrameSender {
    type Error: core::error::Error;

    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

pub trait FrameReceiver {
    type Error: core::error::Error;

    fn recv(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
