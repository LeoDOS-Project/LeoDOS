//! CFDP PDU encoding and decoding types.

use crate::ffi;
use core::marker::PhantomData;

/// Raw CFDP PDU header structure.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct PduHeader(pub(crate) ffi::CF_CFDP_PduHeader_t);

/// Logical representation of a CFDP PDU header with decoded fields.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduHeader(pub(crate) ffi::CF_Logical_PduHeader_t);

impl LogicalPduHeader {
    /// Returns the CFDP version number.
    pub fn version(&self) -> u8 {
        self.0.version
    }

    /// Returns the PDU type (0 = file directive, 1 = file data).
    pub fn pdu_type(&self) -> u8 {
        self.0.pdu_type
    }

    /// Returns the direction (0 = toward receiver, 1 = toward sender).
    pub fn direction(&self) -> u8 {
        self.0.direction
    }

    /// Returns the transmission mode (0 = acknowledged, 1 = unacknowledged).
    pub fn txm_mode(&self) -> u8 {
        self.0.txm_mode
    }

    /// Returns whether CRC is present (0 = no, 1 = yes).
    pub fn crc_flag(&self) -> u8 {
        self.0.crc_flag
    }

    /// Returns the large file flag (0 = 32-bit size, 1 = 64-bit size).
    pub fn large_flag(&self) -> u8 {
        self.0.large_flag
    }

    /// Returns the entity ID length in octets.
    pub fn eid_length(&self) -> u8 {
        self.0.eid_length
    }

    /// Returns the transaction sequence number length in octets.
    pub fn txn_seq_length(&self) -> u8 {
        self.0.txn_seq_length
    }

    /// Returns the encoded header length in octets.
    pub fn header_encoded_length(&self) -> u16 {
        self.0.header_encoded_length
    }

    /// Returns the encoded data length in octets.
    pub fn data_encoded_length(&self) -> u16 {
        self.0.data_encoded_length
    }

    /// Returns the source entity ID.
    pub fn source_eid(&self) -> u32 {
        self.0.source_eid
    }

    /// Returns the destination entity ID.
    pub fn destination_eid(&self) -> u32 {
        self.0.destination_eid
    }

    /// Returns the transaction sequence number.
    pub fn sequence_num(&self) -> u32 {
        self.0.sequence_num
    }
}

/// State for encoding CFDP PDUs into a buffer.
pub struct EncoderState<'a> {
    inner: ffi::CF_EncoderState_t,
    _marker: PhantomData<&'a mut [u8]>,
}

impl<'a> EncoderState<'a> {
    /// Creates a new encoder state wrapping the given buffer.
    pub fn new(buffer: &'a mut [u8]) -> Self {
        let mut inner = ffi::CF_EncoderState_t::default();
        inner.base = buffer.as_mut_ptr();
        inner.codec_state.is_valid = true;
        inner.codec_state.next_offset = 0;
        inner.codec_state.max_size = buffer.len();
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    /// Returns true if the encoder is in a valid state.
    pub fn is_ok(&self) -> bool {
        self.inner.codec_state.is_valid
    }

    /// Returns the current position in the buffer.
    pub fn position(&self) -> usize {
        self.inner.codec_state.next_offset
    }

    /// Returns the remaining space in the buffer.
    pub fn remaining(&self) -> usize {
        self.inner.codec_state.max_size - self.inner.codec_state.next_offset
    }

    #[allow(dead_code)]
    pub(crate) fn as_raw_mut(&mut self) -> *mut ffi::CF_EncoderState_t {
        &mut self.inner
    }
}

/// State for decoding CFDP PDUs from a buffer.
pub struct DecoderState<'a> {
    inner: ffi::CF_DecoderState_t,
    _marker: PhantomData<&'a [u8]>,
}

impl<'a> DecoderState<'a> {
    /// Creates a new decoder state wrapping the given buffer.
    pub fn new(buffer: &'a [u8]) -> Self {
        let mut inner = ffi::CF_DecoderState_t::default();
        inner.base = buffer.as_ptr();
        inner.codec_state.is_valid = true;
        inner.codec_state.next_offset = 0;
        inner.codec_state.max_size = buffer.len();
        Self {
            inner,
            _marker: PhantomData,
        }
    }

    /// Returns true if the decoder is in a valid state.
    pub fn is_ok(&self) -> bool {
        self.inner.codec_state.is_valid
    }

    /// Returns the current position in the buffer.
    pub fn position(&self) -> usize {
        self.inner.codec_state.next_offset
    }

    /// Returns the remaining bytes in the buffer.
    pub fn remaining(&self) -> usize {
        self.inner.codec_state.max_size - self.inner.codec_state.next_offset
    }

    #[allow(dead_code)]
    pub(crate) fn as_raw_mut(&mut self) -> *mut ffi::CF_DecoderState_t {
        &mut self.inner
    }
}

/// Logical representation of an EOF (End of File) PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduEof(pub(crate) ffi::CF_Logical_PduEof_t);

impl LogicalPduEof {
    /// Returns the condition code from the EOF PDU.
    pub fn condition_code(&self) -> ffi::CF_CFDP_ConditionCode_t {
        self.0.cc
    }

