use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use heapless::Deque;

use super::{FrameReceiver, FrameSender};
use crate::datalink::sdlp::tc::{BypassFlag, ControlFlag, TelecommandTransferFrame};

/// Configuration for a Telecommand link channel.
#[derive(Debug, Clone)]
pub struct TcConfig {
    /// Spacecraft ID for outgoing frames.
    pub scid: u16,
    /// Virtual Channel ID for outgoing frames.
    pub vcid: u8,
    /// Bypass flag indicating Type-A or Type-B acceptance checks.
    pub bypass: BypassFlag,
    /// Control flag indicating data or control command frames.
    pub control: ControlFlag,
    /// Maximum data field length in bytes.
    pub max_frame_data_len: usize,
}

/// Errors that can occur during TC link operations.
#[derive(Debug, Clone)]
pub enum TcError<E> {
    /// The underlying link returned an error.
    Link(E),
    /// The data exceeds the maximum frame data length.
    FrameTooLarge,
    /// A received frame failed to parse.
    InvalidFrame,
    /// The internal send queue is full.
    QueueFull,
    /// Failed to construct a TC Transfer Frame.
    BuildError,
}

impl<E: core::fmt::Display> core::fmt::Display for TcError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Link(e) => write!(f, "link error: {e}"),
            Self::FrameTooLarge => write!(f, "frame too large"),
            Self::InvalidFrame => write!(f, "invalid frame"),
            Self::QueueFull => write!(f, "send queue full"),
            Self::BuildError => write!(f, "frame build error"),
        }
    }
}

impl<E: core::error::Error> core::error::Error for TcError<E> {}

struct PendingPacket<const MTU: usize> {
    data: [u8; MTU],
    len: usize,
}

struct TcSenderState<E, const QUEUE: usize, const MTU: usize> {
    config: TcConfig,
    sequence: u8,
    pending: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TcError<E>>,
    closed: bool,
}

/// Shared state channel for the TC sender, split into handle and driver.
pub struct TcSenderChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TcSenderState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TcSenderChannel<E, QUEUE, MTU> {
    /// Creates a new TC sender channel with the given configuration.
    pub fn new(config: TcConfig) -> Self {
        Self {
            state: RefCell::new(TcSenderState {
                config,
                sequence: 0,
                pending: Deque::new(),
                error: None,
                closed: false,
            }),
        }
    }

    /// Splits the channel into a handle for sending and a driver for processing.
    pub fn split<W: FrameSender<Error = E>>(
        &self,
        writer: W,
    ) -> (
        TcSenderHandle<'_, E, QUEUE, MTU>,
        TcSenderDriver<'_, W, E, QUEUE, MTU>,
    ) {
        (
            TcSenderHandle { channel: self },
            TcSenderDriver {
                writer,
                channel: self,
                frame_buffer: [0u8; MTU],
            },
        )
    }
}

/// User-facing handle for enqueuing TC frames to send.
pub struct TcSenderHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TcSenderChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TcSenderHandle<'a, E, QUEUE, MTU> {
    /// Enqueues data to be sent as a TC frame, waiting if the queue is full.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), TcError<E>> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            if data.len() > state.config.max_frame_data_len {
                return Poll::Ready(Err(TcError::FrameTooLarge));
            }

            if data.len() > MTU {
                return Poll::Ready(Err(TcError::FrameTooLarge));
            }

            if state.pending.is_full() {
                return Poll::Pending;
            }

            let mut packet = PendingPacket {
                data: [0u8; MTU],
                len: data.len(),
            };
            packet.data[..data.len()].copy_from_slice(data);

            state.pending.push_back(packet).ok();
            Poll::Ready(Ok(()))
        })
        .await
    }

    /// Signals that no more data will be sent on this channel.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Returns true if the send queue is empty.
    pub fn is_empty(&self) -> bool {
        self.channel.state.borrow().pending.is_empty()
    }
}

impl<'a, E: Clone + core::error::Error, const QUEUE: usize, const MTU: usize> FrameSender
    for TcSenderHandle<'a, E, QUEUE, MTU>
{
    type Error = TcError<E>;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        TcSenderHandle::send(self, data).await
    }
}

/// Background driver that dequeues pending packets and writes TC frames.
pub struct TcSenderDriver<'a, W, E, const QUEUE: usize, const MTU: usize> {
    writer: W,
    channel: &'a TcSenderChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
}

impl<'a, W: FrameSender, E: Clone, const QUEUE: usize, const MTU: usize>
    TcSenderDriver<'a, W, E, QUEUE, MTU>
