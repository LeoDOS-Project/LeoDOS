//! CFDP PDU codec functions for encoding and decoding PDUs.

use crate::ffi;
use crate::cf::pdu::{
    DecoderState, EncoderState, LogicalPduAck, LogicalPduEof, LogicalPduFin, LogicalPduHeader,
    LogicalPduMd, LogicalPduNak,
};
use crate::error::Error;
use crate::status::{self, Status};

impl EncoderState<'_> {
    /// Encodes a PDU header without the size field.
    pub fn encode_header_without_size(&mut self, header: &mut LogicalPduHeader) {
        unsafe {
            ffi::CF_CFDP_EncodeHeaderWithoutSize(self.as_raw_mut(), &mut header.0);
        }
    }

    /// Updates an already-encoded PDU header with the final size.
    pub fn encode_header_final_size(&mut self, header: &mut LogicalPduHeader) {
        unsafe {
            ffi::CF_CFDP_EncodeHeaderFinalSize(self.as_raw_mut(), &mut header.0);
        }
    }

    /// Encodes an EOF PDU.
    pub fn encode_eof(&mut self, eof: &mut LogicalPduEof) {
        unsafe {
            ffi::CF_CFDP_EncodeEof(self.as_raw_mut(), &mut eof.0);
        }
    }

    /// Encodes a FIN PDU.
    pub fn encode_fin(&mut self, fin: &mut LogicalPduFin) {
        unsafe {
            ffi::CF_CFDP_EncodeFin(self.as_raw_mut(), &mut fin.0);
        }
    }

    /// Encodes an ACK PDU.
    pub fn encode_ack(&mut self, ack: &mut LogicalPduAck) {
        unsafe {
            ffi::CF_CFDP_EncodeAck(self.as_raw_mut(), &mut ack.0);
        }
    }

    /// Encodes a NAK PDU.
    pub fn encode_nak(&mut self, nak: &mut LogicalPduNak) {
        unsafe {
            ffi::CF_CFDP_EncodeNak(self.as_raw_mut(), &mut nak.0);
        }
    }

    /// Encodes a metadata PDU.
    pub fn encode_md(&mut self, md: &mut LogicalPduMd) {
        unsafe {
            ffi::CF_CFDP_EncodeMd(self.as_raw_mut(), &mut md.0);
        }
    }

    /// Encodes a CRC value.
    pub fn encode_crc(&mut self, crc: &mut u32) {
        unsafe {
            ffi::CF_CFDP_EncodeCrc(self.as_raw_mut(), crc);
        }
    }

    /// Encodes an integer in the specified number of octets.
    pub fn encode_integer(&mut self, value: u64, size: u8) {
        unsafe {
            ffi::CF_EncodeIntegerInSize(self.as_raw_mut(), value, size);
        }
    }
}

impl DecoderState<'_> {
    /// Decodes a PDU header.
    pub fn decode_header(&mut self, header: &mut LogicalPduHeader) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_DecodeHeader(self.as_raw_mut(), &mut header.0) })
    }

    /// Decodes an EOF PDU.
    pub fn decode_eof(&mut self, eof: &mut LogicalPduEof) {
        unsafe {
            ffi::CF_CFDP_DecodeEof(self.as_raw_mut(), &mut eof.0);
        }
    }

    /// Decodes a FIN PDU.
    pub fn decode_fin(&mut self, fin: &mut LogicalPduFin) {
        unsafe {
            ffi::CF_CFDP_DecodeFin(self.as_raw_mut(), &mut fin.0);
        }
    }

    /// Decodes an ACK PDU.
    pub fn decode_ack(&mut self, ack: &mut LogicalPduAck) {
        unsafe {
            ffi::CF_CFDP_DecodeAck(self.as_raw_mut(), &mut ack.0);
        }
    }

    /// Decodes a NAK PDU.
    pub fn decode_nak(&mut self, nak: &mut LogicalPduNak) {
        unsafe {
            ffi::CF_CFDP_DecodeNak(self.as_raw_mut(), &mut nak.0);
        }
    }

    /// Decodes a metadata PDU.
    pub fn decode_md(&mut self, md: &mut LogicalPduMd) {
        unsafe {
            ffi::CF_CFDP_DecodeMd(self.as_raw_mut(), &mut md.0);
        }
    }

    /// Decodes a CRC value.
    pub fn decode_crc(&mut self, crc: &mut u32) {
        unsafe {
            ffi::CF_CFDP_DecodeCrc(self.as_raw_mut(), crc);
        }
    }

    /// Decodes an integer from the specified number of octets.
    pub fn decode_integer(&mut self, size: u8) -> u64 {
        unsafe { ffi::CF_DecodeIntegerInSize(self.as_raw_mut(), size) }
    }
}

/// Returns the minimum number of octets needed to encode a value.
pub fn get_value_encoded_size(value: u64) -> u8 {
    unsafe { ffi::CF_CFDP_GetValueEncodedSize(value) }
}

/// Checks if there is enough space in the codec state for a chunk of the given size.
#[allow(dead_code)]
pub(crate) fn codec_check_size(state: &mut ffi::CF_CodecState_t, chunk_size: usize) -> bool {
    unsafe { ffi::CF_CFDP_CodecCheckSize(state, chunk_size) }
}

