use core::future::poll_fn;
use core::task::Poll;

use heapless::Deque;

use crate::coding::CodingReader;
use crate::coding::CodingWriter;
use crate::datalink::framing::sdlp::tc::BypassFlag;
use crate::datalink::framing::sdlp::tc::ControlFlag;
use crate::datalink::framing::sdlp::tc::TelecommandTransferFrame;
use crate::network::spp::SpacePacket;
use crate::utils::cell::SyncRefCell;

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
#[derive(Debug, Clone, thiserror::Error)]
pub enum TcError<E> {
    /// The underlying link returned an error.
    #[error("link error: {0}")]
    Link(E),
    /// The data exceeds the maximum frame data length.
    #[error("frame too large")]
    FrameTooLarge,
    /// A received frame failed to parse.
    #[error("invalid frame")]
    InvalidFrame,
    /// The internal send queue is full.
    #[error("send queue full")]
    QueueFull,
    /// Failed to construct a TC Transfer Frame.
    #[error("frame build error")]
    BuildError,
}

struct PendingPacket<const MTU: usize> {
    data: [u8; MTU],
    len: usize,
}

struct TcWriteState<E, const QUEUE: usize, const MTU: usize> {
    config: TcConfig,
    sequence: u8,
    pending: Deque<PendingPacket<MTU>, QUEUE>,
    error: Option<TcError<E>>,
    flush: bool,
    closed: bool,
}

/// Shared state channel for the TC writer, split into handle
/// and driver.
pub struct TcWriteChannel<E, const QUEUE: usize, const MTU: usize> {
    state: SyncRefCell<TcWriteState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TcWriteChannel<E, QUEUE, MTU> {
    /// Creates a new TC write channel with the given configuration.
    pub fn new(config: TcConfig) -> Self {
        Self {
            state: SyncRefCell::new(TcWriteState {
                config,
                sequence: 0,
                pending: Deque::new(),
                error: None,
                flush: false,
                closed: false,
            }),
        }
    }

    /// Splits the channel into a handle for enqueuing and a
    /// driver for processing.
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
                accum_len: 0,
            },
        )
    }
}

/// User-facing handle for enqueuing Space Packets to send.
pub struct TcWriteHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TcWriteChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TcWriteHandle<'a, E, QUEUE, MTU> {
    /// Enqueues a Space Packet to be packed into a TC frame.
    ///
    /// Multiple packets are accumulated into a single frame.
    /// The frame is sent when it fills up or [`flush`](Self::flush)
    /// is called. Waits if the queue is full.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), TcError<E>> {
        poll_fn(|_cx| {
            self.channel.state.with_mut(|state| {
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
        })
        .await
    }

    /// Forces the driver to send the current partially-filled
    /// frame.
    pub fn flush(&mut self) {
        self.channel.state.with_mut(|s| s.flush = true);
    }

    /// Signals that no more data will be sent on this channel.
    pub fn close(&mut self) {
        self.channel.state.with_mut(|s| {
            s.flush = true;
            s.closed = true;
        });
    }

    /// Returns true if the send queue is empty.
    pub fn is_empty(&self) -> bool {
        self.channel.state.with(|s| s.pending.is_empty())
    }
}

/// Background driver that packs Space Packets into TC Transfer
/// Frames and writes them through the coding pipeline.
pub struct TcWriteDriver<'a, W, E, const QUEUE: usize, const MTU: usize> {
    writer: W,
    channel: &'a TcWriteChannel<E, QUEUE, MTU>,
    frame_buffer: [u8; MTU],
    accum_len: usize,
}

impl<'a, W: CodingWriter, E: Clone, const QUEUE: usize, const MTU: usize>
    TcWriteDriver<'a, W, E, QUEUE, MTU>
