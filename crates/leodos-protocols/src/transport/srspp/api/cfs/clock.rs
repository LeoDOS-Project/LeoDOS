//! Time-source abstraction for the SRSPP endpoint.
//!
//! [`Clock`] decouples [`SrsppEndpoint`](super::endpoint::SrsppEndpoint)
//! from `leodos_libcfs::cfe::time::SysTime::now()` and the cFE
//! runtime's `sleep`. Production code passes [`CfeClock`]; tests pass
//! a mock that returns scripted values without touching cFE FFI.
//!
//! Two shapes of test mock are useful:
//! - **Immediate-yield clock**: `now()` returns a fixed value, `sleep`
//!   returns `async {}`. Sufficient for happy-path tests that send and
//!   receive without exercising retransmission timing.
//! - **Virtual-time clock**: tests advance an internal cell and tick
//!   sleep wakeups manually. Needed for retransmit / RTO tests.
//!
//! Only the first is needed for the initial round of endpoint tests.

use core::future::Future;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;

/// Source of "current time" and "wait until later" used by the SRSPP
/// run loop and its retransmission timers.
pub trait Clock {
    /// The current time as the SRSPP retransmit timer expects it.
    fn now(&self) -> SysTime;

    /// Suspend until `duration` has elapsed (or longer — the run loop
    /// tolerates spurious wakes).
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()>;
}

/// Production [`Clock`] backed by cFE's `CFE_TIME_GetTime` and the
/// `leodos_libcfs::runtime::time::sleep` future.
#[derive(Debug, Copy, Clone, Default)]
pub struct CfeClock;

impl Clock for CfeClock {
    fn now(&self) -> SysTime {
        SysTime::now()
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> {
        leodos_libcfs::runtime::time::sleep(duration)
    }
}
