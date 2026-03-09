use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use heapless::Deque;

use crate::datalink::{DataLinkReader, DataLinkWriter};
use crate::network::{NetworkReader, NetworkWriter};

/// Error from a local in-process channel.
#[derive(Debug, Clone)]
pub enum LocalLinkError {
    /// The internal queue has no remaining capacity.
    QueueFull,
    /// The channel has been closed.
    Closed,
}

impl core::fmt::Display for LocalLinkError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::QueueFull => write!(f, "queue full"),
            Self::Closed => write!(f, "channel closed"),
        }
    }
}

impl core::error::Error for LocalLinkError {}

struct Packet<const MTU: usize> {
    data: [u8; MTU],
    len: usize,
}

struct LocalChannelState<const QUEUE: usize, const MTU: usize> {
    to_router: Deque<Packet<MTU>, QUEUE>,
    from_router: Deque<Packet<MTU>, QUEUE>,
    closed: bool,
}

/// A single-threaded bidirectional channel between an app and a router.
pub struct LocalChannel<const QUEUE: usize, const MTU: usize> {
    state: RefCell<LocalChannelState<QUEUE, MTU>>,
}

impl<const QUEUE: usize, const MTU: usize> LocalChannel<QUEUE, MTU> {
    /// Creates a new local channel with empty queues.
    pub fn new() -> Self {
        Self {
            state: RefCell::new(LocalChannelState {
                to_router: Deque::new(),
                from_router: Deque::new(),
                closed: false,
            }),
        }
    }

    /// Splits the channel into application-side and router-side handles.
    pub fn split(&self) -> (LocalAppHandle<'_, QUEUE, MTU>, LocalRouterHandle<'_, QUEUE, MTU>) {
        (
            LocalAppHandle { channel: self },
            LocalRouterHandle { channel: self },
        )
    }

    /// Closes the channel, causing future operations to return `Closed`.
    pub fn close(&self) {
        self.state.borrow_mut().closed = true;
    }
}

impl<const QUEUE: usize, const MTU: usize> Default for LocalChannel<QUEUE, MTU> {
    fn default() -> Self {
        Self::new()
    }
}

/// Application-side handle for sending to and receiving from the router.
pub struct LocalAppHandle<'a, const QUEUE: usize, const MTU: usize> {
    channel: &'a LocalChannel<QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> NetworkWriter for LocalAppHandle<'a, QUEUE, MTU> {
    type Error = LocalLinkError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if state.closed {
                return Poll::Ready(Err(LocalLinkError::Closed));
            }

            if state.to_router.is_full() {
                return Poll::Pending;
            }

            let mut packet = Packet {
                data: [0u8; MTU],
                len: data.len().min(MTU),
            };
            packet.data[..packet.len].copy_from_slice(&data[..packet.len]);
            state.to_router.push_back(packet).ok();

            Poll::Ready(Ok(()))
        })
        .await
    }
}

impl<'a, const QUEUE: usize, const MTU: usize> NetworkReader for LocalAppHandle<'a, QUEUE, MTU> {
    type Error = LocalLinkError;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if let Some(packet) = state.from_router.pop_front() {
                let len = packet.len.min(buffer.len());
                buffer[..len].copy_from_slice(&packet.data[..len]);
                return Poll::Ready(Ok(len));
            }

            if state.closed {
                return Poll::Ready(Err(LocalLinkError::Closed));
            }

            Poll::Pending
        })
        .await
    }
}

/// Router-side handle for sending to and receiving from the application.
pub struct LocalRouterHandle<'a, const QUEUE: usize, const MTU: usize> {
    channel: &'a LocalChannel<QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> DataLinkWriter for LocalRouterHandle<'a, QUEUE, MTU> {
    type Error = LocalLinkError;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if state.closed {
                return Poll::Ready(Err(LocalLinkError::Closed));
            }

            if state.from_router.is_full() {
                return Poll::Pending;
            }

            let mut packet = Packet {
                data: [0u8; MTU],
                len: data.len().min(MTU),
            };
            packet.data[..packet.len].copy_from_slice(&data[..packet.len]);
            state.from_router.push_back(packet).ok();

            Poll::Ready(Ok(()))
        })
        .await
    }
}

impl<'a, const QUEUE: usize, const MTU: usize> DataLinkReader for LocalRouterHandle<'a, QUEUE, MTU> {
    type Error = LocalLinkError;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if let Some(packet) = state.to_router.pop_front() {
                let len = packet.len.min(buffer.len());
                buffer[..len].copy_from_slice(&packet.data[..len]);
                return Poll::Ready(Ok(len));
            }

            if state.closed {
                return Poll::Ready(Err(LocalLinkError::Closed));
            }

            Poll::Pending
        })
        .await
    }
}
