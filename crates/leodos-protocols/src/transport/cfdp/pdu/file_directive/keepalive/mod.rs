/// Keep Alive PDU for large file transactions (64-bit progress).
pub mod large;
/// Keep Alive PDU for small file transactions (32-bit progress).
pub mod small;

/// A parsed Keep Alive PDU, dispatching between small and large file variants.
#[derive(Debug)]
pub enum KeepAlivePdu<'a> {
    /// Keep Alive PDU for small file transactions (32-bit progress).
    Small(&'a small::KeepAlivePduSmall),
    /// Keep Alive PDU for large file transactions (64-bit progress).
    Large(&'a large::KeepAlivePduLarge),
}

impl<'a> KeepAlivePdu<'a> {
    /// Returns the receiver's progress as a byte count.
    pub fn progress(&self) -> u64 {
        match self {
            KeepAlivePdu::Small(small) => small.progress() as u64,
            KeepAlivePdu::Large(large) => large.progress(),
        }
    }
}
