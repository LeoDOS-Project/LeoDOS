use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::pdu::EntityId;

/// A zero-copy view of the Value of an Entity ID TLV.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvEntityId {
    /// The variable-length entity ID bytes.
    rest: [u8],
}

impl TlvEntityId {
    /// Parses and returns the entity ID from the TLV value.
    pub fn id(&self) -> Result<EntityId, CfdpError> {
        EntityId::from_bytes(&self.rest)
    }
}
