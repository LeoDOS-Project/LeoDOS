#![allow(dead_code)]
/// I/O traits for data sources and sinks.
pub mod io;
/// SpaceCoMP job request definition.
pub mod job;
/// MapReduce pipeline: collector, mapper, and reducer.
pub mod mapreduce;
/// SpaceCoMP wire-format messages.
pub mod packet;
/// Task-to-satellite assignment algorithms.
pub mod scheduler;
/// Zero-copy data schema trait.
pub mod schema;
