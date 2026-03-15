//! ISL Gossip protocol — epidemic flood with epoch dedup.
//!
//! [`Gossip`] owns four directional datalinks and implements
//! [`NetworkRead`] + [`NetworkWrite`]. Every packet is flooded
//! to all neighbors (minus the predecessor) and delivered
//! locally. Duplicate epochs are silently dropped.

/// Sliding-window duplicate filter for epoch-based deduplication.
pub mod bitmap;
/// Gossip packet structure and builder.
pub mod packet;

use futures::FutureExt as _;
use futures::future::Either;
use zerocopy::FromBytes as _;
use zerocopy::IntoBytes as _;
use zerocopy::network_endian::U16;

use bitmap::DuplicateFilter;

use crate::datalink::Datalink;
use crate::datalink::DatalinkRead;
use crate::datalink::DatalinkWrite;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::network::isl::gossip::packet::Epoch;
use crate::network::isl::gossip::packet::IslGossipTelecommand;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;
use crate::network::spp::Apid;
use crate::utils::ringbuf::RingBuffer;

/// Per-direction link state: the link itself plus its input
/// buffer, output queue, and staging buffer.
struct Port<L, const MTU: usize, const OUT: usize> {
    link: L,
    buf: [u8; MTU],
    out: RingBuffer<OUT>,
    stage: [u8; MTU],
}

impl<L, const MTU: usize, const OUT: usize> Port<L, MTU, OUT> {
    fn new(link: L) -> Self {
        Self {
            link,
            buf: [0u8; MTU],
            out: RingBuffer::new(),
            stage: [0u8; MTU],
        }
    }
}

/// Error from a directional link or from gossip parsing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GossipError<E> {
    /// Error on the north link.
    #[error("North link error: {0}")]
    North(E),
    /// Error on the south link.
    #[error("South link error: {0}")]
    South(E),
    /// Error on the east link.
    #[error("East link error: {0}")]
    East(E),
    /// Error on the west link.
    #[error("West link error: {0}")]
    West(E),
    /// The caller's buffer is too small for the payload.
    #[error("buffer too small: need {needed} bytes, got {provided}")]
    BufferTooSmall {
        /// Payload size in bytes.
        needed: usize,
        /// Caller buffer size in bytes.
        provided: usize,
    },
}

/// Epidemic gossip flood over a 4-connected torus mesh.
///
/// Implements [`NetworkWrite`] (wrap + flood) and
/// [`NetworkRead`] (receive, dedup, forward, deliver).
pub struct Gossip<N, const MTU: usize = 256, const OUT: usize = 2048> {
    north: Port<N, MTU, OUT>,
    south: Port<N, MTU, OUT>,
    east: Port<N, MTU, OUT>,
    west: Port<N, MTU, OUT>,
    address: Address,
    torus: Torus,
    apid: Apid,
    function_code: u8,
    dedup: DuplicateFilter,
    next_epoch: u16,
    buf: [u8; MTU],
}

#[bon::bon]
impl<N, const MTU: usize, const OUT: usize> Gossip<N, MTU, OUT>
where
    N: Datalink,
{
    #[builder]
    /// Creates a new gossip node with directional links.
    pub fn new(
        north: N,
        south: N,
        east: N,
        west: N,
        address: Address,
        torus: Torus,
        apid: Apid,
        function_code: u8,
    ) -> Self {
        Self {
            north: Port::new(north),
            south: Port::new(south),
            east: Port::new(east),
            west: Port::new(west),
            address,
            torus,
            apid,
            function_code,
            dedup: DuplicateFilter::new(),
            next_epoch: 0,
            buf: [0u8; MTU],
        }
    }

    /// Returns this node's own address.
    pub fn address(&self) -> Address {
        self.address
    }
}

impl<N, const MTU: usize, const OUT: usize> Gossip<N, MTU, OUT>
where
    N: Datalink,
{
    /// Compute which directions to flood a locally-originated
    /// packet (all neighbors in service area).
    fn flood_directions(
        &self,
        service_area_min: u8,
        service_area_max: u8,
    ) -> [bool; 4] {
        let my_point = Point::from(self.address);
        let all = [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ];
        let mut dirs = [false; 4];
        for (i, direction) in all.iter().enumerate() {
            let neighbor = self.torus.neighbor(my_point, *direction);
            let addr = Address::from(neighbor);
            dirs[i] = addr.is_in_service_area(
                service_area_min,
                service_area_max,
            );
        }
        dirs
    }

}

impl<N, const MTU: usize, const OUT: usize> NetworkWrite
    for Gossip<N, MTU, OUT>
where
    N: Datalink,
{
    type Error = GossipError<N::WriteError>;

    async fn write(
        &mut self,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        let epoch_val = self.next_epoch;
        self.next_epoch = self.next_epoch.wrapping_add(1);
        let epoch = Epoch(U16::new(epoch_val));
        self.dedup.is_duplicate(epoch.0.get());

        let pkt = IslGossipTelecommand::builder()
            .buffer(&mut self.buf)
            .apid(self.apid)
            .function_code(self.function_code)
            .origin(self.address)
            .predecessor(self.address)
            .service_area_min(0)
            .service_area_max(255)
            .epoch(epoch)
            .payload_len(data.len())
            .build()
            .ok();

        let Some(pkt) = pkt else {
            return Ok(());
        };
        pkt.payload.copy_from_slice(data);
        pkt.set_cfe_checksum();
        let len = pkt.as_bytes().len();

        let dirs = self.flood_directions(0, 255);

        if dirs[0] {
            let (_, mut w) = self.north.link.split();
            w.write(&self.buf[..len])
                .await
                .map_err(GossipError::North)?;
        }
        if dirs[1] {
            let (_, mut w) = self.south.link.split();
            w.write(&self.buf[..len])
                .await
                .map_err(GossipError::South)?;
        }
        if dirs[2] {
            let (_, mut w) = self.east.link.split();
            w.write(&self.buf[..len])
                .await
                .map_err(GossipError::East)?;
        }
        if dirs[3] {
            let (_, mut w) = self.west.link.split();
            w.write(&self.buf[..len])
                .await
                .map_err(GossipError::West)?;
        }

        Ok(())
    }
}

