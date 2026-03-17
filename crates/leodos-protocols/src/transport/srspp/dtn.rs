/// Delay-tolerant delivery for SRSPP.
///
/// When the SRSPP sender detects that a destination is
/// unreachable (e.g., no ground station in LOS), the whole
/// message is written to a [`MessageStore`] instead of entering
/// the SRSPP retransmit buffer. The driver periodically
/// checks reachability via [`Reachable`] and drains stored
/// messages through the normal SRSPP path when contact
/// returns.
use crate::network::isl::address::Address;

// ── Store ───────────────────────────────────────────────

/// Result of a [`MessageStore::write`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreResult {
    /// Message was stored successfully.
    Stored,
    /// Store is full — message was not stored.
    Full,
}

/// Persistent message store for delay-tolerant delivery.
///
/// Stores whole application messages (pre-segmentation) on
/// behalf of the SRSPP sender. Messages are keyed by
/// destination [`Address`] and drained in FIFO order per
/// target.
///
/// Implementations typically use OSAL file I/O
/// (`leodos_libcfs::os::fs`) for persistence. A RAM-backed
/// implementation can be used for testing.
pub trait MessageStore {
    /// Persist a message for later delivery.
    ///
    /// - `target` — destination address (e.g., a ground
    ///   station).
    /// - `data` — raw message bytes (pre-segmentation).
    /// - `ttl_secs` — time-to-live in seconds; 0 = no
    ///   expiry.
    /// - `created_at_secs` — creation timestamp in seconds
    ///   (wrapping).
    fn write(
        &mut self,
        target: Address,
        data: &[u8],
        ttl_secs: u16,
        created_at_secs: u32,
    ) -> StoreResult;

    /// Read and remove the oldest stored message for
    /// `target`. Copies the message into `buf` and returns
    /// its length. Returns `None` if nothing is stored for
    /// that target.
    fn read(&mut self, target: Address, buf: &mut [u8]) -> Option<usize>;

    /// Returns the byte length of the next message for
    /// `target` without removing it. Used by the driver to
    /// check if the message fits in the SRSPP buffer before
    /// reading.
    fn peek_size(&self, target: Address) -> Option<usize>;

    /// Bitmap of targets that have pending messages.
    /// Bit N set = ground station N has at least one
    /// stored message.
    fn pending_targets(&self) -> u16;

    /// Discard all messages whose TTL has expired.
    /// `now_secs` uses the same epoch as `created_at_secs`.
    fn expire(&mut self, now_secs: u32);
}

// ── Reachable ───────────────────────────────────────────

/// Reachability oracle for delay-tolerant delivery.
///
/// The SRSPP sender queries this before transmitting. If
/// the target is unreachable from the origin, the message
/// goes to the [`MessageStore`] instead of the retransmit buffer.
pub trait Reachable {
    /// Returns `true` if `target` is reachable from
    /// `origin` at this moment.
    fn is_reachable(&self, origin: Address, target: Address) -> bool;
}

// ── Discard ─────────────────────────────────────────────

/// Reason a stored message was discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscardReason {
    /// TTL expired before the message could be sent.
    Expired,
    /// The message won't survive until the next contact
    /// window.
    WontSurviveUntilContact,
    /// The store is full and the message could not be
    /// stored.
    StoreFull,
}

/// Callback invoked when a stored message is discarded.
///
/// The default [`SilentDiscard`] does nothing. Apps can
/// implement this to log, count, or persist discards.
pub trait DiscardPolicy {
    /// Called when a message is discarded.
    fn on_discard(&mut self, target: Address, data: &[u8], reason: DiscardReason);
}

// ── Default implementations ─────────────────────────────

/// A no-op store that rejects all writes. Used when DTN
/// is disabled (paired with [`AlwaysReachable`]).
pub struct NoStore;

impl MessageStore for NoStore {
    fn write(
        &mut self,
        _target: Address,
        _data: &[u8],
        _ttl_secs: u16,
        _created_at_secs: u32,
    ) -> StoreResult {
        StoreResult::Full
    }

    fn read(&mut self, _target: Address, _buf: &mut [u8]) -> Option<usize> {
        None
    }

    fn peek_size(&self, _target: Address) -> Option<usize> {
        None
    }

    fn pending_targets(&self) -> u16 {
        0
    }

    fn expire(&mut self, _now_secs: u32) {}
}

/// An oracle that considers all destinations reachable.
/// Used when DTN is disabled.
pub struct AlwaysReachable;

impl Reachable for AlwaysReachable {
    fn is_reachable(&self, _origin: Address, _target: Address) -> bool {
        true
    }
}

/// A discard policy that silently drops messages.
pub struct SilentDiscard;

impl DiscardPolicy for SilentDiscard {
    fn on_discard(&mut self, _target: Address, _data: &[u8], _reason: DiscardReason) {}
}
