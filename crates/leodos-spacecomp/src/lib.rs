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

pub mod bufwriter;
pub mod config;
pub mod coordinator;
pub mod error;
pub mod job;
pub mod node;
pub mod packet;
pub mod plan;
pub mod reader;
pub mod transport;

pub use config::SpaceCompConfig;
pub use error::SpaceCompError;
pub use node::SpaceComp;
pub use node::SpaceCompNode;
