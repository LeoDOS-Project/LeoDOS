//! Bidirectional bridge between a network layer and a
//! datalink.
//!
//! [`Bridge`] forwards packets in both directions: inbound
//! from the network side to the datalink side, and outbound
//! from the datalink side to the network side.
//!
//! # Example
//!
//! ```ignore
//! let channel = LocalChannel::new();
//! let (app_handle, router_handle) = channel.split();
//! let mut bridge = Bridge::new(router, router_handle);
//! join(app(app_handle), bridge.run()).await;
//! ```

use futures::FutureExt as _;

use crate::datalink::{DatalinkRead, DatalinkWrite};
use crate::network::{NetworkRead, NetworkWrite};

/// Bidirectional bridge between a [`NetworkRead`] +
/// [`NetworkWrite`] and a [`DatalinkRead`] +
/// [`DatalinkWrite`].
pub struct Bridge<N, D, const MTU: usize> {
    network: N,
    link: D,
}

impl<N, D, const MTU: usize> Bridge<N, D, MTU> {
    /// Creates a new bridge.
    pub fn new(network: N, link: D) -> Self {
        Self { network, link }
    }
}

impl<N, D, const MTU: usize> Bridge<N, D, MTU>
where
    N: NetworkRead + NetworkWrite,
    D: DatalinkRead + DatalinkWrite,
{
    /// Runs the bridge loop forever.
    pub async fn run(&mut self) -> ! {
        let mut from_net = [0u8; MTU];
        let mut from_link = [0u8; MTU];

        enum Event {
            FromNetwork(usize),
            FromLink(usize),
            Err,
        }

        loop {
            let event = {
                let net_read =
                    self.network.read(&mut from_net).fuse();
                let link_read =
                    self.link.read(&mut from_link).fuse();
                pin_utils::pin_mut!(net_read, link_read);

                futures::select_biased! {
                    r = net_read => match r {
                        Ok(len) => Event::FromNetwork(len),
                        Err(_) => Event::Err,
                    },
                    r = link_read => match r {
                        Ok(len) => Event::FromLink(len),
                        Err(_) => Event::Err,
                    },
                }
            };

            match event {
                Event::FromNetwork(len) => {
                    let _ =
                        self.link.write(&from_net[..len]).await;
                }
                Event::FromLink(len) => {
                    let _ = self
                        .network
                        .write(&from_link[..len])
                        .await;
                }
                Event::Err => {}
            }
        }
    }
}
