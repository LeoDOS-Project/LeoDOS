#![allow(dead_code)]
/// I/O traits for data sources and sinks.
pub mod io;
/// Job planning utilities (Aoi).
pub mod plan;
/// SpaceCoMP role traits: collector, mapper, reducer.
pub mod roles;
/// Zero-copy data schema trait.
pub mod schema;
