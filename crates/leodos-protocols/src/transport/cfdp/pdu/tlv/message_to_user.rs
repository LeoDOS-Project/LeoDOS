use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy view of the Value of a Message to User TLV.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvMessageToUser {
    /// The variable-length message payload bytes.
    rest: [u8],
}

impl TlvMessageToUser {
    /// Returns the message payload bytes.
    pub fn message(&self) -> &[u8] {
        &self.rest
    }
}
