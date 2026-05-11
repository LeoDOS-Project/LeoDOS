/// Routing algorithm trait and implementations.
pub mod algorithm;
/// ISL routable packet definitions and builders.
pub mod packet;


use futures::FutureExt as _;
use futures::future::Either;
use zerocopy::IntoBytes as _;

use crate::buffer_pool::BufferPool;
use crate::datalink::Datalink;
use crate::datalink::DatalinkRead;
use crate::datalink::DatalinkWrite;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl;
use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::packet::IslRoutingTelecommand;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::{Direction, Hop};
use crate::utils::clock::Clock;
use crate::utils::ringbuf::RingBuffer;

/// Per-direction link state: the link itself plus its
/// pool-allocated input buffer, output queue, and staging buffer.
struct Port<'pool, L, P: BufferPool + 'pool, const OUT: usize> {
    /// Bidirectional data link, split into read/write
    /// halves on each poll cycle.
    link: L,
    /// Input buffer for packets read from this link.
    buf: P::Buf<'pool>,
    /// Output queue for packets waiting to be forwarded
    /// through this link.
    out: RingBuffer<OUT>,
    /// Staging buffer: the front of `out` is copied here
    /// before starting a write future, so the ring stays
    /// free for new enqueues while the write is in flight.
    stage: P::Buf<'pool>,
}

impl<'pool, L, P: BufferPool + 'pool, const OUT: usize> Port<'pool, L, P, OUT> {
    fn new(link: L, pool: &'pool P, mtu: usize) -> Result<Self, P::Error> {
        Ok(Self {
            link,
            buf: pool.alloc_bytes(mtu)?,
            out: RingBuffer::new(),
            stage: pool.alloc_bytes(mtu)?,
        })
    }
}

/// A SpacePacket router with per-direction output queues.
///
/// ISL directions (N/S/E/W) share link type `N`. The ground
/// link has an independent type `G` since it may use a
/// different physical layer. Both must implement [`Datalink`]
/// so the router can split them into read/write halves for
/// concurrent I/O.
///
/// Buffers come from a [`BufferPool`] supplied at construction;
/// the pool's lifetime `'pool` outlives the router. This keeps
/// the per-Port input/staging buffers off the stack and gives
/// the whole router a shared, fallible memory budget.
///
/// The `read()` loop uses `select_biased!` to poll all 5
/// readers and all 5 writers concurrently, eliminating
/// head-of-line blocking and deadlock.
pub struct Router<'pool, N, G, A, C, P: BufferPool + 'pool, const OUT: usize = 2048> {
    north: Port<'pool, N, P, OUT>,
    south: Port<'pool, N, P, OUT>,
    east: Port<'pool, N, P, OUT>,
    west: Port<'pool, N, P, OUT>,
    ground: Port<'pool, G, P, OUT>,
    address: Address,
    algorithm: A,
    clock: C,
    /// Diagnostic counters. Mutated each `read()` iteration; the
    /// router app can read+clear them periodically to detect spins.
    diag: core::cell::Cell<RouterDiag>,
}

/// Per-event counters for the router's `read()` select loop. Reset
/// each time `take_diag()` is called.
#[derive(Default, Debug, Clone, Copy)]
pub struct RouterDiag {
    /// Total `read()` iterations.
    pub iterations: u64,
    /// ISL reads indexed by direction (N, S, E, W).
    pub isl_reads: [u64; 4],
    /// ISL writes indexed by direction (N, S, E, W).
    pub isl_writes: [u64; 4],
    /// Reads from the ground link.
    pub ground_read: u64,
    /// Writes to the ground link.
    pub ground_write: u64,
    /// Last-seen routing decision: (port_read_from, target_raw, hop_code).
    /// hop_code: 0=local, 1=N, 2=S, 3=E, 4=W, 5=Ground. port_read_from: 0=G, 1=N, 2=S, 3=E, 4=W.
    pub last_route: (u8, u16, u8),
}

/// Error from a directional link or from ISL parsing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RouterError<E, GE = E> {
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
    /// Error on the ground link.
    #[error("Ground link error: {0}")]
    Ground(GE),
    /// The caller's buffer is too small for the received packet.
    #[error("buffer too small: need {needed} bytes, got {provided}")]
    BufferTooSmall {
        /// Packet size in bytes.
        needed: usize,
        /// Caller buffer size in bytes.
        provided: usize,
    },
    /// Error parsing the ISL message.
    #[error("ISL message error: {0}")]
    IslMessageError(#[from] isl::routing::packet::IslMessageError),
}

