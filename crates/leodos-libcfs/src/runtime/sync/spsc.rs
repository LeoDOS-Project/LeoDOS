//! An asynchronous, single-producer, single-producer channel for `no_std` environments.
//!
//! This channel is a thin async wrapper around `heapless::spsc::Queue`.

use core::task::{Context, Poll};
use heapless::spsc::{Consumer, Producer, Queue};

/// The sending half of a channel.
pub struct Sender<'a, T> {
    producer: Producer<'a, T>,
}

/// The receiving half of a channel.
pub struct Receiver<'a, T, const N: usize> {
    consumer: Consumer<'a, T>,
}

/// An asynchronous single-producer, single-consumer channel.
pub struct Channel<T, const N: usize> {
    queue: Queue<T, N>,
}

/// Creates a new asynchronous channel.
pub fn channel<'a, T, const N: usize>() -> Channel<T, N> {
    Channel {
        queue: Queue::new(),
    }
}

impl<T, const N: usize> Channel<T, N> {
    /// Splits the channel into its sending and receiving halves.
    pub fn split(&mut self) -> (Sender<'_, T>, Receiver<'_, T, N>) {
        let (producer, consumer) = self.queue.split();
        (Sender { producer }, Receiver { consumer })
    }
}

impl<'a, T> Sender<'a, T> {
    /// Sends a value, waiting until there is capacity.
    pub async fn send(&mut self, value: T) {
        let mut value = Some(value);
        core::future::poll_fn(|_cx: &mut Context<'_>| {
            let val = value.take().unwrap();
            match self.producer.enqueue(val) {
                Ok(()) => Poll::Ready(()),
                Err(val) => {
                    value = Some(val);
                    Poll::Pending
                }
            }
        })
        .await
    }
}

impl<'a, T, const N: usize> Receiver<'a, T, N> {
    /// Receives a value, waiting until one is available.
    pub async fn recv(&mut self) -> T {
        core::future::poll_fn(|_cx: &mut Context<'_>| {
            if let Some(value) = self.consumer.dequeue() {
                Poll::Ready(value)
            } else {
                Poll::Pending
            }
        })
        .await
    }
}
