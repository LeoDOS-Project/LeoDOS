//! SpaceCoMP library for distributed computation on cFS.
//!
//! Provides [`SpaceCompNode`] which handles SRSPP transport,
//! message dispatch, coordinator orchestration, and phase
//! signaling. The user provides three closures that define
//! the computation at each stage.
//!
//! # Example
//!
//! ```ignore
//! SpaceCompNode::builder()
//!     .config(config)
//!     .collect(|partition_id, tx, buf, job_id| async move {
//!         // read sensor data, send chunks to mapper
//!     })
//!     .map(|rx, tx, buf, job_id, collector_count| async move {
//!         // process data, send results to reducer
//!     })
//!     .reduce(|rx, tx, buf, job_id, mapper_count| async move {
//!         // aggregate results, send final output
//!     })
//!     .build()
//!     .run()
//!     .await?;
//! ```

#![no_std]

pub mod config;
pub mod coordinator;
pub mod error;
pub mod node;

pub use config::SpaceCompConfig;
pub use error::SpaceCompError;
pub use node::SpaceCompNode;
