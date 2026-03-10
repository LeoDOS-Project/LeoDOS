//! ISL Gossip protocol with channel-based sender/receiver/driver split.
//!
//! The [`GossipChannel`] owns shared state and is split into three handles:
//! - [`GossipSender`] — app-side, queues gossip for flooding
//! - [`GossipReceiver`] — app-side, receives unique gossip messages
//! - [`GossipDriver`] — router-side, processes incoming packets and
//!   drains outgoing gossip

/// Sliding-window duplicate filter for epoch-based deduplication.
pub mod bitmap;
/// Gossip packet structure and builder.
pub mod packet;

use core::future::poll_fn;
use core::task::Poll;

use bitmap::DuplicateFilter;
use heapless::Deque;
use heapless::Vec;
use zerocopy::IntoBytes;
use zerocopy::network_endian::U16;

use crate::network::isl::address::Address;
use crate::network::isl::gossip::packet::Epoch;
use crate::network::isl::gossip::packet::IslGossipTelecommand;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;
use crate::network::spp::Apid;
use crate::utils::cell::SyncRefCell;

/// Error from a gossip channel operation.
#[derive(Debug, Clone)]
pub enum GossipError {
    /// The channel has been closed.
    Closed,
}

impl core::fmt::Display for GossipError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Closed => write!(f, "gossip channel closed"),
        }
    }
}

impl core::error::Error for GossipError {}

/// Configuration for a gossip channel.
#[derive(Clone, Debug)]
pub struct GossipConfig {
    /// Network topology.
    pub torus: Torus,
    /// This node's address.
    pub my_address: Address,
    /// APID for outgoing gossip packets.
    pub apid: Apid,
    /// cFE function code for outgoing gossip packets.
    pub function_code: u8,
}

/// Metadata about a received gossip message, returned by
/// [`GossipReceiver::recv`].
#[derive(Clone, Debug)]
pub struct GossipMessage {
    /// The node that originated this gossip.
    pub origin: Address,
    /// Number of payload bytes written to the buffer.
    pub len: usize,
}

// ── Internal buffer types ──

struct PacketBuf<const MTU: usize> {
    origin: Address,
    data: [u8; MTU],
    len: usize,
}

struct OutgoingGossip<const MTU: usize> {
    service_area_min: u8,
    service_area_max: u8,
    data: [u8; MTU],
    len: usize,
}

struct GossipState<const QUEUE: usize, const MTU: usize> {
    /// Locally-originated gossip waiting to be flooded.
    outbox: Deque<OutgoingGossip<MTU>, QUEUE>,
    /// Unique received gossip waiting for the app to read.
    inbox: Deque<PacketBuf<MTU>, QUEUE>,
    /// Sliding-window epoch deduplication filter.
    dedup: DuplicateFilter,
    /// Counter for assigning epochs to locally-originated gossip.
    next_epoch: u16,
    /// Whether the channel has been closed.
    closed: bool,
}

// ── Channel ──

/// Shared gossip state that is split into sender, receiver, and
/// driver handles.
pub struct GossipChannel<const QUEUE: usize = 8, const MTU: usize = 256> {
    state: SyncRefCell<GossipState<QUEUE, MTU>>,
    config: GossipConfig,
}

impl<const QUEUE: usize, const MTU: usize> GossipChannel<QUEUE, MTU> {
    /// Creates a new gossip channel.
    pub fn new(config: GossipConfig) -> Self {
        Self {
            state: SyncRefCell::new(GossipState {
                outbox: Deque::new(),
                inbox: Deque::new(),
                dedup: DuplicateFilter::new(),
                next_epoch: 0,
                closed: false,
            }),
            config,
        }
    }

    /// Splits into app-side sender/receiver and a router-side driver.
    pub fn split(
        &self,
    ) -> (
        GossipSender<'_, QUEUE, MTU>,
        GossipReceiver<'_, QUEUE, MTU>,
        GossipDriver<'_, QUEUE, MTU>,
    ) {
        (
            GossipSender { channel: self },
            GossipReceiver { channel: self },
            GossipDriver { channel: self },
        )
    }

    /// Closes the channel.
    pub fn close(&self) {
        self.state.with_mut(|s| s.closed = true);
    }
}

// ── App-side sender ──

/// Handle for originating gossip messages.
pub struct GossipSender<'a, const QUEUE: usize, const MTU: usize> {
    channel: &'a GossipChannel<QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> GossipSender<'a, QUEUE, MTU> {
    /// Queue a gossip message for flooding to neighbors.
    ///
    /// Waits asynchronously if the outbox is full.
    pub async fn send(
        &mut self,
        service_area_min: u8,
        service_area_max: u8,
        data: &[u8],
    ) -> Result<(), GossipError> {
        poll_fn(|_cx| {
            self.channel.state.with_mut(|state| {
                if state.closed {
                    return Poll::Ready(Err(GossipError::Closed));
                }

                if state.outbox.is_full() {
                    return Poll::Pending;
                }

                let len = data.len().min(MTU);
                let mut buf = OutgoingGossip {
                    service_area_min,
                    service_area_max,
                    data: [0u8; MTU],
                    len,
                };
                buf.data[..len].copy_from_slice(&data[..len]);
                state.outbox.push_back(buf).ok();

                Poll::Ready(Ok(()))
            })
        })
        .await
    }
}

// ── App-side receiver ──

