pub mod pipe;
pub mod udp;

use leodos_libcfs::error::Error as CfsError;

/// Errors from CFS data link operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CfsLinkError {
    /// An error from the CFS runtime.
    #[error("CFS error: {0}")]
    Cfs(#[from] CfsError),
    /// The provided buffer is too small.
    #[error("buffer too small: need {required}, have {available}")]
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual buffer size available.
        available: usize,
    },
}
