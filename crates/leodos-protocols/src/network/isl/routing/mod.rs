/// Routing algorithm trait and implementations.
pub mod algorithm;
/// Local in-process channel between router and application.
pub mod local;
/// ISL routable packet definitions and builders.
pub mod packet;
/// Standalone router service with driver/client split.
pub mod service;

use futures::FutureExt as _;
use zerocopy::IntoBytes as _;

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
use crate::network::isl::torus::Torus;

/// A SpacePacket router.
pub struct Router<N, S, E, W, G, R, const MTU: usize = 1024> {
    // Physical links (Sat-to-Sat)
    north: N,
    south: S,
    east: E,
    west: W,
    // Ground link (Sat-to-Earth)
    ground: G,

    // Routing configuration
    address: Address,
    torus: Torus,
    algorithm: R,

    // Link buffers
    north_buf: [u8; MTU],
    south_buf: [u8; MTU],
    east_buf: [u8; MTU],
    west_buf: [u8; MTU],
    ground_buf: [u8; MTU],
}

/// Error from a specific directional link or from ISL parsing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RouterError<N, S, E, W, G> {
    /// Error on the north link.
    #[error("North link error: {0}")]
    North(N),
    /// Error on the south link.
    #[error("South link error: {0}")]
    South(S),
    /// Error on the east link.
    #[error("East link error: {0}")]
    East(E),
    /// Error on the west link.
    #[error("West link error: {0}")]
    West(W),
    /// Error on the ground link.
    #[error("Ground link error: {0}")]
    Ground(G),
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
impl<N, S, E, W, G, R, const MTU: usize> Router<N, S, E, W, G, R, MTU>
where
    N: DatalinkWrite + DatalinkRead,
    S: DatalinkWrite + DatalinkRead,
    E: DatalinkWrite + DatalinkRead,
    W: DatalinkWrite + DatalinkRead,
    G: DatalinkWrite + DatalinkRead,
    R: RoutingAlgorithm,
{
    #[builder]
    /// Creates a new router with directional links and config.
    pub fn new(
        north: N,
        south: S,
        east: E,
        west: W,
        ground: G,
        address: Address,
        torus: Torus,
        algorithm: R,
    ) -> Self {
        Self {
            north,
            south,
            east,
            west,
            ground,
            address,
            torus,
            algorithm,
            north_buf: [0u8; MTU],
            south_buf: [0u8; MTU],
            east_buf: [0u8; MTU],
            west_buf: [0u8; MTU],
            ground_buf: [0u8; MTU],
        }
    }

    /// Returns this router's own address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Determines the next hop for a given destination.
    pub fn next_hop(&self, destination: Address) -> Direction {
        if matches!(destination, Address::Ground { .. }) {
            return Direction::Ground;
        }
        self.algorithm.route(
            &self.torus,
            Point::from(self.address),
            Point::from(destination),
        )
    }
}

impl<N, S, E, W, G, R, const MTU: usize> NetworkWrite for Router<N, S, E, W, G, R, MTU>
where
    N: DatalinkWrite + DatalinkRead,
    S: DatalinkWrite + DatalinkRead,
    E: DatalinkWrite + DatalinkRead,
    W: DatalinkWrite + DatalinkRead,
    G: DatalinkWrite + DatalinkRead,
    R: RoutingAlgorithm,
{
    type Error = RouterError<
        <N as DatalinkWrite>::Error,
        <S as DatalinkWrite>::Error,
        <E as DatalinkWrite>::Error,
        <W as DatalinkWrite>::Error,
        <G as DatalinkWrite>::Error,
    >;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let packet = IslRoutingTelecommand::parse(data).map_err(RouterError::IslMessageError)?;
        let target = packet.isl_header.target();
        let bytes = packet.as_bytes();
        let next = self.next_hop(target);
        match next {
            Direction::North => self.north.write(bytes).await.map_err(RouterError::North),
            Direction::South => self.south.write(bytes).await.map_err(RouterError::South),
            Direction::East => self.east.write(bytes).await.map_err(RouterError::East),
            Direction::West => self.west.write(bytes).await.map_err(RouterError::West),
            Direction::Ground => self.ground.write(bytes).await.map_err(RouterError::Ground),
            Direction::Local => Ok(()),
        }
    }
}

impl<N, S, E, W, G, R, const MTU: usize> NetworkRead for Router<N, S, E, W, G, R, MTU>
where
    N: DatalinkWrite + DatalinkRead<Error = <N as DatalinkWrite>::Error>,
    S: DatalinkWrite + DatalinkRead<Error = <S as DatalinkWrite>::Error>,
    E: DatalinkWrite + DatalinkRead<Error = <E as DatalinkWrite>::Error>,
    W: DatalinkWrite + DatalinkRead<Error = <W as DatalinkWrite>::Error>,
    G: DatalinkWrite + DatalinkRead<Error = <G as DatalinkWrite>::Error>,
    R: RoutingAlgorithm,
{
    type Error = RouterError<
        <N as DatalinkWrite>::Error,
        <S as DatalinkWrite>::Error,
        <E as DatalinkWrite>::Error,
        <W as DatalinkWrite>::Error,
        <G as DatalinkWrite>::Error,
    >;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            let (len, dir) = {
                let n = self.north.read(&mut self.north_buf).fuse();
                let s = self.south.read(&mut self.south_buf).fuse();
                let e = self.east.read(&mut self.east_buf).fuse();
                let w = self.west.read(&mut self.west_buf).fuse();
                let g = self.ground.read(&mut self.ground_buf).fuse();
                pin_utils::pin_mut!(n, s, e, w, g);
                futures::select_biased! {
                    r = n => r.map(|l| (l, Direction::North)).map_err(RouterError::North),
                    r = s => r.map(|l| (l, Direction::South)).map_err(RouterError::South),
                    r = e => r.map(|l| (l, Direction::East)).map_err(RouterError::East),
                    r = w => r.map(|l| (l, Direction::West)).map_err(RouterError::West),
                    r = g => r.map(|l| (l, Direction::Ground)).map_err(RouterError::Ground),
                }?
            };
            let buf = match dir {
                Direction::North => &self.north_buf[..len],
                Direction::South => &self.south_buf[..len],
                Direction::East => &self.east_buf[..len],
                Direction::West => &self.west_buf[..len],
                Direction::Ground => &self.ground_buf[..len],
                Direction::Local => unreachable!(),
            };

            let Ok(packet) = IslRoutingTelecommand::parse(buf) else {
                continue;
            };
            let next = self.next_hop(packet.isl_header.target());

            if next == Direction::Local {
                if buffer.len() < len {
                    return Err(RouterError::BufferTooSmall {
                        needed: len,
                        provided: buffer.len(),
                    });
                }
                buffer[..len].copy_from_slice(buf);
                return Ok(len);
            }

            match next {
                Direction::North => self.north.write(buf).await.map_err(RouterError::North),
                Direction::South => self.south.write(buf).await.map_err(RouterError::South),
                Direction::East => self.east.write(buf).await.map_err(RouterError::East),
                Direction::West => self.west.write(buf).await.map_err(RouterError::West),
                Direction::Ground => self.ground.write(buf).await.map_err(RouterError::Ground),
                Direction::Local => Ok(()),
            }?
        }
    }
}