    /// Returns the file checksum.
    pub fn crc(&self) -> u32 {
        self.0.crc
    }

    /// Returns the file size.
    pub fn size(&self) -> u32 {
        self.0.size
    }
}

/// Logical representation of a FIN (Finished) PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduFin(pub(crate) ffi::CF_Logical_PduFin_t);

impl LogicalPduFin {
    /// Returns the condition code from the FIN PDU.
    pub fn condition_code(&self) -> ffi::CF_CFDP_ConditionCode_t {
        self.0.cc
    }

    /// Returns the file status.
    pub fn file_status(&self) -> ffi::CF_CFDP_FinFileStatus_t {
        self.0.file_status
    }

    /// Returns the delivery code (0 = complete, non-zero = incomplete).
    pub fn delivery_code(&self) -> u8 {
        self.0.delivery_code
    }
}

/// Logical representation of an ACK (Acknowledge) PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduAck(pub(crate) ffi::CF_Logical_PduAck_t);

impl LogicalPduAck {
    /// Returns the directive code being acknowledged.
    pub fn ack_directive_code(&self) -> u8 {
        self.0.ack_directive_code
    }

    /// Returns the ACK subtype code.
    pub fn ack_subtype_code(&self) -> u8 {
        self.0.ack_subtype_code
    }

    /// Returns the condition code from the ACK PDU.
    pub fn condition_code(&self) -> ffi::CF_CFDP_ConditionCode_t {
        self.0.cc
    }

    /// Returns the transaction status.
    pub fn txn_status(&self) -> ffi::CF_CFDP_AckTxnStatus_t {
        self.0.txn_status
    }
}

/// Logical representation of a Metadata PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduMd(pub(crate) ffi::CF_Logical_PduMd_t);

impl LogicalPduMd {
    /// Returns whether closure is requested (0 = no, 1 = yes).
    pub fn close_req(&self) -> u8 {
        self.0.close_req
    }

    /// Returns the checksum type (0 = legacy modular checksum).
    pub fn checksum_type(&self) -> u8 {
        self.0.checksum_type
    }

    /// Returns the file size.
    pub fn size(&self) -> u32 {
        self.0.size
    }
}

/// Logical representation of a NAK (Negative Acknowledge) PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduNak(pub(crate) ffi::CF_Logical_PduNak_t);

impl LogicalPduNak {
    /// Returns the scope start offset.
    pub fn scope_start(&self) -> u32 {
        self.0.scope_start
    }

    /// Returns the scope end offset.
    pub fn scope_end(&self) -> u32 {
        self.0.scope_end
    }
}

/// Logical representation of a File Data PDU header.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduFileData(pub(crate) ffi::CF_Logical_PduFileDataHeader_t);

impl LogicalPduFileData {
    /// Returns the continuation state.
    pub fn continuation_state(&self) -> u8 {
        self.0.continuation_state
    }

    /// Returns the file offset for this data segment.
    pub fn offset(&self) -> u32 {
        self.0.offset
    }

    /// Returns the length of the data segment.
    pub fn data_len(&self) -> usize {
        self.0.data_len
    }
}

/// A segment request representing a range of missing file data.
#[derive(Debug, Clone, Copy)]
pub struct SegmentRequest {
    /// Start offset of the missing segment.
    pub offset_start: u32,
    /// End offset of the missing segment.
    pub offset_end: u32,
}

impl From<ffi::CF_Logical_SegmentRequest_t> for SegmentRequest {
    fn from(val: ffi::CF_Logical_SegmentRequest_t) -> Self {
        Self {
            offset_start: val.offset_start,
            offset_end: val.offset_end,
        }
    }
}

/// Logical Length-Value pair (used for filenames in CFDP).
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalLv(pub(crate) ffi::CF_Logical_Lv_t);

impl LogicalLv {
    /// Returns the length of the data.
    pub fn length(&self) -> u8 {
        self.0.length
    }

    /// Returns the data as a byte slice, if available.
    ///
    /// # Safety
    /// The data pointer must be valid for the length specified.
    pub unsafe fn data(&self) -> Option<&[u8]> {
        if self.0.data_ptr.is_null() || self.0.length == 0 {
            None
        } else {
            Some(core::slice::from_raw_parts(
                self.0.data_ptr as *const u8,
                self.0.length as usize,
            ))
        }
    }
}

/// Logical Type-Length-Value tuple (used for optional fields in CFDP).
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalTlv(pub(crate) ffi::CF_Logical_Tlv_t);

impl LogicalTlv {
    /// Returns the TLV type.
    pub fn tlv_type(&self) -> ffi::CF_CFDP_TlvType_t {
        self.0.type_
    }

    /// Returns the length of the data.
    pub fn length(&self) -> u8 {
        self.0.length
    }
}

/// List of TLV entries in a PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalTlvList(pub(crate) ffi::CF_Logical_TlvList_t);

impl LogicalTlvList {
    /// Returns the number of TLV entries.
    pub fn num_tlv(&self) -> u8 {
        self.0.num_tlv
    }
}

