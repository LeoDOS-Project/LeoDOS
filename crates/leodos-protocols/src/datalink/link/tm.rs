use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use heapless::Deque;

use super::{FrameReceiver, FrameSender};
use crate::coding::randomizer::{Randomizer, Tm255Randomizer};
use crate::datalink::sdlp::tm::TelemetryTransferFrame;

/// Configuration for a Telemetry link channel.
#[derive(Debug, Clone)]
pub struct TmConfig {
    /// Spacecraft ID for TM frames.
    pub scid: u16,
    /// Virtual Channel ID for TM frames.
    pub vcid: u8,
    /// Maximum data field length in bytes.
    pub max_frame_data_len: usize,
    /// Whether to apply CCSDS pseudo-randomization.
    pub randomize: bool,
}

/// Errors that can occur during TM link operations.
#[derive(Debug, Clone)]
pub enum TmError<E> {
    /// The underlying link returned an error.
    Link(E),
    /// The data exceeds the maximum frame data length.
    FrameTooLarge,
    /// A received frame failed to parse.
    InvalidFrame,
    /// The internal receive queue is full.
    QueueFull,
    /// Failed to construct a TM Transfer Frame.
    BuildError,
}

impl<E: core::fmt::Display> core::fmt::Display for TmError<E> {
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

impl<E: core::error::Error> core::error::Error for TmError<E> {}

struct PendingPacket<const MTU: usize> {
    data: [u8; MTU],
    len: usize,
}

struct TmSenderState<E, const QUEUE: usize, const MTU: usize> {
    config: TmConfig,
    mc_frame_count: u8,
    vc_frame_count: u8,
    pending: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TmError<E>>,
    closed: bool,
}

/// Shared state channel for the TM sender, split into handle and driver.
pub struct TmSenderChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TmSenderState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TmSenderChannel<E, QUEUE, MTU> {
    /// Creates a new TM sender channel with the given configuration.
    pub fn new(config: TmConfig) -> Self {
        Self {
            state: RefCell::new(TmSenderState {
                config,
                mc_frame_count: 0,
                vc_frame_count: 0,
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
        TmSenderHandle<'_, E, QUEUE, MTU>,
        TmSenderDriver<'_, W, E, QUEUE, MTU>,
    ) {
        (
            TmSenderHandle { channel: self },
            TmSenderDriver {
                writer,
                channel: self,
                frame_buffer: [0u8; MTU],
                randomizer: Tm255Randomizer::new(),
            },
        )
    }
}

/// User-facing handle for enqueuing TM frames to send.
pub struct TmSenderHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TmSenderChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TmSenderHandle<'a, E, QUEUE, MTU> {
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
    for TmSenderHandle<'a, E, QUEUE, MTU>
{
    type Error = TmError<E>;

    async fn send(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            if data.len() > state.config.max_frame_data_len {
                return Poll::Ready(Err(TmError::FrameTooLarge));
            }

            if data.len() > MTU {
                return Poll::Ready(Err(TmError::FrameTooLarge));
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
}

/// Background driver that dequeues pending packets and writes TM frames.
pub struct TmSenderDriver<'a, W, E, const QUEUE: usize, const MTU: usize> {
    writer: W,
    channel: &'a TmSenderChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
    randomizer: Tm255Randomizer,
}

impl<'a, W: FrameSender, E: Clone, const QUEUE: usize, const MTU: usize>
    TmSenderDriver<'a, W, E, QUEUE, MTU>
where
    W::Error: Into<E>,
{
    /// Runs the send loop, processing queued packets until the channel is closed.
    pub async fn run(&mut self) -> Result<(), TmError<E>> {
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

    async fn send_frame(&mut self, data: &[u8]) -> Result<(), TmError<E>> {
        let (config, mc_count, vc_count) = {
            let mut state = self.channel.state.borrow_mut();
            let mc = state.mc_frame_count;
            let vc = state.vc_frame_count;
            state.mc_frame_count = state.mc_frame_count.wrapping_add(1);
            state.vc_frame_count = state.vc_frame_count.wrapping_add(1);
            (state.config.clone(), mc, vc)
        };

        let total_len = TelemetryTransferFrame::HEADER_SIZE + data.len();

        let frame = TelemetryTransferFrame::builder()
            .buffer(&mut self.frame_buffer[..total_len])
            .version(0)
            .scid(config.scid)
            .vcid(config.vcid)
            .mc_frame_count(mc_count)
            .vc_frame_count(vc_count)
            .first_header_pointer(0)
            .build()
            .map_err(|_| TmError::BuildError)?;

        frame.data_field_mut().copy_from_slice(data);

        if config.randomize {
            self.randomizer.apply(&mut self.frame_buffer[..total_len]);
        }

        self.writer
            .send(&self.frame_buffer[..total_len])
            .await
            .map_err(|e| TmError::Link(e.into()))?;

        Ok(())
    }
}

struct TmReceiverState<E, const QUEUE: usize, const MTU: usize> {
    config: TmConfig,
    received: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TmError<E>>,
    closed: bool,
}

/// Shared state channel for the TM receiver, split into handle and driver.
pub struct TmReceiverChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TmReceiverState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TmReceiverChannel<E, QUEUE, MTU> {
    /// Creates a new TM receiver channel with the given configuration.
    pub fn new(config: TmConfig) -> Self {
        Self {
            state: RefCell::new(TmReceiverState {
                config,
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
        TmReceiverHandle<'_, E, QUEUE, MTU>,
        TmReceiverDriver<'_, R, E, QUEUE, MTU>,
    ) {
        (
            TmReceiverHandle { channel: self },
            TmReceiverDriver {
                reader,
                channel: self,
                frame_buffer: [0u8; MTU],
                derandomize_buffer: [0u8; MTU],
                randomizer: Tm255Randomizer::new(),
            },
        )
    }
}

/// User-facing handle for receiving TM frame data.
pub struct TmReceiverHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TmReceiverChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TmReceiverHandle<'a, E, QUEUE, MTU> {
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
    for TmReceiverHandle<'a, E, QUEUE, MTU>
{
    type Error = TmError<E>;

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

/// Background driver that reads TM frames and enqueues parsed data.
pub struct TmReceiverDriver<'a, R, E, const QUEUE: usize, const MTU: usize> {
    reader: R,
    channel: &'a TmReceiverChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
    derandomize_buffer: [u8; MTU],
    randomizer: Tm255Randomizer,
}

impl<'a, R: FrameReceiver, E: Clone, const QUEUE: usize, const MTU: usize>
    TmReceiverDriver<'a, R, E, QUEUE, MTU>
where
    R::Error: Into<E>,
{
    /// Runs the receive loop, reading frames until the channel is closed.
    pub async fn run(&mut self) -> Result<(), TmError<E>> {
        loop {
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            let len = self
                .reader
                .recv(&mut self.frame_buffer)
                .await
                .map_err(|e| TmError::Link(e.into()))?;

            if len == 0 {
                continue;
            }

            let should_derandomize = self.channel.state.borrow().config.randomize;

            let frame = TelemetryTransferFrame::parse(
                &self.frame_buffer[..len],
                &mut self.derandomize_buffer,
                &OptionalRandomizer {
                    inner: &self.randomizer,
                    enabled: should_derandomize,
                },
            )
            .map_err(|_| TmError::InvalidFrame)?;

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

struct OptionalRandomizer<'a> {
    inner: &'a Tm255Randomizer,
    enabled: bool,
}

impl Randomizer for OptionalRandomizer<'_> {
    fn apply(&self, data: &mut [u8]) {
        if self.enabled {
            self.inner.apply(data);
        }
    }

    fn table(&self) -> &[u8] {
        self.inner.table()
    }
}
