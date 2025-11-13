pub enum WamrHostError {
    /// The builder exceeded its capacity for native symbols.
    BuilderCapacityExceeded,
}

pub type Result<T> = core::result::Result<T, WamrHostError>;
