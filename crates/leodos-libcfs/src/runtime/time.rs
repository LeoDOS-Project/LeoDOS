//! Asynchronous timer utilities.

use crate::cfe::time::SysTime;
use crate::cfe::duration::Duration;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

/// Waits for a specified amount of time to pass.
///
/// This function is an async version of `std::thread::sleep`. It creates a future
/// that will complete after the given duration has elapsed, without blocking the
/// cFS task.
pub fn sleep(duration: Duration) -> Sleep {
    Sleep {
        deadline: SysTime::now() + SysTime::from(duration),
    }
}

/// A future that completes at a specified time.
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Sleep {
    deadline: SysTime,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Since our runtime polls continuously, we just need to check the current
        // mission time against our deadline on each poll.
        if SysTime::now() >= self.deadline {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
