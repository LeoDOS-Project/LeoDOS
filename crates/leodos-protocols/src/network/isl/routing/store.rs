/// Backend for persistent packet storage (store-and-forward).
///
/// When the Router cannot deliver a ground-destined packet
/// (ground station not in LOS), it stores the packet via
/// this trait. When the ground link becomes available, the
/// Router drains stored packets.
///
/// Implementations:
/// - Disk-based (cFS apps, using OSAL file APIs)
/// - RAM-based (tests)
pub trait PacketStore {
    /// Store a packet for later delivery.
    fn store(&mut self, data: &[u8]) -> bool;

    /// Read the next stored packet into `buf`.
    /// Returns the packet length, or `None` if empty.
    fn next(&mut self, buf: &mut [u8]) -> Option<usize>;

    /// Number of packets currently stored.
    fn len(&self) -> usize;

    /// Whether the store is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A no-op store that drops all packets. Used when
/// store-and-forward is disabled.
pub struct NoStore;

impl PacketStore for NoStore {
    fn store(&mut self, _data: &[u8]) -> bool {
        false
    }

    fn next(&mut self, _buf: &mut [u8]) -> Option<usize> {
        None
    }

    fn len(&self) -> usize {
        0
    }
}
