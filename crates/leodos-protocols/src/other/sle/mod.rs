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

pub use cltu::{
    CltuBindInvocation, CltuStartInvocation, CltuStatus,
    CltuTransferDataInvocation, CltuTransferDataReturn,
};
pub use isp1::{Credentials, Isp1Frame};
pub use raf::{
    RafBindInvocation, RafBindReturn, RafStartInvocation,
    RafTransferBuffer, RafTransferDataInvocation,
    RequestedFrameQuality,
};
pub use types::{
    BindResult, ServiceInstanceId, ServiceType, SleError, Time,
};