#[bon::bon]
impl<'pool, N, G, A, C, P: BufferPool + 'pool, const OUT: usize> Router<'pool, N, G, A, C, P, OUT>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
{
    #[builder]
    /// Creates a new router with directional links.
    ///
    /// Allocates two MTU-sized buffers per port (input + staging)
    /// from `pool`. Returns an error if the pool cannot satisfy
    /// the ten allocations.
    pub fn new(
        pool: &'pool P,
        mtu: usize,
        north: N,
        south: N,
        east: N,
        west: N,
        ground: G,
        address: Address,
        algorithm: A,
        clock: C,
    ) -> Result<Self, P::Error> {
        Ok(Self {
            north: Port::new(north, pool, mtu)?,
            south: Port::new(south, pool, mtu)?,
            east: Port::new(east, pool, mtu)?,
            west: Port::new(west, pool, mtu)?,
            ground: Port::new(ground, pool, mtu)?,
            diag: core::cell::Cell::new(RouterDiag::default()),
            address,
            algorithm,
            clock,
        })
    }

    /// Returns this router's own address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Read and reset the diagnostic counters. Intended to be polled
    /// at a low rate (e.g. once per second) to surface spin loops.
    pub fn take_diag(&self) -> RouterDiag {
        let snap = self.diag.get();
        self.diag.set(RouterDiag::default());
        snap
    }
}

impl<'pool, N, G, A, C, P, const OUT: usize> NetworkWrite for Router<'pool, N, G, A, C, P, OUT>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
    P: BufferPool + 'pool,
{
    type Error = RouterError<N::WriteError, G::WriteError>;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let packet = IslRoutingTelecommand::parse(data).map_err(RouterError::IslMessageError)?;
        let target = packet.isl_header.target();
        let bytes = packet.as_bytes();
        let next = self
            .algorithm
            .route(Point::from(self.address), target, self.clock.now());
        match next {
            Hop::Isl(Direction::North) => {
                let (_, mut w) = self.north.link.split();
                w.write(bytes).await.map_err(RouterError::North)
            }
            Hop::Isl(Direction::South) => {
                let (_, mut w) = self.south.link.split();
                w.write(bytes).await.map_err(RouterError::South)
            }
            Hop::Isl(Direction::East) => {
                let (_, mut w) = self.east.link.split();
                w.write(bytes).await.map_err(RouterError::East)
            }
            Hop::Isl(Direction::West) => {
                let (_, mut w) = self.west.link.split();
                w.write(bytes).await.map_err(RouterError::West)
            }
            Hop::Ground => {
                let (_, mut w) = self.ground.link.split();
                w.write(bytes).await.map_err(RouterError::Ground)
            }
            Hop::Local => Ok(()),
        }
    }
}

