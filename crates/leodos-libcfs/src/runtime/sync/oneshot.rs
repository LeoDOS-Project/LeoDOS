//! An asynchronous, single-message channel for `no_std` environments.

use core::cell::RefCell;
use core::future::Future;
use core::mem;
use core::pin::Pin;
use core::task::{Context, Poll};

/// The internal state of the oneshot channel.
enum State<T> {
    /// The channel is empty, waiting for a value.
    Empty,
    /// The channel is full, holding a value.
    Full(T),
    /// The channel is closed because the Sender or Receiver was dropped.
    Closed,
}

/// The shared core of the oneshot channel.
struct Core<T> {
    state: RefCell<State<T>>,
}

/// A handle to a oneshot channel that owns the shared state.
pub struct Channel<T> {
    core: Core<T>,
}

/// The sending half of a oneshot channel.
pub struct Sender<'a, T> {
    core: &'a Core<T>,
}

/// The receiving half of a oneshot channel.
pub struct Receiver<'a, T> {
    core: &'a Core<T>,
}

/// An error returned by `Sender::send` if the `Receiver` has been dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendError<T>(pub T);

/// An error returned by `Receiver::recv` if the `Sender` has been dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecvError;

/// Creates a new oneshot channel, returning the owning `Channel` handle.
pub fn channel<T>() -> Channel<T> {
    Channel {
        core: Core {
            state: RefCell::new(State::Empty),
        },
    }
}

impl<T> Channel<T> {
    /// Splits the channel into its sending and receiving halves.
    pub fn split(&mut self) -> (Sender<'_, T>, Receiver<'_, T>) {
        (Sender { core: &self.core }, Receiver { core: &self.core })
    }
}

impl<'a, T> Sender<'a, T> {
    /// Attempts to send a value to the receiver.
    ///
    /// This is a synchronous operation; it succeeds or fails immediately.
    ///
    /// # Errors
    ///
    /// Returns `Err(SendError(value))` if the receiver has already been dropped.
    pub fn send(self, value: T) -> Result<(), SendError<T>> {
        // We take ownership of `self` to ensure `send` can only be called once.
        let mut state = self.core.state.borrow_mut();

        match *state {
            State::Empty => {
                *state = State::Full(value);
                Ok(())
            }
            // If it's already full or closed, the receiver must have been dropped.
            _ => Err(SendError(value)),
        }
    }
}

impl<T> Drop for Core<T> {
    fn drop(&mut self) {
        // Ensure state is marked as closed when the channel is finally dropped.
        *self.state.borrow_mut() = State::Closed;
    }
}

impl<'a, T> Drop for Sender<'a, T> {
    fn drop(&mut self) {
        // If the sender is dropped before sending, mark the channel as closed
        // to notify the receiver.
        let mut state = self.core.state.borrow_mut();
        if matches!(*state, State::Empty) {
            *state = State::Closed;
        }
    }
}

impl<'a, T> Receiver<'a, T> {
    /// Waits for a value to be sent on the channel.
    ///
    /// This returns a future that resolves to the value, or an error if the
    /// sender is dropped before sending a value.
    pub async fn recv(self) -> Result<T, RecvError> {
        // A simple future that polls the channel state.
        struct RecvFuture<'a, T> {
            core: &'a Core<T>,
        }

        impl<'a, T> Future for RecvFuture<'a, T> {
            type Output = Result<T, RecvError>;

            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                let mut state = self.core.state.borrow_mut();
                match mem::replace(&mut *state, State::Closed) {
                    State::Full(value) => Poll::Ready(Ok(value)),
                    State::Closed => Poll::Ready(Err(RecvError)),
                    State::Empty => {
                        // Not ready yet, put the state back and pend.
                        *state = State::Empty;
                        Poll::Pending
                    }
                }
            }
        }

        RecvFuture { core: self.core }.await
    }
}
