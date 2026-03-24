//! SpaceCoMP library for distributed computation on cFS.
//!
//! Provides [`SpaceCompNode`] which handles SRSPP transport,
//! message dispatch, and coordinator orchestration. The user
//! implements [`SpaceCompJob`] to define their computation.
//!
//! # Example
//!
//! ```ignore
//! struct MyJob;
//!
//! impl SpaceCompJob for MyJob {
//!     type Collected = MyRecord;
//!     type Mapped = MyIntermediate;
//!     type Result = MyOutput;
//!
//!     fn collector(&mut self) -> impl Collector<Input = Self::Collected, Output = Self::Collected> { ... }
//!     fn mapper(&mut self) -> impl Mapper<Input = Self::Collected, Output = Self::Mapped> { ... }
//!     fn reducer(&mut self) -> impl Reducer<Input = Self::Mapped, Output = Self::Result> { ... }
//! }
//!
//! SpaceCompNode::builder()
//!     .job(MyJob)
//!     .config(config)
//!     .build()
//!     .run()
//!     .await?;
//! ```

#![no_std]

pub mod config;
pub mod coordinator;
pub mod error;
pub mod job;
pub mod node;
pub mod transport;

pub use config::SpaceCompConfig;
pub use error::SpaceCompError;
pub use job::SpaceCompJob;
pub use node::SpaceCompNode;

pub use leodos_protocols::application::spacecomp::io::sink::Sink;
pub use leodos_protocols::application::spacecomp::io::source::Source;
pub use leodos_protocols::application::spacecomp::io::writer::MessageSender;
pub use leodos_protocols::application::spacecomp::roles::collector::Collector;
pub use leodos_protocols::application::spacecomp::roles::mapper::Mapper;
pub use leodos_protocols::application::spacecomp::roles::reducer::Reducer;
pub use leodos_protocols::application::spacecomp::schema::Schema;