impl EncoderState<'_> {
    /// Reserves space for a chunk and returns a pointer to write data to.
    ///
    /// Returns a pointer to the reserved buffer location, or null if insufficient space.
    pub fn do_encode_chunk(&mut self, chunk_size: usize) -> *mut u8 {
        unsafe { ffi::CF_CFDP_DoEncodeChunk(self.as_raw_mut(), chunk_size) as *mut u8 }
    }
}

impl DecoderState<'_> {
    /// Reads a chunk and returns a pointer to the data.
    ///
    /// Returns a pointer to the data in the buffer, or null if insufficient data.
    pub fn do_decode_chunk(&mut self, chunk_size: usize) -> *const u8 {
        unsafe { ffi::CF_CFDP_DoDecodeChunk(self.as_raw_mut(), chunk_size) as *const u8 }
    }
}

impl EncoderState<'_> {
    /// Encodes a file directive header.
    pub fn encode_file_directive_header(
        &mut self,
        fdh: &mut crate::cf::pdu::LogicalPduFileDirectiveHeader,
    ) {
        unsafe {
            ffi::CF_CFDP_EncodeFileDirectiveHeader(self.as_raw_mut(), &mut fdh.0);
        }
    }

    /// Encodes an LV (Length-Value) pair.
    pub fn encode_lv(&mut self, lv: &mut crate::cf::pdu::LogicalLv) {
        unsafe {
            ffi::CF_CFDP_EncodeLV(self.as_raw_mut(), &mut lv.0);
        }
    }

    /// Encodes a TLV (Type-Length-Value) tuple.
    pub fn encode_tlv(&mut self, tlv: &mut crate::cf::pdu::LogicalTlv) {
        unsafe {
            ffi::CF_CFDP_EncodeTLV(self.as_raw_mut(), &mut tlv.0);
        }
    }

    /// Encodes a segment request.
    #[allow(dead_code)]
    pub(crate) fn encode_segment_request(&mut self, sr: &mut ffi::CF_Logical_SegmentRequest_t) {
        unsafe {
            ffi::CF_CFDP_EncodeSegmentRequest(self.as_raw_mut(), sr);
        }
    }

    /// Encodes all TLVs in a list.
    pub fn encode_all_tlv(&mut self, tlv_list: &mut crate::cf::pdu::LogicalTlvList) {
        unsafe {
            ffi::CF_CFDP_EncodeAllTlv(self.as_raw_mut(), &mut tlv_list.0);
        }
    }

    /// Encodes all segments in a list.
    pub fn encode_all_segments(&mut self, segment_list: &mut crate::cf::pdu::LogicalSegmentList) {
        unsafe {
            ffi::CF_CFDP_EncodeAllSegments(self.as_raw_mut(), &mut segment_list.0);
        }
    }

    /// Encodes a file data header.
    pub fn encode_file_data_header(
        &mut self,
        with_meta: bool,
        fd: &mut crate::cf::pdu::LogicalPduFileData,
    ) {
        unsafe {
            ffi::CF_CFDP_EncodeFileDataHeader(self.as_raw_mut(), with_meta, &mut fd.0);
        }
    }
}

impl DecoderState<'_> {
    /// Decodes a file directive header.
    pub fn decode_file_directive_header(
        &mut self,
        fdh: &mut crate::cf::pdu::LogicalPduFileDirectiveHeader,
    ) {
        unsafe {
            ffi::CF_CFDP_DecodeFileDirectiveHeader(self.as_raw_mut(), &mut fdh.0);
        }
    }

    /// Decodes an LV (Length-Value) pair.
    pub fn decode_lv(&mut self, lv: &mut crate::cf::pdu::LogicalLv) {
        unsafe {
            ffi::CF_CFDP_DecodeLV(self.as_raw_mut(), &mut lv.0);
        }
    }

    /// Decodes a TLV (Type-Length-Value) tuple.
    pub fn decode_tlv(&mut self, tlv: &mut crate::cf::pdu::LogicalTlv) {
        unsafe {
            ffi::CF_CFDP_DecodeTLV(self.as_raw_mut(), &mut tlv.0);
        }
    }

    /// Decodes a segment request.
    #[allow(dead_code)]
    pub(crate) fn decode_segment_request(&mut self, sr: &mut ffi::CF_Logical_SegmentRequest_t) {
        unsafe {
            ffi::CF_CFDP_DecodeSegmentRequest(self.as_raw_mut(), sr);
        }
    }

    /// Decodes all TLVs into a list.
    pub fn decode_all_tlv(
        &mut self,
        tlv_list: &mut crate::cf::pdu::LogicalTlvList,
        limit: u8,
    ) {
        unsafe {
            ffi::CF_CFDP_DecodeAllTlv(self.as_raw_mut(), &mut tlv_list.0, limit);
        }
    }

    /// Decodes all segments into a list.
    pub fn decode_all_segments(
        &mut self,
        segment_list: &mut crate::cf::pdu::LogicalSegmentList,
        limit: u8,
    ) {
        unsafe {
            ffi::CF_CFDP_DecodeAllSegments(self.as_raw_mut(), &mut segment_list.0, limit);
        }
    }

    /// Decodes a file data header.
    pub fn decode_file_data_header(
        &mut self,
        with_meta: bool,
        fd: &mut crate::cf::pdu::LogicalPduFileData,
    ) {
        unsafe {
            ffi::CF_CFDP_DecodeFileDataHeader(self.as_raw_mut(), with_meta, &mut fd.0);
        }
    }
}
