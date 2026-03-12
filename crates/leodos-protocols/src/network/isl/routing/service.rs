//! Standalone router service with driver/client split.
//!
//! Wraps a [`Router`] and a [`LocalChannel`] for use cases
//! where the router runs as an independent task (without
//! SRSPP or another transport driving it directly).
//!
//! # Example
//!
//! ```ignore
//! let channel = LocalChannel::new();
//! let (client, mut driver) = RouterService::new(router, &channel);
//! join(app(client), driver.run()).await;
//! ```

use futures::FutureExt as _;

use crate::datalink::{Datalink, DatalinkRead, DatalinkWrite};
use crate::network::isl::routing::Router;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::local::{LocalAppHandle, LocalChannel, LocalRouterHandle};
use crate::network::{NetworkRead, NetworkWrite};
use crate::utils::clock::Clock;

/// Builds a client/driver pair from a router and channel.
pub struct RouterService;

impl RouterService {
    /// Splits a router and channel into a client handle and
    /// an I/O driver.
    pub fn new<'a, N, G, A, C, const MTU: usize, const OUT: usize, const QUEUE: usize>(
        router: Router<N, G, A, C, MTU, OUT>,
        channel: &'a LocalChannel<QUEUE, MTU>,
    ) -> (
        RouterClient<'a, QUEUE, MTU>,
        RouterDriver<'a, N, G, A, C, MTU, OUT, QUEUE>,
    )
    where
        N: Datalink,
        G: Datalink,
        A: RoutingAlgorithm,
        C: Clock,
    {
        let (app, router_end) = channel.split();
        (
            RouterClient { handle: app },
            RouterDriver {
                router,
                handle: router_end,
            },
        )
    }
}

/// Application-side handle for communicating with the router.
///
/// Implements [`NetworkWrite`] and [`NetworkRead`] by
/// forwarding through the [`LocalChannel`].
pub struct RouterClient<'a, const QUEUE: usize, const MTU: usize> {
    handle: LocalAppHandle<'a, QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> NetworkWrite for RouterClient<'a, QUEUE, MTU> {
    type Error = <LocalAppHandle<'a, QUEUE, MTU> as NetworkWrite>::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.handle.write(data).await
    }
}

impl<'a, const QUEUE: usize, const MTU: usize> NetworkRead for RouterClient<'a, QUEUE, MTU> {
    type Error = <LocalAppHandle<'a, QUEUE, MTU> as NetworkRead>::Error;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.handle.read(buffer).await
    }
}

/// I/O driver that runs the router and bridges the local
/// channel.
pub struct RouterDriver<'a, N, G, A, C, const MTU: usize, const OUT: usize, const QUEUE: usize> {
    router: Router<N, G, A, C, MTU, OUT>,
    handle: LocalRouterHandle<'a, QUEUE, MTU>,
}

impl<'a, N, G, A, C, const MTU: usize, const OUT: usize, const QUEUE: usize>
    RouterDriver<'a, N, G, A, C, MTU, OUT, QUEUE>
where
    N: Datalink,
    G: Datalink,
    A: RoutingAlgorithm,
    C: Clock,
{
    /// Runs the router loop: receives from the router's
    /// network reader, pushes local-destined packets to the
    /// channel, and forwards outgoing packets from the
    /// channel through the router.
    pub async fn run(&mut self) -> ! {
        let mut handle_buf = [0u8; MTU];
        let mut router_buf = [0u8; MTU];

        enum Event {
            FromNetwork(usize),
            FromApp(usize),
            Err,
        }

        loop {
            let event = {
                let handle = self.router.read(&mut handle_buf).fuse();
                let router = self.handle.read(&mut router_buf).fuse();
                pin_utils::pin_mut!(handle, router);

                futures::select_biased! {
                    r = handle => match r {
                        Ok(len) => Event::FromNetwork(len),
                        Err(_) => Event::Err,
                    },
                    r = router => match r {
                        Ok(len) => Event::FromApp(len),
                        Err(_) => Event::Err,
                    },
                }
            };

            match event {
                Event::FromNetwork(len) => {
                    let _ = self.handle.write(&handle_buf[..len]).await;
                }
                Event::FromApp(len) => {
                    let _ = self.router.write(&router_buf[..len]).await;
                }
                Event::Err => {}
            }
        }
    }
}
