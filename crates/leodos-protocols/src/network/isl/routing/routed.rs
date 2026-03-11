//! Router with integrated local channel.
//!
//! [`RoutedRouter`] bundles directional links and routing config
//! without requiring a manually-created [`LocalChannel`]. The
//! [`run()`](RoutedRouter::run) method creates the local channel
//! internally and passes the app-side handle to a closure.

use core::future::Future;

use crate::datalink::{DatalinkReader, DatalinkWriter};
use crate::network::isl::address::Address;
use crate::network::isl::routing::algorithm::RoutingAlgorithm;
use crate::network::isl::routing::local::{LocalAppHandle, LocalChannel};
use crate::network::isl::routing::Router;
use crate::network::isl::torus::Torus;

/// A router builder that creates its own local channel.
///
/// Unlike [`Router`], this type does not require an external
/// [`LocalChannel`]. Instead, [`run()`](RoutedRouter::run)
/// creates one internally and passes the app-side handle to
/// the provided closure.
///
/// # Example
///
/// ```ignore
/// let routed = RoutedRouter::builder()
///     .north(north_link)
///     .south(south_link)
///     .east(east_link)
///     .west(west_link)
///     .ground(ground_link)
///     .address(address)
///     .torus(torus)
///     .algorithm(algorithm)
///     .build();
///
/// routed.run::<8, 1024>(|app_handle| async move {
///     // use app_handle with SrsppNode or directly
/// }).await;
/// ```
pub struct RoutedRouter<N, S, E, W, G, R> {
    north: N,
    south: S,
    east: E,
    west: W,
    ground: G,
    address: Address,
    torus: Torus,
    algorithm: R,
}

#[bon::bon]
impl<N, S, E, W, G, R> RoutedRouter<N, S, E, W, G, R> {
    /// Creates a new routed router.
    #[builder]
    pub fn new(
        north: N,
        south: S,
        east: E,
        west: W,
        ground: G,
        address: Address,
        torus: Torus,
        algorithm: R,
    ) -> Self {
        Self {
            north,
            south,
            east,
            west,
            ground,
            address,
            torus,
            algorithm,
        }
    }
}

impl<N, S, E, W, G, R> RoutedRouter<N, S, E, W, G, R> {
    /// Returns this router's address.
    pub fn address(&self) -> Address {
        self.address
    }
}

impl<N, S, E, W, G, R> RoutedRouter<N, S, E, W, G, R>
where
    N: DatalinkWriter + DatalinkReader<Error = <N as DatalinkWriter>::Error>,
    S: DatalinkWriter + DatalinkReader<Error = <S as DatalinkWriter>::Error>,
    E: DatalinkWriter + DatalinkReader<Error = <E as DatalinkWriter>::Error>,
    W: DatalinkWriter + DatalinkReader<Error = <W as DatalinkWriter>::Error>,
    G: DatalinkWriter + DatalinkReader<Error = <G as DatalinkWriter>::Error>,
    R: RoutingAlgorithm,
{
    /// Run the router with an integrated local channel.
    ///
    /// Creates a [`LocalChannel<Q, MTU>`] internally, starts
    /// the router loop, and passes the app-side handle to `app`.
    /// Both the router loop and the app closure run concurrently
    /// until either completes (in practice, the router loop runs
    /// indefinitely).
    ///
    /// `Q` is the local channel queue depth and `MTU` is the
    /// maximum packet size.
    pub async fn run<const Q: usize, const MTU: usize, F, Fut>(
        self,
        app: F,
    ) where
        F: FnOnce(LocalAppHandle<'_, Q, MTU>) -> Fut,
        Fut: Future,
    {
        let channel = LocalChannel::<Q, MTU>::new();
        let (app_handle, router_handle) = channel.split();
        let mut router = Router::builder()
            .north(self.north)
            .south(self.south)
            .east(self.east)
            .west(self.west)
            .ground(self.ground)
            .local(router_handle)
            .address(self.address)
            .torus(self.torus)
            .algorithm(self.algorithm)
            .build();

        futures::future::join(router.run(), app(app_handle)).await;
    }
}
