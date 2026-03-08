//! NOS3 spacecraft subsystem simulators.
//!
//! Each module provides a safe interface to one simulated
//! hardware component — sensors, actuators, and payloads
//! used in the NASA Operational Simulator for Small Sats.

pub mod adcs;
pub mod adcs_math;
pub mod radio;
pub mod eps;
pub mod css;
pub mod fss;
pub mod imu;
pub mod mag;
pub mod star_tracker;
pub mod reaction_wheel;
pub mod torquer;
pub mod thruster;
pub mod novatel;
pub mod arducam;
