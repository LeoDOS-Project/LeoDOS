//! A simple implementation of `select` for futures.

use core::future;
use core::future::Future;
use core::task::Context;
use core::task::Poll;

use pin_utils::pin_mut;

/// An enum returned by `select` to indicate which future completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Either<A, B> {
    /// The first future completed.
    Left(A),
    /// The second future completed.
    Right(B),
}

/// Waits for either of two futures to complete.
///
/// This function polls both futures concurrently and returns the result of the
/// first one that finishes. The future that did not complete is dropped.
pub async fn select_either<'a, F1, F2>(future1: F1, future2: F2) -> Either<F1::Output, F2::Output>
where
    F1: Future + 'a,
    F2: Future + 'a,
{
    // Pin both futures to the stack.
    pin_mut!(future1);
    pin_mut!(future2);

    future::poll_fn(|cx: &mut Context<'_>| {
        if let Poll::Ready(output) = future1.as_mut().poll(cx) {
            return Poll::Ready(Either::Left(output));
        }

        if let Poll::Ready(output) = future2.as_mut().poll(cx) {
            return Poll::Ready(Either::Right(output));
        }

        Poll::Pending
    })
    .await
}
