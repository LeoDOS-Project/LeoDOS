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
//!     async fn collect(&self, tx, job_id, assign) { ... }
//!     async fn map(&self, rx, tx, job_id, assign) { ... }
//!     async fn reduce(&self, rx, tx, job_id, assign) { ... }
//! }
//!
//! SpaceCompNode::builder()
//!     .config(config)
//!     .build()
//!     .run(&MyApp)
//!     .await?;
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

pub use config::SpaceCompConfig;
pub use error::SpaceCompError;
pub use node::SpaceComp;
pub use node::SpaceCompNode;