where
    W::Error: Into<E>,
{
    /// Tries to pack one pending packet into the frame buffer.
    ///
    /// Returns `true` if a packet was packed.
    fn try_pack_one(&mut self) -> bool {
        self.channel.state.with_mut(|state| {
            let Some(front) = state.pending.front() else {
                return false;
            };
            if self.accum_len + front.len > state.config.max_frame_data_len {
                return false;
            }
            let packet = state.pending.pop_front().unwrap();
            let offset = TelecommandTransferFrame::HEADER_SIZE + self.accum_len;
            self.frame_buffer[offset..offset + packet.len]
                .copy_from_slice(&packet.data[..packet.len]);
            self.accum_len += packet.len;
            true
        })
    }

    /// Returns `true` if the next pending packet would not fit
    /// in the current frame.
    fn frame_full(&self) -> bool {
        self.channel.state.with(|s| match s.pending.front() {
            Some(p) => self.accum_len + p.len > s.config.max_frame_data_len,
            None => false,
        })
    }

    /// Finalizes the accumulated data into a TC Transfer Frame
    /// and sends it.
    async fn send_frame(&mut self) -> Result<(), TcError<E>> {
        let frame_len = self.channel.state.with_mut(|state| {
            let seq = state.sequence;
            state.sequence = state.sequence.wrapping_add(1);

            let total = TelecommandTransferFrame::HEADER_SIZE + self.accum_len;

            TelecommandTransferFrame::builder()
                .buffer(&mut self.frame_buffer[..total])
                .scid(state.config.scid)
                .vcid(state.config.vcid)
                .bypass_flag(state.config.bypass)
                .control_flag(state.config.control)
                .seq(seq)
                .data_field_len(self.accum_len)
                .build()
                .map_err(|_| TcError::BuildError)?;

            Ok::<_, TcError<E>>(total)
        })?;

        self.accum_len = 0;

        self.writer
            .write(&self.frame_buffer[..frame_len])
            .await
            .map_err(|e| TcError::Link(e.into()))
    }

    /// Runs the send loop, packing Space Packets into frames
    /// and sending them when full or flushed.
    pub async fn run(&mut self) -> Result<(), TcError<E>> {
        loop {
            poll_fn(|_cx| {
                self.channel.state.with(|s| {
                    if s.closed || s.flush || !s.pending.is_empty() {
                        Poll::Ready(())
                    } else {
                        Poll::Pending
                    }
                })
            })
            .await;

            // Pack as many pending packets as fit.
            while self.try_pack_one() {
                if self.frame_full() {
                    self.send_frame().await?;
                }
            }

            // Send if flushed or closed.
            let flush = self.channel.state.with_mut(|s| {
                let f = s.flush;
                s.flush = false;
                f
            });

            if flush && self.accum_len > 0 {
                self.send_frame().await?;
            }

            if self
                .channel
                .state
                .with(|s| s.closed && s.pending.is_empty())
            {
                return Ok(());
            }
        }
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
    state: SyncRefCell<TcReadState<E, QUEUE, MTU>>,
}

impl<E: Clone, const QUEUE: usize, const MTU: usize> TcReadChannel<E, QUEUE, MTU> {
    /// Creates a new TC read channel.
    pub fn new() -> Self {
        Self {
            state: SyncRefCell::new(TcReadState {
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

/// User-facing handle for receiving Space Packets from TC
/// frames.
pub struct TcReadHandle<'a, E, const QUEUE: usize, const MTU: usize> {
    channel: &'a TcReadChannel<E, QUEUE, MTU>,
}

impl<'a, E: Clone, const QUEUE: usize, const MTU: usize> TcReadHandle<'a, E, QUEUE, MTU> {
    /// Signals that no more data should be received on this
    /// channel.
    pub fn close(&mut self) {
        self.channel.state.with_mut(|s| s.closed = true);
    }

    /// Returns true if there is received data available.
    pub fn has_data(&self) -> bool {
        !self.channel.state.with(|s| s.received.is_empty())
    }

    /// Receives the next Space Packet from a TC frame.
    pub async fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, TcError<E>> {
        poll_fn(|_cx| {
            self.channel.state.with_mut(|state| {
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
        })
        .await
    }
}

/// Background driver that reads TC frames from the coding
/// pipeline and extracts individual Space Packets.
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
    /// Runs the receive loop, reading frames and extracting
    /// Space Packets until the channel is closed.
    pub async fn run(&mut self) -> Result<(), TcError<E>> {
        loop {
            if self.channel.state.with(|s| s.closed) {
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

            let mut pos = 0;
            while pos < data_field.len() {
                let remaining = &data_field[pos..];
                let Ok(packet) = SpacePacket::parse(remaining) else {
                    break;
                };
                let pkt_len = packet.primary_header.packet_len();

                self.channel.state.with_mut(|state| {
                    if !state.received.is_full() {
                        let mut entry = PendingPacket {
                            data: [0u8; MTU],
                            len: pkt_len,
                        };
                        entry.data[..pkt_len].copy_from_slice(&remaining[..pkt_len]);
                        state.received.push_back(entry).ok();
                    }
                });

                pos += pkt_len;
            }
        }
    }
}