/// Handle for receiving unique gossip messages.
pub struct GossipReceiver<'a, const QUEUE: usize, const MTU: usize> {
    channel: &'a GossipChannel<QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> GossipReceiver<'a, QUEUE, MTU> {
    /// Receive the next unique gossip message.
    ///
    /// Blocks until a message is available. The payload is copied
    /// into `buf` and metadata is returned in [`GossipMessage`].
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<GossipMessage, GossipError> {
        poll_fn(|_cx| {
            self.channel.state.with_mut(|state| {
                if let Some(pkt) = state.inbox.pop_front() {
                    let len = pkt.len.min(buf.len());
                    buf[..len].copy_from_slice(&pkt.data[..len]);
                    return Poll::Ready(Ok(GossipMessage {
                        origin: pkt.origin,
                        len,
                    }));
                }

                if state.closed {
                    return Poll::Ready(Err(GossipError::Closed));
                }

                Poll::Pending
            })
        })
        .await
    }

    /// Try to receive without blocking.
    pub fn try_recv(&mut self, buf: &mut [u8]) -> Option<GossipMessage> {
        self.channel.state.with_mut(|state| {
            let pkt = state.inbox.pop_front()?;
            let len = pkt.len.min(buf.len());
            buf[..len].copy_from_slice(&pkt.data[..len]);
            Some(GossipMessage {
                origin: pkt.origin,
                len,
            })
        })
    }
}

// ── Router-side driver ──

/// Handle used by the router to process incoming gossip and drain
/// outgoing gossip.
pub struct GossipDriver<'a, const QUEUE: usize, const MTU: usize> {
    channel: &'a GossipChannel<QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> GossipDriver<'a, QUEUE, MTU> {
    /// Process an incoming gossip packet.
    ///
    /// Deduplicates by epoch, enqueues unique messages for the
    /// receiver, and returns the directions to forward the packet.
    pub fn process_incoming<'p>(
        &self,
        packet: &'p IslGossipTelecommand,
    ) -> Vec<(Direction, &'p IslGossipTelecommand), 4> {
        let header = &packet.gossip_header;
        let epoch = header.epoch();

        let is_new = self.channel.state.with_mut(|state| {
            if state.dedup.is_duplicate(epoch.0.get()) {
                return false;
            }

            if !state.inbox.is_full() {
                let payload = &packet.payload;
                let len = payload.len().min(MTU);
                let mut entry = PacketBuf {
                    origin: header.origin(),
                    data: [0u8; MTU],
                    len,
                };
                entry.data[..len].copy_from_slice(&payload[..len]);
                state.inbox.push_back(entry).ok();
            }

            true
        });

        if is_new {
            self.forward_gossip(packet)
        } else {
            Vec::new()
        }
    }

    /// Take the next locally-originated gossip packet for flooding.
    ///
    /// Builds the full gossip packet in `buf`. Returns the packet
    /// length and the directions to send it, or `None` if the
    /// outbox is empty.
    pub fn poll_outgoing(&self, buf: &mut [u8]) -> Option<(usize, Vec<Direction, 4>)> {
        let (outgoing, epoch) = self.channel.state.with_mut(|state| {
            let outgoing = state.outbox.pop_front()?;
            let e = state.next_epoch;
            state.next_epoch = state.next_epoch.wrapping_add(1);
            let epoch = Epoch(U16::new(e));
            state.dedup.is_duplicate(epoch.0.get());
            Some((outgoing, epoch))
        })?;

        let config = &self.channel.config;
        let pkt = IslGossipTelecommand::builder()
            .buffer(buf)
            .apid(config.apid)
            .function_code(config.function_code)
            .origin(config.my_address)
            .predecessor(config.my_address)
            .service_area_min(outgoing.service_area_min)
            .service_area_max(outgoing.service_area_max)
            .epoch(epoch)
            .payload_len(outgoing.len)
            .build()
            .ok()?;
        pkt.payload.copy_from_slice(&outgoing.data[..outgoing.len]);
        pkt.set_cfe_checksum();
        let len = pkt.as_bytes().len();

        let directions =
            self.flood_directions(outgoing.service_area_min, outgoing.service_area_max);

        Some((len, directions))
    }

    /// Compute which neighbor directions to forward a packet to.
    fn forward_gossip<'p>(
        &self,
        packet: &'p IslGossipTelecommand,
    ) -> Vec<(Direction, &'p IslGossipTelecommand), 4> {
        let header = &packet.gossip_header;
        let from_address = header.predecessor();
        let config = &self.channel.config;

        let mut forwards = Vec::new();
        for direction in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            let my_point = Point::from(config.my_address);
            let neighbor_point = config.torus.neighbor(my_point, direction);
            let to_address = Address::from(neighbor_point);

            if to_address != from_address
                && to_address.is_in_service_area(header.service_area_min, header.service_area_max)
            {
                forwards
                    .push((direction, packet))
                    .expect("cannot exceed capacity");
            }
        }
        forwards
    }

    /// Compute flood directions for a locally-originated packet
    /// (no sender to skip).
    fn flood_directions(&self, service_area_min: u8, service_area_max: u8) -> Vec<Direction, 4> {
        let config = &self.channel.config;

        let mut directions = Vec::new();
        for direction in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            let my_point = Point::from(config.my_address);
            let neighbor_point = config.torus.neighbor(my_point, direction);
            let to_address = Address::from(neighbor_point);

            if to_address.is_in_service_area(service_area_min, service_area_max) {
                directions.push(direction).expect("cannot exceed capacity");
            }
        }
        directions
    }
}
