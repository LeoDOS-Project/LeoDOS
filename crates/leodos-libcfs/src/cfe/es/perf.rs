//! Safe wrapper for CFE Performance Logging.

use crate::ffi;

/// A performance marker that logs entry and exit points for performance measurement.
///
/// Automatically starts logging on creation and stops logging when dropped.
pub struct PerfMarker {
    id: u32,
}

impl PerfMarker {
    /// Creates a new performance marker with the given ID and logs an "entry" event.
    ///
    /// # Arguments
    /// * `id`: A numeric identifier for the performance event.
    ///
    /// # C-API Mapping
    /// This calls `CFE_ES_PerfLogAdd(id, 0)`.
    pub fn new(id: u32) -> Self {
        unsafe {
            ffi::CFE_ES_PerfLogAdd(id, 0);
        }
        Self { id }
    }
}

impl Drop for PerfMarker {
    /// Logs an "exit" event when the marker goes out of scope.
    ///
    /// # C-API Mapping
    /// This calls `CFE_ES_PerfLogAdd(self.id, 1)`.
    fn drop(&mut self) {
        unsafe {
            ffi::CFE_ES_PerfLogAdd(self.id, 1);
        }
    }
}