impl<N, const MTU: usize, const OUT: usize> NetworkRead
    for Gossip<N, MTU, OUT>
where
    N: Datalink,
{
    type Error = GossipError<N::ReadError>;

    async fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> Result<usize, Self::Error> {
        loop {
            let Self {
                north,
                south,
                east,
                west,
                address,
                torus,
                dedup,
                ..
            } = self;

            let (mut nr, mut nw) = north.link.split();
            let (mut sr, mut sw) = south.link.split();
            let (mut er, mut ew) = east.link.split();
            let (mut wr, mut ww) = west.link.split();

            macro_rules! stage {
                ($w:expr, $port:expr) => {
                    match $port.out.front() {
                        Some(data) => {
                            let len = data.len();
                            $port.stage[..len]
                                .copy_from_slice(data);
                            Either::Left(
                                $w.write(&$port.stage[..len])
                                    .fuse(),
                            )
                        }
                        None => {
                            Either::Right(
                                futures::future::pending(),
                            )
                        }
                    }
                };
            }

            enum Event<RE> {
                Read(Result<usize, RE>, Direction),
                Write(Direction),
            }

            let event = {
                let nw_f = stage!(nw, north);
                let sw_f = stage!(sw, south);
                let ew_f = stage!(ew, east);
                let ww_f = stage!(ww, west);

                let nr_f = nr.read(&mut north.buf).fuse();
                let sr_f = sr.read(&mut south.buf).fuse();
                let er_f = er.read(&mut east.buf).fuse();
                let wr_f = wr.read(&mut west.buf).fuse();

                pin_utils::pin_mut!(
                    nr_f, sr_f, er_f, wr_f, nw_f, sw_f,
                    ew_f, ww_f
                );

                futures::select_biased! {
                    _ = nw_f => Event::Write(Direction::North),
                    _ = sw_f => Event::Write(Direction::South),
                    _ = ew_f => Event::Write(Direction::East),
                    _ = ww_f => Event::Write(Direction::West),
                    r = nr_f => Event::Read(r, Direction::North),
                    r = sr_f => Event::Read(r, Direction::South),
                    r = er_f => Event::Read(r, Direction::East),
                    r = wr_f => Event::Read(r, Direction::West),
                }
            };

            match event {
                Event::Write(dir) => match dir {
                    Direction::North => north.out.pop(),
                    Direction::South => south.out.pop(),
                    Direction::East => east.out.pop(),
                    Direction::West => west.out.pop(),
                },
                Event::Read(result, dir) => {
                    let (buf, len) = match dir {
                        Direction::North => (
                            &north.buf[..],
                            result.map_err(GossipError::North)?,
                        ),
                        Direction::South => (
                            &south.buf[..],
                            result.map_err(GossipError::South)?,
                        ),
                        Direction::East => (
                            &east.buf[..],
                            result.map_err(GossipError::East)?,
                        ),
                        Direction::West => (
                            &west.buf[..],
                            result.map_err(GossipError::West)?,
                        ),
                    };

                    let raw = &buf[..len];
                    let Ok(pkt) =
                        IslGossipTelecommand::ref_from_bytes(raw)
                    else {
                        continue;
                    };

                    let header = &pkt.gossip_header;
                    let epoch = header.epoch();

                    if dedup.is_duplicate(epoch.0.get()) {
                        continue;
                    }

                    let predecessor = header.predecessor();
                    let my_point = Point::from(*address);
                    let all = [
                        Direction::North,
                        Direction::South,
                        Direction::East,
                        Direction::West,
                    ];
                    let fwd = {
                        let mut dirs = [false; 4];
                        for (i, d) in
                            all.iter().enumerate()
                        {
                            let n =
                                torus.neighbor(my_point, *d);
                            let a = Address::from(n);
                            dirs[i] = a != predecessor
                                && a.is_in_service_area(
                                    header.service_area_min,
                                    header.service_area_max,
                                );
                        }
                        dirs
                    };

                    if fwd[0] {
                        north.out.push(raw);
                    }
                    if fwd[1] {
                        south.out.push(raw);
                    }
                    if fwd[2] {
                        east.out.push(raw);
                    }
                    if fwd[3] {
                        west.out.push(raw);
                    }

                    let payload = &pkt.payload;
                    let payload_len = payload.len();
                    if buffer.len() < payload_len {
                        return Err(GossipError::BufferTooSmall {
                            needed: payload_len,
                            provided: buffer.len(),
                        });
                    }
                    buffer[..payload_len]
                        .copy_from_slice(payload);
                    return Ok(payload_len);
                }
            }
        }
    }
}
