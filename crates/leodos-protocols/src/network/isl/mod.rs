/// ISL network addressing (satellite, ground, service area).
pub mod address;
/// Geographic coordinate conversions for AOI mapping.
pub mod geo;
/// Epidemic gossip protocol for constellation-wide broadcast.
pub mod gossip;
/// Projection between geographic and grid coordinates.
pub mod projection;
/// Packet routing across the satellite mesh network.
pub mod routing;
/// Constellation shell with physical ISL distance calculations.
pub mod shell;
/// Toroidal grid topology and direction helpers.
pub mod torus;
