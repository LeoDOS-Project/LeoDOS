use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy view of the Value of a Flow Label TLV.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvFlowLabel {
    rest: [u8],
}

impl TlvFlowLabel {
    pub fn label(&self) -> &[u8] {
        &self.rest
    }
}
