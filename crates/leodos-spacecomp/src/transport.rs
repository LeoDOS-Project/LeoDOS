//! Transport traits for SpaceComp role functions.

use leodos_protocols::network::isl::address::Address;

use leodos_protocols::transport::srspp::api::cfs::SrsppRxHandle;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
use leodos_protocols::transport::srspp::api::cfs::TransportError;
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

    async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, SpaceCompError>
    where
        F: FnOnce(&[u8]) -> Ret;
}

// ── Impls for SRSPP handles ─────────────────────────────────

impl<'a, E: Clone, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize>
    Tx for SrsppTxHandle<'a, E, S, R, WIN, BUF, MTU>
where
    SpaceCompError: From<TransportError<E>>,
{
    async fn send(&mut self, target: Address, data: &[u8]) -> Result<(), SpaceCompError> {
        SrsppTxHandle::send(self, target, data).await?;
        Ok(())
    }
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    Rx for SrsppRxHandle<'a, E, R, MAX_STREAMS>
where
    SpaceCompError: From<TransportError<E>>,
{
    async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), SpaceCompError> {
        Ok(SrsppRxHandle::recv(self, buf).await?)
    }

    async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, SpaceCompError>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        Ok(SrsppRxHandle::recv_with(self, f).await?)
    }
}

// ── Impls for mutable references (enables `impl Rx` by value) ──

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    Rx for &mut SrsppRxHandle<'a, E, R, MAX_STREAMS>
where
    SpaceCompError: From<TransportError<E>>,
{
    async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), SpaceCompError> {
        Ok(SrsppRxHandle::recv(*self, buf).await?)
    }

    async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, SpaceCompError>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        Ok(SrsppRxHandle::recv_with(*self, f).await?)
    }
}
