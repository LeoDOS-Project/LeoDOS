use core::slice::Iter;

use crate::transport::cfdp::pdu::file_directive::nak::large::NakSegmentLarge;
use crate::transport::cfdp::pdu::file_directive::nak::small::NakSegmentSmall;
use crate::transport::cfdp::CfdpError;

/// NAK PDU for large file transactions (64-bit offsets).
pub mod large;
/// NAK PDU for small file transactions (32-bit offsets).
pub mod small;

/// The maximum number of missing segment requests that can be included in a single NAK PDU.
pub const MAX_NAK_SEGMENTS: usize = 32;

/// A parsed NAK PDU, dispatching between small and large file variants.
#[derive(Debug)]
pub enum NakPdu<'a> {
    /// NAK PDU for small file transactions (32-bit offsets).
    Small(&'a small::NakPduSmall),
    /// NAK PDU for large file transactions (64-bit offsets).
    Large(&'a large::NakPduLarge),
}

/// A size-independent representation of a single NAK segment request.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NakSegmentRequest {
    start_offset: u64,
    end_offset: u64,
}

impl NakSegmentRequest {
    /// Get the start_offset of the missing segment.
    pub fn start_offset(&self) -> u64 {
        self.start_offset
    }

    /// Get the end_offset of the missing segment.
    pub fn end_offset(&self) -> u64 {
        self.end_offset
    }
}

impl<'a> NakPdu<'a> {
    /// Get the start_of_scope field as a u64.
    pub fn start_of_scope(&self) -> u64 {
        match self {
            NakPdu::Small(pdu) => pdu.start_of_scope() as u64,
            NakPdu::Large(pdu) => pdu.start_of_scope(),
        }
    }

    /// Get the end_of_scope field as a u64.
    pub fn end_of_scope(&self) -> u64 {
        match self {
            NakPdu::Small(pdu) => pdu.end_of_scope() as u64,
            NakPdu::Large(pdu) => pdu.end_of_scope(),
        }
    }

    /// Get the segment requests as a vector of NakSegmentRequest.
    pub fn segment_requests(&self) -> Result<NakSegmentsIterator<'a>, CfdpError> {
        match self {
            NakPdu::Small(pdu) => {
                let segments = pdu.segment_requests()?;
                Ok(NakSegmentsIterator::Small(segments.iter()))
            }
            NakPdu::Large(pdu) => {
                let segments = pdu.segment_requests()?;
                Ok(NakSegmentsIterator::Large(segments.iter()))
            }
        }
    }

    /// Returns the raw trailing bytes after the scope fields.
    pub fn rest(&self) -> &[u8] {
        match self {
            NakPdu::Small(pdu) => pdu.rest(),
            NakPdu::Large(pdu) => pdu.rest(),
        }
    }
}

/// An iterator over NAK segment requests, abstracting small and large variants.
#[derive(Debug)]
pub enum NakSegmentsIterator<'a> {
    /// Iterator over 32-bit NAK segments.
    Small(Iter<'a, NakSegmentSmall>),
    /// Iterator over 64-bit NAK segments.
    Large(Iter<'a, NakSegmentLarge>),
}

impl<'a> Iterator for NakSegmentsIterator<'a> {
    type Item = NakSegmentRequest;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            NakSegmentsIterator::Small(iter) => iter.next().map(|seg| NakSegmentRequest {
                start_offset: seg.start_offset() as u64,
                end_offset: seg.end_offset() as u64,
            }),
            NakSegmentsIterator::Large(iter) => iter.next().map(|seg| NakSegmentRequest {
                start_offset: seg.start_offset(),
                end_offset: seg.end_offset(),
            }),
        }
    }
}

