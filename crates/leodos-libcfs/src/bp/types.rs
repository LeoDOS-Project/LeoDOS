//! Core BPv7 types and enums.

use crate::ffi;

/// bplib status code.
pub type Status = ffi::BPLib_Status_t;

/// Bundle block type codes per BPv7 section 9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockType {
    /// Payload block (type 1).
    Payload = ffi::BPLib_BlockType_Payload as u8,
    /// Previous node block (type 6).
    PrevNode = ffi::BPLib_BlockType_PrevNode as u8,
    /// Bundle age block (type 7).
    Age = ffi::BPLib_BlockType_Age as u8,
    /// Hop count block (type 10).
    HopCount = ffi::BPLib_BlockType_HopCount as u8,
    /// Custody Transfer Enhancement Block (type 15).
    Cteb = ffi::BPLib_BlockType_CTEB as u8,
    /// Custody Reporting Enhancement Block (type 16).
    Creb = ffi::BPLib_BlockType_CREB as u8,
}

/// CRC type for bundle integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CrcType {
    /// No CRC.
    None = ffi::BPLib_CRC_Type_None as u32,
    /// CRC-16.
    Crc16 = ffi::BPLib_CRC_Type_CRC16 as u32,
    /// CRC-32C (Castagnoli).
    Crc32C = ffi::BPLib_CRC_Type_CRC32C as u32,
}

/// Convergence layer adapter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ClaType {
    /// UDP convergence layer.
    Udp = ffi::UDPType as u32,
    /// TCP convergence layer.
    Tcp = ffi::TCPType as u32,
    /// Encapsulation Packet Protocol.
    Epp = ffi::EPPType as u32,
    /// Licklider Transmission Protocol.
    Ltp = ffi::LTPType as u32,
}

/// Contact run state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ContactState {
    /// Contact torn down.
    TornDown = ffi::BPLIB_CLA_TORNDOWN as u32,
    /// Contact set up but not started.
    Setup = ffi::BPLIB_CLA_SETUP as u32,
    /// Contact started and active.
    Started = ffi::BPLIB_CLA_STARTED as u32,
    /// Contact stopped.
    Stopped = ffi::BPLIB_CLA_STOPPED as u32,
    /// Contact exited.
    Exited = ffi::BPLIB_CLA_EXITED as u32,
}

/// Bundle processing flags.
pub mod flags {
    use crate::ffi;
    use bitflags::bitflags;

    bitflags! {
        /// Bundle processing control flags per BPv7.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct BundleFlags: u64 {
            /// Bundle is a fragment.
            const FRAGMENT = ffi::BPLIB_BUNDLE_PROC_FRAG_FLAG as u64;
            /// ADU is an administrative record.
            const ADMIN_RECORD = ffi::BPLIB_BUNDLE_PROC_ADMIN_RECORD_FLAG as u64;
            /// Bundle must not be fragmented.
            const NO_FRAGMENT = ffi::BPLIB_BUNDLE_PROC_NO_FRAG_FLAG as u64;
            /// Acknowledgement requested.
            const ACK = ffi::BPLIB_BUNDLE_PROC_ACK_FLAG as u64;
            /// Status time requested.
            const STATUS_TIME = ffi::BPLIB_BUNDLE_PROC_STATUS_TIME_FLAG as u64;
            /// Reception report requested.
            const RECV_REPORT = ffi::BPLIB_BUNDLE_PROC_RECV_REPORT_FLAG as u64;
            /// Forwarding report requested.
            const FORWARD = ffi::BPLIB_BUNDLE_PROC_FORWARD_FLAG as u64;
            /// Delivery report requested.
            const DELIVERY = ffi::BPLIB_BUNDLE_PROC_DELIVERY_FLAG as u64;
            /// Deletion report requested.
            const DELETE = ffi::BPLIB_BUNDLE_PROC_DELETE_FLAG as u64;
        }
    }
}
