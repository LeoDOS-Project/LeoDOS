use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use heapless::Deque;

use crate::coding::{CodingReader, CodingWriter};
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

struct TcWriteState<E, const QUEUE: usize, const MTU: usize> {
    config: TcConfig,
    sequence: u8,
    pending: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TcError<E>>,
    closed: bool,
}

/// Shared state channel for the TC writer, split into handle
/// and driver.
pub struct TcWriteChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TcWriteState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TcWriteChannel<E, QUEUE, MTU> {
    /// Creates a new TC write channel with the given configuration.
    pub fn new(config: TcConfig) -> Self {
        Self {
            state: RefCell::new(TcWriteState {
                config,
                sequence: 0,
                pending: Deque::new(),
                error: None,
                closed: false,
            }),
        }
    }

    /// Splits the channel into a handle for enqueuing and a driver
    /// for processing.
    pub fn split<W: CodingWriter<Error = E>>(
        &self,
        writer: W,
    ) -> (
        TcWriteHandle<'_, E, QUEUE, MTU>,
        TcWriteDriver<'_, W, E, QUEUE, MTU>,
    ) {
        (
            TcWriteHandle { channel: self },
            TcWriteDriver {
                writer,
                channel: self,
                frame_buffer: [0u8; MTU],
            },
        )
    }
}

/// User-facing handle for enqueuing TC frames to send.
pub struct TcWriteHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TcWriteChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TcWriteHandle<'a, E, QUEUE, MTU> {
    /// Enqueues data to be sent as a TC frame, waiting if the
    /// queue is full.
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

/// Background driver that dequeues pending packets and writes TC
/// frames through the coding pipeline.
pub struct TcWriteDriver<'a, W, E, const QUEUE: usize, const MTU: usize> {
    writer: W,
    channel: &'a TcWriteChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
}

impl<'a, W: CodingWriter, E: Clone, const QUEUE: usize, const MTU: usize>
    TcWriteDriver<'a, W, E, QUEUE, MTU>
where
    W::Error: Into<E>,
{
    /// Runs the send loop, processing queued packets until the
    /// channel is closed.
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
            .write(&self.frame_buffer[..frame_len])
            .await
            .map_err(|e| TcError::Link(e.into()))?;

        Ok(())
    }
}

struct TcReadState<E, const QUEUE: usize, const MTU: usize> {
    received: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TcError<E>>,
    closed: bool,
}

/// Shared state channel for the TC reader, split into handle
/// and driver.
pub struct TcReadChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TcReadState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TcReadChannel<E, QUEUE, MTU> {
    /// Creates a new TC read channel.
    pub fn new() -> Self {
        Self {
            state: RefCell::new(TcReadState {
                received: Deque::new(),
                error: None,
                closed: false,
            }),
        }
    }

    /// Splits the channel into a handle for reading and a driver
    /// for processing.
    pub fn split<R: CodingReader<Error = E>>(
        &self,
        reader: R,
    ) -> (
        TcReadHandle<'_, E, QUEUE, MTU>,
        TcReadDriver<'_, R, E, QUEUE, MTU>,
    ) {
        (
            TcReadHandle { channel: self },
            TcReadDriver {
                reader,
                channel: self,
                frame_buffer: [0u8; MTU],
            },
        )
    }
}

/// User-facing handle for receiving TC frame data.
pub struct TcReadHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TcReadChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TcReadHandle<'a, E, QUEUE, MTU> {
    /// Signals that no more data should be received on this
    /// channel.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Returns true if there is received data available.
    pub fn has_data(&self) -> bool {
        !self.channel.state.borrow().received.is_empty()
    }

    /// Receives TC data into the buffer, waiting if no data is
    /// available.
    pub async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, TcError<E>> {
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

/// Background driver that reads TC frames from the coding
/// pipeline and enqueues parsed data.
pub struct TcReadDriver<'a, R, E, const QUEUE: usize, const MTU: usize> {
    reader: R,
    channel: &'a TcReadChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
}

impl<'a, R: CodingReader, E: Clone, const QUEUE: usize, const MTU: usize>
    TcReadDriver<'a, R, E, QUEUE, MTU>
where
    R::Error: Into<E>,
{
    /// Runs the receive loop, reading frames until the channel is
    /// closed.
    pub async fn run(&mut self) -> Result<(), TcError<E>> {
        loop {
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            let len = self
                .reader
                .read(&mut self.frame_buffer)
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
