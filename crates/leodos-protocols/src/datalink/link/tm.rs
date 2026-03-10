use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use heapless::Deque;

use crate::coding::{CodingReader, CodingWriter};
use crate::datalink::framing::sdlp::tm::TelemetryTransferFrame;

/// Configuration for a Telemetry link channel.
#[derive(Debug, Clone)]
pub struct TmConfig {
    /// Spacecraft ID for TM frames.
    pub scid: u16,
    /// Virtual Channel ID for TM frames.
    pub vcid: u8,
    /// Maximum data field length in bytes.
    pub max_frame_data_len: usize,
}

/// Errors that can occur during TM link operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum TmError<E> {
    /// The underlying link returned an error.
    #[error("link error: {0}")]
    Link(E),
    /// The data exceeds the maximum frame data length.
    #[error("frame too large")]
    FrameTooLarge,
    /// A received frame failed to parse.
    #[error("invalid frame")]
    InvalidFrame,
    /// The internal receive queue is full.
    #[error("send queue full")]
    QueueFull,
    /// Failed to construct a TM Transfer Frame.
    #[error("frame build error")]
    BuildError,
}

struct PendingPacket<const MTU: usize> {
    data: [u8; MTU],
    len: usize,
}

struct TmWriteState<E, const QUEUE: usize, const MTU: usize> {
    config: TmConfig,
    mc_frame_count: u8,
    vc_frame_count: u8,
    pending: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TmError<E>>,
    closed: bool,
}

/// Shared state channel for the TM writer, split into handle
/// and driver.
pub struct TmWriteChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TmWriteState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TmWriteChannel<E, QUEUE, MTU> {
    /// Creates a new TM write channel with the given configuration.
    pub fn new(config: TmConfig) -> Self {
        Self {
            state: RefCell::new(TmWriteState {
                config,
                mc_frame_count: 0,
                vc_frame_count: 0,
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
        TmWriteHandle<'_, E, QUEUE, MTU>,
        TmWriteDriver<'_, W, E, QUEUE, MTU>,
    ) {
        (
            TmWriteHandle { channel: self },
            TmWriteDriver {
                writer,
                channel: self,
                frame_buffer: [0u8; MTU],
            },
        )
    }
}

/// User-facing handle for enqueuing TM frames to send.
pub struct TmWriteHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TmWriteChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TmWriteHandle<'a, E, QUEUE, MTU> {
    /// Signals that no more data will be sent on this channel.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Returns true if the send queue is empty.
    pub fn is_empty(&self) -> bool {
        self.channel.state.borrow().pending.is_empty()
    }

    /// Enqueues data to be sent as a TM frame, waiting if
    /// the queue is full.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), TmError<E>> {
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

/// Background driver that dequeues pending packets and writes TM
/// frames through the coding pipeline.
pub struct TmWriteDriver<'a, W, E, const QUEUE: usize, const MTU: usize> {
    writer: W,
    channel: &'a TmWriteChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
}

impl<'a, W: CodingWriter, E: Clone, const QUEUE: usize, const MTU: usize>
    TmWriteDriver<'a, W, E, QUEUE, MTU>
where
    W::Error: Into<E>,
{
    /// Runs the send loop, processing queued packets until the
    /// channel is closed.
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

        self.writer
            .write(&self.frame_buffer[..total_len])
            .await
            .map_err(|e| TmError::Link(e.into()))?;

        Ok(())
    }
}

struct TmReadState<E, const QUEUE: usize, const MTU: usize> {
    received: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TmError<E>>,
    closed: bool,
}

/// Shared state channel for the TM reader, split into handle
/// and driver.
pub struct TmReadChannel<E, const QUEUE: usize, const MTU: usize> {
    state: RefCell<TmReadState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TmReadChannel<E, QUEUE, MTU> {
    /// Creates a new TM read channel.
    pub fn new() -> Self {
        Self {
            state: RefCell::new(TmReadState {
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
        TmReadHandle<'_, E, QUEUE, MTU>,
        TmReadDriver<'_, R, E, QUEUE, MTU>,
    ) {
        (
            TmReadHandle { channel: self },
            TmReadDriver {
                reader,
                channel: self,
                frame_buffer: [0u8; MTU],
            },
        )
    }
}

/// User-facing handle for receiving TM frame data.
pub struct TmReadHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TmReadChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TmReadHandle<'a, E, QUEUE, MTU> {
    /// Signals that no more data should be received on this
    /// channel.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Returns true if there is received data available.
    pub fn has_data(&self) -> bool {
        !self.channel.state.borrow().received.is_empty()
    }

    /// Receives TM data into the buffer, waiting if no data is
    /// available.
    pub async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, TmError<E>> {
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

/// Background driver that reads TM frames from the coding
/// pipeline and enqueues parsed data.
pub struct TmReadDriver<'a, R, E, const QUEUE: usize, const MTU: usize> {
    reader: R,
    channel: &'a TmReadChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
}

impl<'a, R: CodingReader, E: Clone, const QUEUE: usize, const MTU: usize>
    TmReadDriver<'a, R, E, QUEUE, MTU>
where
    R::Error: Into<E>,
{
    /// Runs the receive loop, reading frames until the channel is
    /// closed.
    pub async fn run(&mut self) -> Result<(), TmError<E>> {
        loop {
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            let len = self
                .reader
                .read(&mut self.frame_buffer)
                .await
                .map_err(|e| TmError::Link(e.into()))?;

            if len == 0 {
                continue;
            }

            // Parse the raw frame (no derandomization here — the
            // CodingReader pipeline already handled it).
            let frame = TelemetryTransferFrame::parse_raw(&self.frame_buffer[..len])
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
