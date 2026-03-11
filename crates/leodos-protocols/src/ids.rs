/// Protocol identifier newtypes for type-safe ID handling.
///
/// Each type wraps `u32`. Validation (bit-width checks) is
/// performed at point of use, not at construction.

/// Spacecraft Identifier.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[repr(transparent)]
pub struct Scid(u32);

impl Scid {
    /// Creates a new spacecraft ID.
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw value.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl core::fmt::Display for Scid {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Virtual Channel Identifier.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[repr(transparent)]
pub struct Vcid(u32);

impl Vcid {
    /// Creates a new virtual channel ID.
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw value.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl core::fmt::Display for Vcid {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}
