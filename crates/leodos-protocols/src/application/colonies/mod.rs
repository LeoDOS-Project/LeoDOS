/// Job executor that polls and processes ColonyOS assignments.
pub mod executor;
/// ColonyOS packet format, headers, and LV-encoded payloads.
pub mod messages;
/// ColonyOS client for sending requests and receiving responses.
pub mod client;

pub use executor::ColoniesExecutor;
pub use messages::ColoniesOpCode;
