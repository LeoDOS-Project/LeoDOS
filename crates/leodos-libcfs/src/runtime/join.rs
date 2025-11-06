//! Structured concurrency primitives.
//!
//! This module provides combinators like `join` that allow running multiple
//! futures concurrently within a specific lifetime scope, making it possible
//! to safely borrow stack variables in async tasks.

use core::future::Future;
use core::task::{Context, Poll};

use pin_utils::pin_mut;

/// Polls two futures concurrently until both complete, returning their results.
pub async fn join<'a, F1, F2>(future1: F1, future2: F2) -> (F1::Output, F2::Output)
where
    F1: Future + 'a,
    F2: Future + 'a,
{
    pin_mut!(future1);
    pin_mut!(future2);

    let mut result1: Option<F1::Output> = None;
    let mut result2: Option<F2::Output> = None;

    core::future::poll_fn(|cx: &mut Context<'_>| {
        if result1.is_none() {
            if let Poll::Ready(output) = future1.as_mut().poll(cx) {
                result1 = Some(output);
            }
        }

        if result2.is_none() {
            if let Poll::Ready(output) = future2.as_mut().poll(cx) {
                result2 = Some(output);
            }
        }

        if let (Some(res1), Some(res2)) = (result1.take(), result2.take()) {
            Poll::Ready((res1, res2))
        } else {
            Poll::Pending
        }
    })
    .await
}
