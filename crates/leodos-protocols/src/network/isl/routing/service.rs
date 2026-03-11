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

use crate::datalink::{DatalinkRead, DatalinkWrite};
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
    pub fn new<'a, N, S, E, W, G, R, C, const MTU: usize, const QUEUE: usize>(
        router: Router<N, S, E, W, G, R, C, MTU>,
        channel: &'a LocalChannel<QUEUE, MTU>,
    ) -> (
        RouterClient<'a, QUEUE, MTU>,
        RouterDriver<'a, N, S, E, W, G, R, C, MTU, QUEUE>,
    )
    where
        N: DatalinkWrite + DatalinkRead<Error = <N as DatalinkWrite>::Error>,
        S: DatalinkWrite + DatalinkRead<Error = <S as DatalinkWrite>::Error>,
        E: DatalinkWrite + DatalinkRead<Error = <E as DatalinkWrite>::Error>,
        W: DatalinkWrite + DatalinkRead<Error = <W as DatalinkWrite>::Error>,
        G: DatalinkWrite + DatalinkRead<Error = <G as DatalinkWrite>::Error>,
        R: RoutingAlgorithm,
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
pub struct RouterDriver<'a, N, S, E, W, G, R, C, const MTU: usize, const QUEUE: usize> {
    router: Router<N, S, E, W, G, R, C, MTU>,
    handle: LocalRouterHandle<'a, QUEUE, MTU>,
}

impl<'a, N, S, E, W, G, R, C, const MTU: usize, const QUEUE: usize>
    RouterDriver<'a, N, S, E, W, G, R, C, MTU, QUEUE>
where
    N: DatalinkWrite + DatalinkRead<Error = <N as DatalinkWrite>::Error>,
    S: DatalinkWrite + DatalinkRead<Error = <S as DatalinkWrite>::Error>,
    E: DatalinkWrite + DatalinkRead<Error = <E as DatalinkWrite>::Error>,
    W: DatalinkWrite + DatalinkRead<Error = <W as DatalinkWrite>::Error>,
    G: DatalinkWrite + DatalinkRead<Error = <G as DatalinkWrite>::Error>,
    R: RoutingAlgorithm,
    C: Clock,
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
