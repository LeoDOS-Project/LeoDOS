//! SpaceCoMP library for distributed computation on cFS.
//!
//! Provides [`SpaceCompNode`] which handles SRSPP transport,
//! message dispatch, and coordinator orchestration. The app
//! implements [`SpaceComp`] to define its computation.
//!
//! # Example
//!
//! ```ignore
//! struct MyApp;
//!
//! impl SpaceComp for MyApp {
//!     async fn collect(&mut self, tx: impl Tx) { ... }
//!     async fn map(&mut self, data: &[u8], tx: impl Tx) { ... }
//!     async fn reduce(&mut self, rx: impl Rx, tx: impl Tx) { ... }
//! }
//!
//! SpaceCompNode::builder()
//!     .app_fn(|| Ok(MyApp))
//!     .config(config)
//!     .build()
//!     .start();
//! ```

#![no_std]

pub mod job;
pub mod packet;
pub mod plan;
pub mod reader;

#[cfg(feature = "cfs")]
pub mod bufwriter;
#[cfg(feature = "cfs")]
pub mod config;
#[cfg(feature = "cfs")]
pub mod coordinator;
#[cfg(feature = "cfs")]
pub mod error;
#[cfg(feature = "cfs")]
pub mod node;
#[cfg(feature = "cfs")]
pub mod transport;

#[cfg(feature = "cfs")]
pub use config::SpaceCompConfig;
#[cfg(feature = "cfs")]
pub use error::SpaceCompError;
#[cfg(feature = "cfs")]
pub use node::SpaceComp;
#[cfg(feature = "cfs")]
pub use node::SpaceCompNode;
