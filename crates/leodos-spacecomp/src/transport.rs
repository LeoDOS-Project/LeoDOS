//! Transport traits for SpaceComp role functions.
#![allow(async_fn_in_trait)]

use leodos_libcfs::error::CfsError;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::transport::srspp::api::cfs::SrsppRxHandle;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
use leodos_protocols::transport::srspp::dtn::MessageStore;
use leodos_protocols::transport::srspp::dtn::Reachable;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverBackend;

use crate::SpaceCompError;

/// Sends messages to a target address.
pub trait Tx {
    async fn send(&mut self, target: Address, data: &[u8]) -> Result<(), SpaceCompError>;
}

/// Receives messages from remote sources.
pub trait Rx {
    async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), SpaceCompError>;

    async fn recv_with<T>(&mut self, f: impl FnOnce(&[u8]) -> T) -> Result<T, SpaceCompError>;
}

impl<'a, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize> Tx
    for SrsppTxHandle<'a, CfsError, S, R, WIN, BUF, MTU>
{
    async fn send(&mut self, target: Address, data: &[u8]) -> Result<(), SpaceCompError> {
        SrsppTxHandle::send(self, target, data).await?;
        Ok(())
    }
}

impl<'a, R: ReceiverBackend, const MAX_STREAMS: usize> Rx
    for SrsppRxHandle<'a, CfsError, R, MAX_STREAMS>
{
    async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), SpaceCompError> {
        Ok(SrsppRxHandle::recv(self, buf).await?)
    }

    async fn recv_with<T>(&mut self, f: impl FnOnce(&[u8]) -> T) -> Result<T, SpaceCompError> {
        Ok(SrsppRxHandle::recv_with(self, f).await?)
    }
}

impl<'a, R: ReceiverBackend, const MAX_STREAMS: usize> Rx
    for &mut SrsppRxHandle<'a, CfsError, R, MAX_STREAMS>
{
    async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), SpaceCompError> {
        Ok(SrsppRxHandle::recv(*self, buf).await?)
    }

    async fn recv_with<T>(&mut self, f: impl FnOnce(&[u8]) -> T) -> Result<T, SpaceCompError> {
        Ok(SrsppRxHandle::recv_with(*self, f).await?)
    }
}
