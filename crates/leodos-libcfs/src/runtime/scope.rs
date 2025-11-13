//! Scope - owns tasks and is itself a Future

use core::future::Future;
use core::pin::Pin;
use core::task::Context;
use core::task::Poll;

use heapless::Vec;

use crate::error::Error;
use crate::runtime::task::Task;

/// A scope that can spawn and manage multiple tasks
#[must_use]
pub struct Scope<'a, const MAX_TASK_SIZE: usize = 512, const MAX_TASKS: usize = 8> {
    tasks: Vec<Task<'a, MAX_TASK_SIZE>, MAX_TASKS>,
}

impl<'a, const MAX_TASK_SIZE: usize, const MAX_TASKS: usize> Scope<'a, MAX_TASK_SIZE, MAX_TASKS> {
    /// Create a new Scope
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    /// Spawn a task into this scope
    pub fn spawn<F>(&mut self, future: F) -> Result<(), Error>
    where
        F: Future<Output = Result<(), Error>> + 'a,
    {
        self.tasks
            .push(Task::new(future))
            .map_err(|_| Error::TaskPoolFull)
    }
}

// Make Scope itself a Future that completes when all tasks complete
impl<'a, const MAX_TASK_SIZE: usize, const MAX_TASKS: usize> Future
    for Scope<'a, MAX_TASK_SIZE, MAX_TASKS>
{
    type Output = Result<(), Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut all_done = true;

        for task in self.tasks.iter_mut() {
            if !task.is_done() {
                match task.poll(cx) {
                    Poll::Ready(Ok(())) => {
                        task.cleanup();
                    }
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(e));
                    }
                    Poll::Pending => {
                        all_done = false;
                    }
                }
            }
        }

        if all_done && !self.tasks.is_empty() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
