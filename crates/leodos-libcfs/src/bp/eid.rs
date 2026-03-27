//! Endpoint Identifier (EID) types and operations.
//!
//! An EID identifies a bundle endpoint — the source or destination of a
//! bundle. BPv7 supports two URI schemes: `dtn:` and `ipn:`.

use crate::ffi;

const IPN_SCHEME: u64 = 2;
const IPN_SSP_2DIGIT: u64 = 2;
const IPN_SSP_3DIGIT: u64 = 3;

/// A bundle endpoint identifier.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Eid(pub(crate) ffi::BPLib_EID_t);

impl Eid {
    /// Creates a new IPN-scheme EID with 2-digit format.
    pub fn ipn(node: u64, service: u64) -> Self {
        Self(ffi::BPLib_EID_t {
            Scheme: IPN_SCHEME,
            IpnSspFormat: IPN_SSP_2DIGIT,
            Allocator: 0,
            Node: node,
            Service: service,
        })
    }

    /// Creates a new IPN-scheme EID with 3-digit format (allocator.node.service).
    pub fn ipn3(allocator: u64, node: u64, service: u64) -> Self {
        Self(ffi::BPLib_EID_t {
            Scheme: IPN_SCHEME,
            IpnSspFormat: IPN_SSP_3DIGIT,
            Allocator: allocator,
            Node: node,
            Service: service,
        })
    }

    /// Returns the node number.
    pub fn node(&self) -> u64 {
        self.0.Node
    }

    /// Returns the service number.
    pub fn service(&self) -> u64 {
        self.0.Service
    }

    /// Returns the allocator number (3-digit IPN only).
    pub fn allocator(&self) -> u64 {
        self.0.Allocator
    }

    /// Checks whether this EID is valid.
    pub fn is_valid(&self) -> bool {
        unsafe { ffi::BPLib_EID_IsValid(&self.0 as *const _ as *mut _) }
    }

    /// Checks whether this EID matches another EID.
    pub fn matches(&self, other: &Eid) -> bool {
        unsafe { ffi::BPLib_EID_IsMatch(&self.0 as *const _, &other.0 as *const _) }
    }
}

/// An EID pattern for matching against multiple endpoints.
///
/// Patterns use min/max ranges for each field. Setting min=0 and
/// max=u64::MAX for a field matches any value (wildcard).
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct EidPattern(pub(crate) ffi::BPLib_EID_Pattern_t);

impl EidPattern {
    /// Creates a pattern that matches a specific node (any service).
    pub fn node(node: u64) -> Self {
        Self(ffi::BPLib_EID_Pattern_t {
            Scheme: IPN_SCHEME,
            IpnSspFormat: IPN_SSP_2DIGIT,
            MaxAllocator: u64::MAX,
            MinAllocator: 0,
            MaxNode: node,
            MinNode: node,
            MaxService: u64::MAX,
            MinService: 0,
        })
    }

    /// Creates a pattern that matches a specific node and service.
    pub fn exact(node: u64, service: u64) -> Self {
        Self(ffi::BPLib_EID_Pattern_t {
            Scheme: IPN_SCHEME,
            IpnSspFormat: IPN_SSP_2DIGIT,
            MaxAllocator: u64::MAX,
            MinAllocator: 0,
            MaxNode: node,
            MinNode: node,
            MaxService: service,
            MinService: service,
        })
    }

    /// Checks whether an EID matches this pattern.
    pub fn matches(&self, eid: &Eid) -> bool {
        unsafe {
            ffi::BPLib_EID_PatternIsMatch(
                &eid.0 as *const _ as *mut _,
                &self.0 as *const _ as *mut _,
            )
        }
    }
}
