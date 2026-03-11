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

use crate::datalink::{DatalinkReader, DatalinkWriter};
use crate::network::isl::routing::Router;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::local::{LocalAppHandle, LocalChannel, LocalRouterHandle};
use crate::network::{NetworkReader, NetworkWriter};

/// Builds a client/driver pair from a router and channel.
pub struct RouterService;

impl RouterService {
    /// Splits a router and channel into a client handle and
    /// an I/O driver.
    pub fn new<'a, N, S, E, W, G, R, const MTU: usize, const QUEUE: usize>(
        router: Router<N, S, E, W, G, R, MTU>,
        channel: &'a LocalChannel<QUEUE, MTU>,
    ) -> (
        RouterClient<'a, QUEUE, MTU>,
        RouterDriver<'a, N, S, E, W, G, R, MTU, QUEUE>,
    )
    where
        N: DatalinkWriter + DatalinkReader<Error = <N as DatalinkWriter>::Error>,
        S: DatalinkWriter + DatalinkReader<Error = <S as DatalinkWriter>::Error>,
        E: DatalinkWriter + DatalinkReader<Error = <E as DatalinkWriter>::Error>,
        W: DatalinkWriter + DatalinkReader<Error = <W as DatalinkWriter>::Error>,
        G: DatalinkWriter + DatalinkReader<Error = <G as DatalinkWriter>::Error>,
        R: RoutingAlgorithm,
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
/// Implements [`NetworkWriter`] and [`NetworkReader`] by
/// forwarding through the [`LocalChannel`].
pub struct RouterClient<'a, const QUEUE: usize, const MTU: usize> {
    handle: LocalAppHandle<'a, QUEUE, MTU>,
}

impl<'a, const QUEUE: usize, const MTU: usize> NetworkWriter for RouterClient<'a, QUEUE, MTU> {
    type Error = <LocalAppHandle<'a, QUEUE, MTU> as NetworkWriter>::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.handle.write(data).await
    }
}

impl<'a, const QUEUE: usize, const MTU: usize> NetworkReader for RouterClient<'a, QUEUE, MTU> {
    type Error = <LocalAppHandle<'a, QUEUE, MTU> as NetworkReader>::Error;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        self.handle.read(buffer).await
    }
}

/// I/O driver that runs the router and bridges the local
/// channel.
pub struct RouterDriver<'a, N, S, E, W, G, R, const MTU: usize, const QUEUE: usize> {
    router: Router<N, S, E, W, G, R, MTU>,
    handle: LocalRouterHandle<'a, QUEUE, MTU>,
}

impl<'a, N, S, E, W, G, R, const MTU: usize, const QUEUE: usize>
    RouterDriver<'a, N, S, E, W, G, R, MTU, QUEUE>
where
    N: DatalinkWriter + DatalinkReader<Error = <N as DatalinkWriter>::Error>,
    S: DatalinkWriter + DatalinkReader<Error = <S as DatalinkWriter>::Error>,
    E: DatalinkWriter + DatalinkReader<Error = <E as DatalinkWriter>::Error>,
    W: DatalinkWriter + DatalinkReader<Error = <W as DatalinkWriter>::Error>,
    G: DatalinkWriter + DatalinkReader<Error = <G as DatalinkWriter>::Error>,
    R: RoutingAlgorithm,
{
    /// Runs the router loop: receives from the router's
    /// network reader, pushes local-destined packets to the
    /// channel, and forwards outgoing packets from the
    /// channel through the router.
    pub async fn run(&mut self) -> ! {
        let mut buf = [0u8; MTU];
        loop {
            // Receive from the network (blocks until a
            // local-destined packet arrives, forwarding
            // non-local packets internally).
            if let Ok(len) = self.router.read(&mut buf).await {
                let _ = self.handle.write(&buf[..len]).await;
            }

            // Drain any outgoing packets from the app.
            let mut local_buf = [0u8; MTU];
            if let Ok(len) = self.handle.read(&mut local_buf).await {
                let _ = self.router.write(&local_buf[..len]).await;
            }
        }
    }
}
