pub mod algorithm;
pub mod local;
pub mod packet;

use futures::FutureExt as _;
use zerocopy::IntoBytes as _;

use crate::datalink::DataLink;
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

pub enum Error<N, S, E, W, G, L> {
    North(N),
    South(S),
    East(E),
    West(W),
    Ground(G),
    Local(L),
    IslMessageError(isl::routing::packet::IslMessageError),
}

#[bon::bon]
impl<N, S, E, W, G, L, R> Router<N, S, E, W, G, L, R>
where
    N: DataLink,
    S: DataLink,
    E: DataLink,
    W: DataLink,
    G: DataLink,
    L: DataLink,
    R: RoutingAlgorithm,
{
    #[builder]
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

    pub async fn recv(
        &mut self,
        north_buffer: &mut [u8],
        south_buffer: &mut [u8],
        east_buffer: &mut [u8],
        west_buffer: &mut [u8],
        ground_buffer: &mut [u8],
        local_buffer: &mut [u8],
    ) -> (
        Result<usize, Error<N::Error, S::Error, E::Error, W::Error, G::Error, L::Error>>,
        Direction,
    ) {
        let n = self.north.recv(north_buffer).fuse();
        let s = self.south.recv(south_buffer).fuse();
        let e = self.east.recv(east_buffer).fuse();
        let w = self.west.recv(west_buffer).fuse();
        let g = self.ground.recv(ground_buffer).fuse();
        let l = self.local.recv(local_buffer).fuse();
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
    ) -> Result<(), Error<N::Error, S::Error, E::Error, W::Error, G::Error, L::Error>> {
        let packet =
            IslRoutingTelecommand::parse(&buffer[..len]).map_err(Error::IslMessageError)?;
        let target = packet.isl_header.target();
        let packet = packet.as_bytes();
        match self.next_hop(target) {
            Direction::North => self.north.send(&packet).await.map_err(Error::North)?,
            Direction::South => self.south.send(&packet).await.map_err(Error::South)?,
            Direction::East => self.east.send(&packet).await.map_err(Error::East)?,
            Direction::West => self.west.send(&packet).await.map_err(Error::West)?,
            Direction::Ground => self.ground.send(&packet).await.map_err(Error::Ground)?,
            Direction::Local => self.local.send(&packet).await.map_err(Error::Local)?,
        }
        Ok(())
    }
}