impl<'pool, N, G, A, C, P, const OUT: usize> NetworkRead for Router<'pool, N, G, A, C, P, OUT>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
    P: BufferPool + 'pool,
{
    type Error = RouterError<N::ReadError, G::ReadError>;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            let Self {
                north,
                south,
                east,
                west,
                ground,
                address,
                algorithm,
                clock,
                diag,
            } = self;
            {
                let mut d = diag.get();
                d.iterations = d.iterations.wrapping_add(1);
                diag.set(d);
            }

            // Split each link into read/write halves.
            let (mut nr, mut nw) = north.link.split();
            let (mut sr, mut sw) = south.link.split();
            let (mut er, mut ew) = east.link.split();
            let (mut wr, mut ww) = west.link.split();
            let (mut gr, mut gw) = ground.link.split();

            // Stage output: copy queue front into staging
            // buffer, then start a write future. If the
            // queue is empty, return pending().
            macro_rules! stage {
                ($w:expr, $port:expr) => {
                    match $port.out.front() {
                        Some(data) => {
                            let len = data.len();
                            $port.stage[..len].copy_from_slice(data);
                            Either::Left($w.write(&$port.stage[..len]).fuse())
                        }
                        None => Either::Right(futures::future::pending()),
                    }
                };
            }

            enum Event<RE, GRE> {
                IslRead(Result<usize, RE>, Direction),
                GroundRead(Result<usize, GRE>),
                IslWrite(Direction),
                GroundWrite,
            }

            // Route a received packet: return if local,
            // enqueue for forwarding otherwise.
            macro_rules! route_packet {
                ($buf:expr, $len:expr, $err_variant:ident, $from_port:expr) => {{
                    let buf = &$buf[..$len];
                    let Ok(packet) = IslRoutingTelecommand::parse(buf) else {
                        continue;
                    };
                    let target = packet.isl_header.target();
                    let next = algorithm.route(
                        Point::from(*address),
                        target,
                        clock.now(),
                    );
                    {
                        let mut d = diag.get();
                        let target_raw: u16 = match target {
                            crate::network::isl::address::Address::Ground { station } => {
                                (station as u16) | 0x8000
                            }
                            crate::network::isl::address::Address::Satellite(p) => {
                                ((p.orb as u16) << 8) | (p.sat as u16)
                            }
                        };
                        let hop_code: u8 = match next {
                            Hop::Local => 0,
                            Hop::Isl(Direction::North) => 1,
                            Hop::Isl(Direction::South) => 2,
                            Hop::Isl(Direction::East) => 3,
                            Hop::Isl(Direction::West) => 4,
                            Hop::Ground => 5,
                        };
                        d.last_route = ($from_port, target_raw, hop_code);
                        diag.set(d);
                    }

                    if next == Hop::Local {
                        if buffer.len() < $len {
                            return Err(RouterError::BufferTooSmall {
                                needed: $len,
                                provided: buffer.len(),
                            });
                        }
                        buffer[..$len].copy_from_slice(buf);
                        return Ok($len);
                    }

                    match next {
                        Hop::Isl(Direction::North) => {
                            north.out.push(buf);
                        }
                        Hop::Isl(Direction::South) => {
                            south.out.push(buf);
                        }
                        Hop::Isl(Direction::East) => {
                            east.out.push(buf);
                        }
                        Hop::Isl(Direction::West) => {
                            west.out.push(buf);
                        }
                        Hop::Ground => {
                            ground.out.push(buf);
                        }
                        Hop::Local => {}
                    }
                }};
            }

            let event = {
                let nw = stage!(nw, north);
                let sw = stage!(sw, south);
                let ew = stage!(ew, east);
                let ww = stage!(ww, west);
                let gw = stage!(gw, ground);

                let nr = nr.read(&mut north.buf[..]).fuse();
                let sr = sr.read(&mut south.buf[..]).fuse();
                let er = er.read(&mut east.buf[..]).fuse();
                let wr = wr.read(&mut west.buf[..]).fuse();
                let gr = gr.read(&mut ground.buf[..]).fuse();

                pin_utils::pin_mut!(nr, sr, er, wr, gr, nw, sw, ew, ww, gw);

                // Writes before reads: drain output queues
                // before accepting new packets. Reads can
                // wait in the link's own buffer; this prevents
                // write starvation under heavy load.
                futures::select_biased! {
                    _ = nw => Event::IslWrite(Direction::North),
                    _ = sw => Event::IslWrite(Direction::South),
                    _ = ew => Event::IslWrite(Direction::East),
                    _ = ww => Event::IslWrite(Direction::West),
                    _ = gw => Event::GroundWrite,
                    r = nr => Event::IslRead(r, Direction::North),
                    r = sr => Event::IslRead(r, Direction::South),
                    r = er => Event::IslRead(r, Direction::East),
                    r = wr => Event::IslRead(r, Direction::West),
                    r = gr => Event::GroundRead(r),
                }
            };

            {
                let mut d = diag.get();
                match event {
                    Event::IslWrite(Direction::North) => d.isl_writes[0] = d.isl_writes[0].wrapping_add(1),
                    Event::IslWrite(Direction::South) => d.isl_writes[1] = d.isl_writes[1].wrapping_add(1),
                    Event::IslWrite(Direction::East) => d.isl_writes[2] = d.isl_writes[2].wrapping_add(1),
                    Event::IslWrite(Direction::West) => d.isl_writes[3] = d.isl_writes[3].wrapping_add(1),
                    Event::GroundWrite => d.ground_write = d.ground_write.wrapping_add(1),
                    Event::IslRead(_, Direction::North) => d.isl_reads[0] = d.isl_reads[0].wrapping_add(1),
                    Event::IslRead(_, Direction::South) => d.isl_reads[1] = d.isl_reads[1].wrapping_add(1),
                    Event::IslRead(_, Direction::East) => d.isl_reads[2] = d.isl_reads[2].wrapping_add(1),
                    Event::IslRead(_, Direction::West) => d.isl_reads[3] = d.isl_reads[3].wrapping_add(1),
                    Event::GroundRead(_) => d.ground_read = d.ground_read.wrapping_add(1),
                }
                diag.set(d);
            }
            match event {
                Event::IslWrite(dir) => match dir {
                    Direction::North => north.out.pop(),
                    Direction::South => south.out.pop(),
                    Direction::East => east.out.pop(),
                    Direction::West => west.out.pop(),
                },
                Event::GroundWrite => ground.out.pop(),
                Event::GroundRead(result) => {
                    let len = result.map_err(RouterError::Ground)?;
                    route_packet!(ground.buf, len, Ground, 0u8);
                }
                Event::IslRead(result, dir) => {
                    let (buf, len, from_port) = match dir {
                        Direction::North => (&north.buf, result.map_err(RouterError::North)?, 1u8),
                        Direction::South => (&south.buf, result.map_err(RouterError::South)?, 2u8),
                        Direction::East => (&east.buf, result.map_err(RouterError::East)?, 3u8),
                        Direction::West => (&west.buf, result.map_err(RouterError::West)?, 4u8),
                    };
                    route_packet!(buf, len, Isl, from_port);
                }
            }
        }
    }
}
