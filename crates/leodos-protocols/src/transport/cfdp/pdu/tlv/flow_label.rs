use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy view of the Value of a Flow Label TLV.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TlvFlowLabel {
    /// The variable-length flow label bytes.
    rest: [u8],
}

impl TlvFlowLabel {
    /// Returns the flow label bytes.
    pub fn label(&self) -> &[u8] {
        &self.rest
    }
}
