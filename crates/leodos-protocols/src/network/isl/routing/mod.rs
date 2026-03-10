/// Routing algorithm trait and implementations.
pub mod algorithm;
/// Local in-process channel between router and application.
pub mod local;
/// ISL routable packet definitions and builders.
pub mod packet;

use futures::FutureExt as _;
use zerocopy::IntoBytes as _;

use crate::datalink::{DataLinkReader, DataLinkWriter};
use crate::network::isl;
use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::packet::IslRoutingTelecommand;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;

/// The central packet router.
/// It moves packets between physical interfaces based on the ISL Header.
pub struct Router<N, S, E, W, G, L, R> {
    // Physical Links (Sat-to-Sat)
    north: N,
    south: S,
    east: E,
    west: W,
    // Ground Link (Sat-to-Earth)
    ground: G,
    // Local Link (Network-to-App)
    local: L,

    // Routing Configuration
    address: Address,
    torus: Torus,
    algorithm: R,
}

/// Error from a specific directional link or from ISL message parsing.
#[derive(Debug, thiserror::Error)]
pub enum Error<N, S, E, W, G, L> {
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
    /// Error on the local link.
    #[error("Local link error: {0}")]
    Local(L),
    /// Error parsing the ISL message.
    #[error("ISL message error: {0}")]
    IslMessageError(#[from] isl::routing::packet::IslMessageError),
}

#[bon::bon]
impl<N, S, E, W, G, L, R> Router<N, S, E, W, G, L, R>
where
    N: DataLinkWriter + DataLinkReader<Error = <N as DataLinkWriter>::Error>,
    S: DataLinkWriter + DataLinkReader<Error = <S as DataLinkWriter>::Error>,
    E: DataLinkWriter + DataLinkReader<Error = <E as DataLinkWriter>::Error>,
    W: DataLinkWriter + DataLinkReader<Error = <W as DataLinkWriter>::Error>,
    G: DataLinkWriter + DataLinkReader<Error = <G as DataLinkWriter>::Error>,
    L: DataLinkWriter + DataLinkReader<Error = <L as DataLinkWriter>::Error>,
    R: RoutingAlgorithm,
{
    #[builder]
    /// Creates a new router with all directional links and routing config.
    pub fn new(
        north: N,
        south: S,
        east: E,
        west: W,
        ground: G,
        local: L,
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
            local,
            address,
            torus,
            algorithm,
        }
    }

    /// Returns this router's own address.
    pub fn address(&self) -> Address {
        self.address
    }

    /// Determines the next hop for a given destination address.
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

    /// Waits for a packet on any link and returns its length and direction.
    pub async fn recv(
        &mut self,
        north_buffer: &mut [u8],
        south_buffer: &mut [u8],
        east_buffer: &mut [u8],
        west_buffer: &mut [u8],
        ground_buffer: &mut [u8],
        local_buffer: &mut [u8],
    ) -> (
        Result<
            usize,
            Error<
                <N as DataLinkWriter>::Error,
                <S as DataLinkWriter>::Error,
                <E as DataLinkWriter>::Error,
                <W as DataLinkWriter>::Error,
                <G as DataLinkWriter>::Error,
                <L as DataLinkWriter>::Error,
            >,
        >,
        Direction,
    ) {
        let n = self.north.read(north_buffer).fuse();
        let s = self.south.read(south_buffer).fuse();
        let e = self.east.read(east_buffer).fuse();
        let w = self.west.read(west_buffer).fuse();
        let g = self.ground.read(ground_buffer).fuse();
        let l = self.local.read(local_buffer).fuse();
        pin_utils::pin_mut!(n, s, e, w, g, l);
        futures::select_biased! {
            len = n => (len.map_err(Error::North), Direction::North),
            len = s => (len.map_err(Error::South), Direction::South),
            len = e => (len.map_err(Error::East), Direction::East),
            len = w => (len.map_err(Error::West), Direction::West),
            len = g => (len.map_err(Error::Ground), Direction::Ground),
            len = l => (len.map_err(Error::Local), Direction::Local),
        }
    }

    /// The main loop. Polls all sources and routes packets.
    pub async fn run(&mut self) -> ! {
        let mut north_buffer = [0u8; 1024];
        let mut south_buffer = [0u8; 1024];
        let mut east_buffer = [0u8; 1024];
        let mut west_buffer = [0u8; 1024];
        let mut ground_buffer = [0u8; 1024];
        let mut local_buffer = [0u8; 1024];
        loop {
            let (len_result, direction) = self
                .recv(
                    &mut north_buffer,
                    &mut south_buffer,
                    &mut east_buffer,
                    &mut west_buffer,
                    &mut ground_buffer,
                    &mut local_buffer,
                )
                .await;

            let Ok(len) = len_result else {
                continue;
            };

            let buffer = match direction {
                Direction::North => &north_buffer,
                Direction::South => &south_buffer,
                Direction::East => &east_buffer,
                Direction::West => &west_buffer,
                Direction::Ground => &ground_buffer,
                Direction::Local => &local_buffer,
            };

            let _ = self.route_packet(buffer, len).await;
        }
    }

    async fn route_packet(
        &mut self,
        buffer: &[u8],
        len: usize,
    ) -> Result<
        (),
        Error<
            <N as DataLinkWriter>::Error,
            <S as DataLinkWriter>::Error,
            <E as DataLinkWriter>::Error,
            <W as DataLinkWriter>::Error,
            <G as DataLinkWriter>::Error,
            <L as DataLinkWriter>::Error,
        >,
    > {
        let packet =
            IslRoutingTelecommand::parse(&buffer[..len]).map_err(Error::IslMessageError)?;
        let target = packet.isl_header.target();
        let packet = packet.as_bytes();
        match self.next_hop(target) {
            Direction::North => self.north.write(&packet).await.map_err(Error::North)?,
            Direction::South => self.south.write(&packet).await.map_err(Error::South)?,
            Direction::East => self.east.write(&packet).await.map_err(Error::East)?,
            Direction::West => self.west.write(&packet).await.map_err(Error::West)?,
            Direction::Ground => self.ground.write(&packet).await.map_err(Error::Ground)?,
            Direction::Local => self.local.write(&packet).await.map_err(Error::Local)?,
        }
        Ok(())
    }
}
