#![allow(dead_code)]
/// I/O traits for data sources and sinks.
pub mod io;
/// SpaceCoMP job request definition.
pub mod job;
/// SpaceCoMP role implementations: coordinator, collector, mapper, and reducer.
pub mod roles;
/// SpaceCoMP wire-format messages.
pub mod packet;
/// Job planning: satellite selection, assignment, and cost estimation.
pub mod plan;
/// Zero-copy data schema trait.
pub mod schema;
