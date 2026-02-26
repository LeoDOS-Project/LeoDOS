//! Defines the ISL Gossip message structure and builders.

pub mod bitmap;
pub mod epoch;
pub mod packet;

use crate::network::isl::address::Address;
use crate::network::isl::gossip::packet::Epoch;
use crate::network::isl::gossip::packet::IslGossipTelecommand;
use crate::network::isl::torus::Direction;
use crate::network::isl::torus::Point;
use crate::network::isl::torus::Torus;
use heapless::Vec;
use zerocopy::network_endian::U16;

pub(crate) const EPOCH_CACHE_SIZE: usize = 128;

/// The state required to process incoming gossip messages, including duplicate detection
/// and routing logic.
pub struct GossipHandler<F>
where
    F: FnMut(&IslGossipTelecommand),
{
    /// A cache of recently seen epochs for duplicate detection.
    epoch_cache: [Epoch; EPOCH_CACHE_SIZE],
    /// The current index in the cache for the next epoch to overwrite.
    epoch_cache_index: u8,
    /// Topology logic for determining neighbor positions.
    torus: Torus,
    /// The address of the node running this handler.
    my_address: Address,
    /// Application-specific logic to handle the gossip payload.
    pub app_logic: F,
}

impl<F> GossipHandler<F>
where
    F: FnMut(&IslGossipTelecommand),
{
    pub fn new(torus: Torus, my_address: Address, app_logic: F) -> Self {
        Self {
            epoch_cache: [Epoch(U16::new(0)); EPOCH_CACHE_SIZE],
            epoch_cache_index: 0,
            torus,
            my_address,
            app_logic,
        }
    }

    pub fn is_duplicate(&mut self, epoch: Epoch) -> bool {
        if self.epoch_cache.contains(&epoch) {
            return true;
        }
        self.epoch_cache[self.epoch_cache_index as usize] = epoch;
        self.epoch_cache_index = (self.epoch_cache_index + 1) % (EPOCH_CACHE_SIZE as u8);
        false
    }

    pub fn process_gossip<'a>(
        &mut self,
        packet: &'a IslGossipTelecommand,
    ) -> Vec<(Direction, &'a IslGossipTelecommand), 4> {
        if self.is_duplicate(packet.gossip_header.epoch) {
            return Vec::new();
        }

        (self.app_logic)(packet);

        self.forward_gossip(packet)
    }

    fn forward_gossip<'a>(
        &self,
        packet: &'a IslGossipTelecommand,
    ) -> Vec<(Direction, &'a IslGossipTelecommand), 4> {
        let header = &packet.gossip_header;
        let from_address = header.from_address();
        let mut gossips = Vec::new();
        for direction in [
            Direction::North,
            Direction::South,
            Direction::East,
            Direction::West,
        ] {
            let my_point = Point::from(self.my_address);
            let neighbor_point = self.torus.neighbor(my_point, direction);
            let to_address = Address::from(neighbor_point);

            if to_address != from_address
                && to_address.is_in_service_area(
                    header.service_area_min,
                    header.service_area_max,
                )
            {
                gossips
                    .push((direction, packet))
                    .expect("Packet list cannot exceed capacity");
            }
        }
        gossips
    }
}
