//! A dynamic task scope for managing pinned futures.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use heapless::Vec;

use crate::error::CfsError;

/// A scope that holds Pinned mutable references to dyn-futures.
pub struct DynScope<'a, const MAX_TASKS: usize = 8> {
    tasks: Vec<Pin<&'a mut dyn Future<Output = Result<(), CfsError>>>, MAX_TASKS>,
}

impl<'a, const MAX_TASKS: usize> DynScope<'a, MAX_TASKS> {
    /// Create a new DynScope
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Spawn a task into this scope.
    ///
    /// The argument must be a `Pin<&mut F>`. This ensures that the
    /// underlying future has already been safely pinned by the caller.
    pub fn spawn<F>(&mut self, future: Pin<&'a mut F>) -> Result<(), CfsError>
    where
        F: Future<Output = Result<(), CfsError>> + 'a,
    {
        self.tasks.push(future).map_err(|_| CfsError::TaskPoolFull)
    }
}

impl<'a, const MAX_TASKS: usize> Future for DynScope<'a, MAX_TASKS> {
    type Output = Result<(), CfsError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let tasks = &mut self.tasks;

        let mut errors: Option<CfsError> = None;

        let mut i = tasks.len();
        while i > 0 {
            i -= 1;

            let pinned_task = &mut tasks[i];

            match pinned_task.as_mut().poll(cx) {
                Poll::Ready(Ok(())) => {
                    tasks.swap_remove(i);
                }
                Poll::Ready(Err(e)) => {
                    errors = Some(e);
                    break;
                }
                Poll::Pending => {}
            }
        }

        if let Some(e) = errors {
            return Poll::Ready(Err(e));
        }

        if tasks.is_empty() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pin_utils::pin_mut;

    #[test]
    fn test_safe_scope() {
        let s1 = async { Ok(()) };
        let s2 = async { Ok(()) };
        pin_mut!(s1);
        pin_mut!(s2);
        let mut scope = DynScope::<8>::new();
        scope.spawn(s1).unwrap();
        scope.spawn(s2).unwrap();
    }
}