/// List of segment requests in a PDU.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalSegmentList(pub(crate) ffi::CF_Logical_SegmentList_t);

impl LogicalSegmentList {
    /// Returns the number of segments.
    pub fn num_segments(&self) -> u8 {
        self.0.num_segments
    }
}

/// File directive header.
#[repr(transparent)]
#[derive(Clone, Copy, Default)]
pub struct LogicalPduFileDirectiveHeader(pub(crate) ffi::CF_Logical_PduFileDirectiveHeader_t);

impl LogicalPduFileDirectiveHeader {
    /// Returns the directive code.
    pub fn directive_code(&self) -> ffi::CF_CFDP_FileDirective_t {
        self.0.directive_code
    }
}

/// Logical PDU buffer encapsulating entire PDU information.
#[repr(transparent)]
pub struct LogicalPduBuffer(pub(crate) ffi::CF_Logical_PduBuffer_t);

impl Default for LogicalPduBuffer {
    fn default() -> Self {
        Self(unsafe { core::mem::zeroed() })
    }
}

impl LogicalPduBuffer {
    /// Returns a reference to the PDU header.
    pub fn pdu_header(&self) -> &LogicalPduHeader {
        unsafe { &*(&self.0.pdu_header as *const _ as *const LogicalPduHeader) }
    }

    /// Returns a mutable reference to the PDU header.
    pub fn pdu_header_mut(&mut self) -> &mut LogicalPduHeader {
        unsafe { &mut *(&mut self.0.pdu_header as *mut _ as *mut LogicalPduHeader) }
    }

    /// Returns a reference to the file directive header.
    pub fn fdirective(&self) -> &LogicalPduFileDirectiveHeader {
        unsafe { &*(&self.0.fdirective as *const _ as *const LogicalPduFileDirectiveHeader) }
    }

    /// Returns a mutable reference to the file directive header.
    pub fn fdirective_mut(&mut self) -> &mut LogicalPduFileDirectiveHeader {
        unsafe { &mut *(&mut self.0.fdirective as *mut _ as *mut LogicalPduFileDirectiveHeader) }
    }

    /// Returns the content CRC value.
    pub fn content_crc(&self) -> u32 {
        self.0.content_crc
    }

    /// Returns a reference to the EOF data (if this is an EOF PDU).
    pub fn eof(&self) -> &LogicalPduEof {
        unsafe { &*(&self.0.int_header.eof as *const _ as *const LogicalPduEof) }
    }

    /// Returns a mutable reference to the EOF data.
    pub fn eof_mut(&mut self) -> &mut LogicalPduEof {
        unsafe { &mut *(&mut self.0.int_header.eof as *mut _ as *mut LogicalPduEof) }
    }

    /// Returns a reference to the FIN data (if this is a FIN PDU).
    pub fn fin(&self) -> &LogicalPduFin {
        unsafe { &*(&self.0.int_header.fin as *const _ as *const LogicalPduFin) }
    }

    /// Returns a mutable reference to the FIN data.
    pub fn fin_mut(&mut self) -> &mut LogicalPduFin {
        unsafe { &mut *(&mut self.0.int_header.fin as *mut _ as *mut LogicalPduFin) }
    }

    /// Returns a reference to the ACK data (if this is an ACK PDU).
    pub fn ack(&self) -> &LogicalPduAck {
        unsafe { &*(&self.0.int_header.ack as *const _ as *const LogicalPduAck) }
    }

    /// Returns a mutable reference to the ACK data.
    pub fn ack_mut(&mut self) -> &mut LogicalPduAck {
        unsafe { &mut *(&mut self.0.int_header.ack as *mut _ as *mut LogicalPduAck) }
    }

    /// Returns a reference to the metadata (if this is a metadata PDU).
    pub fn md(&self) -> &LogicalPduMd {
        unsafe { &*(&self.0.int_header.md as *const _ as *const LogicalPduMd) }
    }

    /// Returns a mutable reference to the metadata.
    pub fn md_mut(&mut self) -> &mut LogicalPduMd {
        unsafe { &mut *(&mut self.0.int_header.md as *mut _ as *mut LogicalPduMd) }
    }

    /// Returns a reference to the NAK data (if this is a NAK PDU).
    pub fn nak(&self) -> &LogicalPduNak {
        unsafe { &*(&self.0.int_header.nak as *const _ as *const LogicalPduNak) }
    }

    /// Returns a mutable reference to the NAK data.
    pub fn nak_mut(&mut self) -> &mut LogicalPduNak {
        unsafe { &mut *(&mut self.0.int_header.nak as *mut _ as *mut LogicalPduNak) }
    }

    /// Returns a reference to the file data header (if this is a file data PDU).
    pub fn fd(&self) -> &LogicalPduFileData {
        unsafe { &*(&self.0.int_header.fd as *const _ as *const LogicalPduFileData) }
    }

    /// Returns a mutable reference to the file data header.
    pub fn fd_mut(&mut self) -> &mut LogicalPduFileData {
        unsafe { &mut *(&mut self.0.int_header.fd as *mut _ as *mut LogicalPduFileData) }
    }
}
