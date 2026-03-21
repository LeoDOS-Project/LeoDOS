//! CCSDS Space Link Extension (SLE) protocol.
//!
//! SLE provides a standardized interface for spacecraft
//! communication through ground station antennas. It runs over
//! TCP using ISP1 framing with ASN.1 BER-encoded PDUs.
//!
//! Two services are supported:
//! - **RAF** (Return All Frames) — receive downlink TM frames
//! - **CLTU** (Forward CLTU) — send uplink command frames
//!
//! This module contains only PDU types and BER codecs.
//! Actual TCP I/O is handled by the caller (e.g. leodos-cli).

/// Minimal ASN.1 BER encoder/decoder.
pub mod ber;
/// CLTU (Forward Command) service PDUs.
pub mod cltu;
/// ISP1 transport layer framing and credentials.
pub mod isp1;
/// RAF (Return All Frames) service PDUs.
pub mod raf;
/// Shared SLE types.
pub mod types;

pub use cltu::CltuBindInvocation;
pub use cltu::CltuStartInvocation;
pub use cltu::CltuStatus;
pub use cltu::CltuTransferDataInvocation;
pub use cltu::CltuTransferDataReturn;
pub use isp1::Credentials;
pub use isp1::Isp1Frame;
pub use raf::RafBindInvocation;
pub use raf::RafBindReturn;
pub use raf::RafStartInvocation;
pub use raf::RafTransferBuffer;
pub use raf::RafTransferDataInvocation;
pub use raf::RequestedFrameQuality;
pub use types::BindResult;
pub use types::ServiceInstanceId;
pub use types::ServiceType;
pub use types::SleError;
pub use types::Time;
