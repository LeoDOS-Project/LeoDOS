/// Routing algorithm trait and implementations.
pub mod algorithm;
/// Local in-process channel between router and application.
pub mod local;
/// ISL routable packet definitions and builders.
pub mod packet;
/// Standalone router service with driver/client split.
pub mod service;

use futures::FutureExt as _;
use futures::future::Either;
use zerocopy::IntoBytes as _;

use crate::datalink::Datalink;
use crate::datalink::DatalinkRead;
use crate::datalink::DatalinkWrite;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl;
use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::packet::IslRoutingTelecommand;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::utils::clock::Clock;
use crate::utils::ringbuf::RingBuffer;

/// Per-direction link state: the link itself plus its
/// input buffer, output queue, and staging buffer.
struct Port<L, const MTU: usize, const OUT: usize> {
    /// Bidirectional data link, split into read/write
    /// halves on each poll cycle.
    link: L,
    /// Input buffer for packets read from this link.
    buf: [u8; MTU],
    /// Output queue for packets waiting to be forwarded
    /// through this link.
    out: RingBuffer<OUT>,
    /// Staging buffer: the front of `out` is copied here
    /// before starting a write future, so the ring stays
    /// free for new enqueues while the write is in flight.
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

/// A SpacePacket router with per-direction output queues.
///
/// ISL directions (N/S/E/W) share link type `N`. The ground
/// link has an independent type `G` since it may use a
/// different physical layer. Both must implement [`Datalink`]
/// so the router can split them into read/write halves for
/// concurrent I/O.
///
/// The `read()` loop uses `select_biased!` to poll all 5
/// readers and all 5 writers concurrently, eliminating
/// head-of-line blocking and deadlock.
pub struct Router<N, G, A, C, const MTU: usize = 1024, const OUT: usize = 2048> {
    north: Port<N, MTU, OUT>,
    south: Port<N, MTU, OUT>,
    east: Port<N, MTU, OUT>,
    west: Port<N, MTU, OUT>,
    ground: Port<G, MTU, OUT>,

    address: Address,
    algorithm: A,
    clock: C,
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
impl<N, G, A, C, const MTU: usize, const OUT: usize> Router<N, G, A, C, MTU, OUT>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
{
    #[builder]
    /// Creates a new router with directional links.
    pub fn new(
        north: N,
        south: N,
        east: N,
        west: N,
        ground: G,
        address: Address,
        algorithm: A,
        clock: C,
    ) -> Self {
        Self {
            north: Port::new(north),
            south: Port::new(south),
            east: Port::new(east),
            west: Port::new(west),
            ground: Port::new(ground),
            address,
            algorithm,
            clock,
        }
    }

    /// Returns this router's own address.
    pub fn address(&self) -> Address {
        self.address
    }
}

impl<N, G, A, C, const MTU: usize, const OUT: usize> NetworkWrite for Router<N, G, A, C, MTU, OUT>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
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
            Direction::North => {
                let (_, mut w) = self.north.link.split();
                w.write(bytes).await.map_err(RouterError::North)
            }
            Direction::South => {
                let (_, mut w) = self.south.link.split();
                w.write(bytes).await.map_err(RouterError::South)
            }
            Direction::East => {
                let (_, mut w) = self.east.link.split();
                w.write(bytes).await.map_err(RouterError::East)
            }
            Direction::West => {
                let (_, mut w) = self.west.link.split();
                w.write(bytes).await.map_err(RouterError::West)
            }
            Direction::Ground => {
                let (_, mut w) = self.ground.link.split();
                w.write(bytes).await.map_err(RouterError::Ground)
            }
            Direction::Local => Ok(()),
        }
    }
}

impl<N, G, A, C, const MTU: usize, const OUT: usize> NetworkRead for Router<N, G, A, C, MTU, OUT>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
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
            } = self;

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
                Read(Result<usize, RE>, Direction),
                GroundRead(Result<usize, GRE>),
                WriteComplete(Direction),
            }

            // Route a received packet: return if local,
            // enqueue for forwarding otherwise.
            macro_rules! route_packet {
                ($buf:expr, $len:expr, $err_variant:ident) => {{
                    let buf = &$buf[..$len];
                    let Ok(packet) = IslRoutingTelecommand::parse(buf) else {
                        continue;
                    };
                    let next = algorithm.route(
                        Point::from(*address),
                        packet.isl_header.target(),
                        clock.now(),
                    );

                    if next == Direction::Local {
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
                        Direction::North => {
                            north.out.push(buf);
                        }
                        Direction::South => {
                            south.out.push(buf);
                        }
                        Direction::East => {
                            east.out.push(buf);
                        }
                        Direction::West => {
                            west.out.push(buf);
                        }
                        Direction::Ground => {
                            ground.out.push(buf);
                        }
                        Direction::Local => {}
                    }
                }};
            }

            let event = {
                let nw = stage!(nw, north);
                let sw = stage!(sw, south);
                let ew = stage!(ew, east);
                let ww = stage!(ww, west);
                let gw = stage!(gw, ground);

                let nr = nr.read(&mut north.buf).fuse();
                let sr = sr.read(&mut south.buf).fuse();
                let er = er.read(&mut east.buf).fuse();
                let wr = wr.read(&mut west.buf).fuse();
                let gr = gr.read(&mut ground.buf).fuse();

                pin_utils::pin_mut!(nr, sr, er, wr, gr, nw, sw, ew, ww, gw);

                futures::select_biased! {
                    r = nr => Event::Read(r, Direction::North),
                    r = sr => Event::Read(r, Direction::South),
                    r = er => Event::Read(r, Direction::East),
                    r = wr => Event::Read(r, Direction::West),
                    r = gr => Event::GroundRead(r),
                    _ = nw => Event::WriteComplete(Direction::North),
                    _ = sw => Event::WriteComplete(Direction::South),
                    _ = ew => Event::WriteComplete(Direction::East),
                    _ = ww => Event::WriteComplete(Direction::West),
                    _ = gw => Event::WriteComplete(Direction::Ground),
                }
            };

            match event {
                Event::WriteComplete(dir) => match dir {
                    Direction::North => north.out.pop(),
                    Direction::South => south.out.pop(),
                    Direction::East => east.out.pop(),
                    Direction::West => west.out.pop(),
                    Direction::Ground => ground.out.pop(),
                    Direction::Local => {}
                },
                Event::GroundRead(result) => {
                    let len = result.map_err(RouterError::Ground)?;
                    route_packet!(ground.buf, len, Ground);
                }
                Event::Read(result, dir) => {
                    let (buf, len) = match dir {
                        Direction::North => (&north.buf, result.map_err(RouterError::North)?),
                        Direction::South => (&south.buf, result.map_err(RouterError::South)?),
                        Direction::East => (&east.buf, result.map_err(RouterError::East)?),
                        Direction::West => (&west.buf, result.map_err(RouterError::West)?),
                        _ => unreachable!(),
                    };
                    route_packet!(buf, len, Isl);
                }
            }
        }
    }
}