where
    W::Error: Into<E>,
{
    /// Runs the send loop, processing queued packets until the channel is closed.
    pub async fn run(&mut self) -> Result<(), TcError<E>> {
        loop {
            let packet = {
                let mut state = self.channel.state.borrow_mut();

                if state.closed && state.pending.is_empty() {
                    return Ok(());
                }

                state.pending.pop_front()
            };

            if let Some(packet) = packet {
                self.send_frame(&packet.data[..packet.len]).await?;
            } else {
                poll_fn(|_cx| {
                    let state = self.channel.state.borrow();
                    if state.closed || !state.pending.is_empty() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                })
                .await;
            }
        }
    }

    async fn send_frame(&mut self, data: &[u8]) -> Result<(), TcError<E>> {
        let (config, seq) = {
            let mut state = self.channel.state.borrow_mut();
            let seq = state.sequence;
            state.sequence = state.sequence.wrapping_add(1);
            (state.config.clone(), seq)
        };

        let frame = TelecommandTransferFrame::builder()
            .buffer(&mut self.frame_buffer)
            .scid(config.scid)
            .vcid(config.vcid)
            .bypass_flag(config.bypass)
            .control_flag(config.control)
            .seq(seq)
            .data_field_len(data.len())
            .build()
            .map_err(|_| TcError::BuildError)?;

        frame.data_field_mut().copy_from_slice(data);

        let frame_len = frame.frame_len();
        self.writer
            .send(&self.frame_buffer[..frame_len])
            .await
            .map_err(|e| TcError::Link(e.into()))?;

        Ok(())
    }
}

struct TcReceiverState<E, const QUEUE: usize, const MTU: usize> {
    received: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TcError<E>>,
    closed: bool,
}

/// Shared state channel for the TC receiver, split into handle and driver.
pub struct TcReceiverChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TcReceiverState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TcReceiverChannel<E, QUEUE, MTU> {
    /// Creates a new TC receiver channel.
    pub fn new() -> Self {
        Self {
            state: RefCell::new(TcReceiverState {
                received: Deque::new(),
                error: None,
                closed: false,
            }),
        }
    }

    /// Splits the channel into a handle for receiving and a driver for processing.
    pub fn split<R: FrameReceiver<Error = E>>(
        &self,
        reader: R,
    ) -> (
        TcReceiverHandle<'_, E, QUEUE, MTU>,
        TcReceiverDriver<'_, R, E, QUEUE, MTU>,
    ) {
        (
            TcReceiverHandle { channel: self },
            TcReceiverDriver {
                reader,
                channel: self,
                frame_buffer: [0u8; MTU],
            },
        )
    }
}

/// User-facing handle for receiving TC frame data.
pub struct TcReceiverHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TcReceiverChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TcReceiverHandle<'a, E, QUEUE, MTU> {
    /// Signals that no more data should be received on this channel.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Returns true if there is received data available.
    pub fn has_data(&self) -> bool {
        !self.channel.state.borrow().received.is_empty()
    }
}

impl<'a, E: Clone + core::error::Error, const QUEUE: usize, const MTU: usize> FrameReceiver
    for TcReceiverHandle<'a, E, QUEUE, MTU>
{
    type Error = TcError<E>;

    async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            if let Some(packet) = state.received.pop_front() {
                let len = packet.len.min(buffer.len());
                buffer[..len].copy_from_slice(&packet.data[..len]);
                return Poll::Ready(Ok(len));
            }

            if state.closed {
                return Poll::Ready(Ok(0));
            }

            Poll::Pending
        })
        .await
    }
}

/// Background driver that reads TC frames and enqueues parsed data.
pub struct TcReceiverDriver<'a, R, E, const QUEUE: usize, const MTU: usize> {
    reader: R,
    channel: &'a TcReceiverChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
}

impl<'a, R: FrameReceiver, E: Clone, const QUEUE: usize, const MTU: usize>
    TcReceiverDriver<'a, R, E, QUEUE, MTU>
where
    R::Error: Into<E>,
{
    /// Runs the receive loop, reading frames until the channel is closed.
    pub async fn run(&mut self) -> Result<(), TcError<E>> {
        loop {
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            let len = self
                .reader
                .recv(&mut self.frame_buffer)
                .await
                .map_err(|e| TcError::Link(e.into()))?;

            if len == 0 {
                continue;
            }

            let frame = TelecommandTransferFrame::parse(&self.frame_buffer[..len])
                .map_err(|_| TcError::InvalidFrame)?;

            let data_field = frame.data_field();

            let mut state = self.channel.state.borrow_mut();

            if state.received.is_full() {
                continue;
            }

            let mut packet = PendingPacket {
                data: [0u8; MTU],
                len: data_field.len(),
            };
            packet.data[..data_field.len()].copy_from_slice(data_field);
            state.received.push_back(packet).ok();
        }
    }
}
