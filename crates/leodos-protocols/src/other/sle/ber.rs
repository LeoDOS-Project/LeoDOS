//! Minimal ASN.1 BER encoder/decoder for SLE.
//!
//! Implements only the subset needed by SLE: INTEGER, OCTET STRING,
//! SEQUENCE, CHOICE, ENUMERATED, NULL, BIT STRING, BOOLEAN.
//! No heap allocation — all operations work on caller-provided
//! byte slices.

use super::types::SleError;

/// ASN.1 tag class.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Class {
    /// Universal (built-in ASN.1 types).
    Universal = 0,
    /// Application-specific.
    Application = 1,
    /// Context-specific (used in CHOICE / tagged fields).
    Context = 2,
    /// Private.
    Private = 3,
}

impl Class {
    fn from_bits(bits: u8) -> Self {
        match bits {
            0 => Self::Universal,
            1 => Self::Application,
            2 => Self::Context,
            _ => Self::Private,
        }
    }
}

/// Well-known universal tag numbers.
pub mod tags {
    /// BOOLEAN
    pub const BOOLEAN: u8 = 1;
    /// INTEGER
    pub const INTEGER: u8 = 2;
    /// BIT STRING
    pub const BIT_STRING: u8 = 3;
    /// OCTET STRING
    pub const OCTET_STRING: u8 = 4;
    /// NULL
    pub const NULL: u8 = 5;
    /// ENUMERATED
    pub const ENUMERATED: u8 = 10;
    /// SEQUENCE / SEQUENCE OF
    pub const SEQUENCE: u8 = 16;
}

/// Encodes a single-byte tag octet.
///
/// Only supports tag numbers 0..30 (short form). Returns the
/// encoded tag byte.
pub fn encode_tag(tag: u8, class: Class, constructed: bool) -> u8 {
    let class_bits = (class as u8) << 6;
    let constructed_bit = if constructed { 0x20 } else { 0x00 };
    class_bits | constructed_bit | (tag & 0x1F)
}

/// Encodes a BER length into `buf`. Returns number of bytes written.
///
/// Short form: lengths 0..127 use one byte.
/// Long form: lengths 128.. use 1 byte for count + N value bytes.
pub fn encode_length(len: usize, buf: &mut [u8]) -> Result<usize, SleError> {
    if len < 128 {
        if buf.is_empty() {
            return Err(SleError::BufferTooSmall);
        }
        buf[0] = len as u8;
        Ok(1)
    } else if len <= 0xFF {
        if buf.len() < 2 {
            return Err(SleError::BufferTooSmall);
        }
        buf[0] = 0x81;
        buf[1] = len as u8;
        Ok(2)
    } else if len <= 0xFFFF {
        if buf.len() < 3 {
            return Err(SleError::BufferTooSmall);
        }
        buf[0] = 0x82;
        buf[1] = (len >> 8) as u8;
        buf[2] = len as u8;
        Ok(3)
    } else if len <= 0xFF_FFFF {
        if buf.len() < 4 {
            return Err(SleError::BufferTooSmall);
        }
        buf[0] = 0x83;
        buf[1] = (len >> 16) as u8;
        buf[2] = (len >> 8) as u8;
        buf[3] = len as u8;
        Ok(4)
    } else {
        if buf.len() < 5 {
            return Err(SleError::BufferTooSmall);
        }
        buf[0] = 0x84;
        buf[1] = (len >> 24) as u8;
        buf[2] = (len >> 16) as u8;
        buf[3] = (len >> 8) as u8;
        buf[4] = len as u8;
        Ok(5)
    }
}

/// Decodes a BER tag byte.
///
/// Returns `(tag_number, class, constructed, bytes_consumed)`.
/// Only supports short-form tags (tag number 0..30).
pub fn decode_tag(
    buf: &[u8],
) -> Result<(u8, Class, bool, usize), SleError> {
    if buf.is_empty() {
        return Err(SleError::Truncated);
    }
    let b = buf[0];
    let class = Class::from_bits(b >> 6);
    let constructed = (b & 0x20) != 0;
    let tag = b & 0x1F;
    if tag == 0x1F {
        return Err(SleError::UnexpectedTag);
    }
    Ok((tag, class, constructed, 1))
}

/// Decodes a BER length field.
///
/// Returns `(length_value, bytes_consumed)`.
pub fn decode_length(buf: &[u8]) -> Result<(usize, usize), SleError> {
    if buf.is_empty() {
        return Err(SleError::Truncated);
    }
    let first = buf[0];
    if first < 128 {
        Ok((first as usize, 1))
    } else {
        let num_bytes = (first & 0x7F) as usize;
        if num_bytes == 0 || num_bytes > 4 {
            return Err(SleError::Truncated);
        }
        if buf.len() < 1 + num_bytes {
            return Err(SleError::Truncated);
        }
        let mut val: usize = 0;
        for i in 0..num_bytes {
            val = (val << 8) | buf[1 + i] as usize;
        }
        Ok((val, 1 + num_bytes))
    }
}

/// A BER writer that encodes TLV elements into a byte slice.
pub struct BerWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> BerWriter<'a> {
    /// Creates a new writer over the given buffer.
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Returns the number of bytes written so far.
    pub fn len(&self) -> usize {
        self.pos
    }

    /// Returns true if no bytes have been written.
    pub fn is_empty(&self) -> bool {
        self.pos == 0
    }

    /// Returns the bytes written so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.pos]
    }

    /// Returns the remaining writable capacity.
    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn write_byte(&mut self, b: u8) -> Result<(), SleError> {
        if self.remaining() == 0 {
            return Err(SleError::BufferTooSmall);
        }
        self.buf[self.pos] = b;
        self.pos += 1;
        Ok(())
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<(), SleError> {
        if self.remaining() < data.len() {
            return Err(SleError::BufferTooSmall);
        }
        self.buf[self.pos..self.pos + data.len()].copy_from_slice(data);
        self.pos += data.len();
        Ok(())
    }

    fn write_tag(
        &mut self,
        tag: u8,
        class: Class,
        constructed: bool,
    ) -> Result<(), SleError> {
        self.write_byte(encode_tag(tag, class, constructed))
    }

    fn write_length(&mut self, len: usize) -> Result<(), SleError> {
        let n = encode_length(len, &mut self.buf[self.pos..])?;
        self.pos += n;
        Ok(())
    }

    /// Writes an ASN.1 BOOLEAN.
    pub fn write_bool(&mut self, value: bool) -> Result<(), SleError> {
        self.write_tag(tags::BOOLEAN, Class::Universal, false)?;
        self.write_length(1)?;
        self.write_byte(if value { 0xFF } else { 0x00 })
    }

    /// Writes an ASN.1 INTEGER (signed, variable length).
    pub fn write_integer(&mut self, value: i64) -> Result<(), SleError> {
        self.write_tag(tags::INTEGER, Class::Universal, false)?;
        let encoded = encode_i64(value);
        self.write_length(encoded.len)?;
        self.write_bytes(&encoded.bytes[..encoded.len])
    }

    /// Writes an ASN.1 OCTET STRING.
    pub fn write_octet_string(
        &mut self,
        data: &[u8],
    ) -> Result<(), SleError> {
        self.write_tag(tags::OCTET_STRING, Class::Universal, false)?;
        self.write_length(data.len())?;
        self.write_bytes(data)
    }

    /// Writes an ASN.1 ENUMERATED value.
    pub fn write_enum(&mut self, value: i64) -> Result<(), SleError> {
        self.write_tag(tags::ENUMERATED, Class::Universal, false)?;
        let encoded = encode_i64(value);
        self.write_length(encoded.len)?;
        self.write_bytes(&encoded.bytes[..encoded.len])
    }

    /// Writes an ASN.1 NULL.
    pub fn write_null(&mut self) -> Result<(), SleError> {
        self.write_tag(tags::NULL, Class::Universal, false)?;
        self.write_length(0)
    }

    /// Writes an ASN.1 BIT STRING with zero unused bits.
    pub fn write_bit_string(
        &mut self,
        data: &[u8],
    ) -> Result<(), SleError> {
        self.write_tag(tags::BIT_STRING, Class::Universal, false)?;
        self.write_length(data.len() + 1)?;
        self.write_byte(0)?; // unused bits = 0
        self.write_bytes(data)
    }

    /// Begins a SEQUENCE. Returns the position of the length field
    /// so it can be patched later with `end_sequence`.
    pub fn begin_sequence(&mut self) -> Result<SeqStart, SleError> {
        self.write_tag(tags::SEQUENCE, Class::Universal, true)?;
        let len_pos = self.pos;
        // Reserve space for a 3-byte length (0x82 + 2 bytes).
        // This supports sequences up to 65535 bytes.
        if self.remaining() < 3 {
            return Err(SleError::BufferTooSmall);
        }
        self.pos += 3;
        Ok(SeqStart {
            len_pos,
            content_start: self.pos,
        })
    }

    /// Ends a SEQUENCE started with `begin_sequence`, patching the
    /// length field.
    pub fn end_sequence(
        &mut self,
        start: SeqStart,
    ) -> Result<(), SleError> {
        let content_len = self.pos - start.content_start;
        if content_len < 128 {
            // Shift content left by 2 bytes (we reserved 3, need 1).
            let src = start.content_start;
            let dst = start.len_pos + 1;
            self.buf.copy_within(src..self.pos, dst);
            self.buf[start.len_pos] = content_len as u8;
            self.pos -= 2;
        } else if content_len <= 0xFF {
            // Shift content left by 1 byte (we reserved 3, need 2).
            let src = start.content_start;
            let dst = start.len_pos + 2;
            self.buf.copy_within(src..self.pos, dst);
            self.buf[start.len_pos] = 0x81;
            self.buf[start.len_pos + 1] = content_len as u8;
            self.pos -= 1;
        } else {
            // Exact fit: 3-byte length encoding.
            self.buf[start.len_pos] = 0x82;
            self.buf[start.len_pos + 1] = (content_len >> 8) as u8;
            self.buf[start.len_pos + 2] = content_len as u8;
        }
        Ok(())
    }

    /// Writes a context-tagged constructed wrapper (implicit tag).
    /// Returns a SeqStart for use with `end_sequence`.
    pub fn begin_context(
        &mut self,
        tag: u8,
        constructed: bool,
    ) -> Result<SeqStart, SleError> {
        self.write_tag(tag, Class::Context, constructed)?;
        let len_pos = self.pos;
        if self.remaining() < 3 {
            return Err(SleError::BufferTooSmall);
        }
        self.pos += 3;
        Ok(SeqStart {
            len_pos,
            content_start: self.pos,
        })
    }
}

/// Bookkeeping for an in-progress SEQUENCE or tagged wrapper.
#[derive(Copy, Clone, Debug)]
pub struct SeqStart {
    len_pos: usize,
    content_start: usize,
}

/// A BER reader that decodes TLV elements from a byte slice.
pub struct BerReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> BerReader<'a> {
    /// Creates a new reader over the given buffer.
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Returns the current read position.
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Returns the remaining unread bytes.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Returns true if all bytes have been consumed.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Peeks at the next tag without advancing the position.
    /// Returns `(tag_number, class, constructed)`.
    pub fn peek_tag(
        &self,
    ) -> Result<(u8, Class, bool), SleError> {
        let (tag, class, constructed, _) =
            decode_tag(&self.buf[self.pos..])?;
        Ok((tag, class, constructed))
    }

    /// Reads and validates a tag, returning its components.
    pub fn read_tag(
        &mut self,
    ) -> Result<(u8, Class, bool), SleError> {
        let (tag, class, constructed, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        self.pos += consumed;
        Ok((tag, class, constructed))
    }

    /// Reads a length field.
    pub fn read_length(&mut self) -> Result<usize, SleError> {
        let (len, consumed) =
            decode_length(&self.buf[self.pos..])?;
        self.pos += consumed;
        Ok(len)
    }

    /// Reads raw bytes of the given length.
    pub fn read_raw(
        &mut self,
        len: usize,
    ) -> Result<&'a [u8], SleError> {
        if self.remaining() < len {
            return Err(SleError::Truncated);
        }
        let data = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(data)
    }

    /// Reads an ASN.1 BOOLEAN.
    pub fn read_bool(&mut self) -> Result<bool, SleError> {
        let (tag, class, _, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if tag != tags::BOOLEAN || class != Class::Universal {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        let len = self.read_length()?;
        if len != 1 {
            return Err(SleError::Truncated);
        }
        let val = self.buf[self.pos];
        self.pos += 1;
        Ok(val != 0)
    }

    /// Reads an ASN.1 INTEGER as i64.
    pub fn read_integer(&mut self) -> Result<i64, SleError> {
        let (tag, class, _, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if tag != tags::INTEGER || class != Class::Universal {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        let len = self.read_length()?;
        if len == 0 || len > 8 {
            return Err(SleError::IntegerOverflow);
        }
        let data = self.read_raw(len)?;
        Ok(decode_i64(data))
    }

    /// Reads an ASN.1 OCTET STRING, returning the raw bytes.
    pub fn read_octet_string(
        &mut self,
    ) -> Result<&'a [u8], SleError> {
        let (tag, class, _, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if tag != tags::OCTET_STRING || class != Class::Universal {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        let len = self.read_length()?;
        self.read_raw(len)
    }

    /// Reads an ASN.1 ENUMERATED as i64.
    pub fn read_enum(&mut self) -> Result<i64, SleError> {
        let (tag, class, _, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if tag != tags::ENUMERATED || class != Class::Universal {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        let len = self.read_length()?;
        if len == 0 || len > 8 {
            return Err(SleError::IntegerOverflow);
        }
        let data = self.read_raw(len)?;
        Ok(decode_i64(data))
    }

    /// Reads an ASN.1 NULL.
    pub fn read_null(&mut self) -> Result<(), SleError> {
        let (tag, class, _, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if tag != tags::NULL || class != Class::Universal {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        let len = self.read_length()?;
        if len != 0 {
            return Err(SleError::Truncated);
        }
        Ok(())
    }

    /// Reads a SEQUENCE tag+length, returning the content length.
    /// The caller should then read the contained elements.
    pub fn read_sequence(&mut self) -> Result<usize, SleError> {
        let (tag, class, constructed, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if tag != tags::SEQUENCE
            || class != Class::Universal
            || !constructed
        {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        self.read_length()
    }

    /// Reads a context-tagged wrapper, returning the tag number
    /// and content length.
    pub fn read_context_tag(
        &mut self,
    ) -> Result<(u8, usize), SleError> {
        let (tag, class, _, consumed) =
            decode_tag(&self.buf[self.pos..])?;
        if class != Class::Context {
            return Err(SleError::UnexpectedTag);
        }
        self.pos += consumed;
        let len = self.read_length()?;
        Ok((tag, len))
    }

    /// Creates a sub-reader limited to `len` bytes from the
    /// current position, advancing past them.
    pub fn sub_reader(
        &mut self,
        len: usize,
    ) -> Result<BerReader<'a>, SleError> {
        if self.remaining() < len {
            return Err(SleError::Truncated);
        }
        let sub = BerReader::new(
            &self.buf[self.pos..self.pos + len],
        );
        self.pos += len;
        Ok(sub)
    }
}

/// Encoded integer bytes + length.
struct EncodedInt {
    bytes: [u8; 8],
    len: usize,
}

/// Encodes a signed i64 into the minimum BER integer bytes.
fn encode_i64(value: i64) -> EncodedInt {
    let raw = value.to_be_bytes();
    let mut start = 0;
    if value >= 0 {
        while start < 7 && raw[start] == 0 && raw[start + 1] < 0x80
        {
            start += 1;
        }
    } else {
        while start < 7
            && raw[start] == 0xFF
            && raw[start + 1] >= 0x80
        {
            start += 1;
        }
    }
    let len = 8 - start;
    let mut bytes = [0u8; 8];
    bytes[..len].copy_from_slice(&raw[start..]);
    EncodedInt { bytes, len }
}

/// Decodes a BER integer from big-endian bytes into i64.
fn decode_i64(data: &[u8]) -> i64 {
    let negative = !data.is_empty() && data[0] & 0x80 != 0;
    let mut val: i64 = if negative { -1 } else { 0 };
    for &b in data {
        val = (val << 8) | b as i64;
    }
    val
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_integer() {
        for &v in &[0i64, 1, -1, 127, 128, -128, -129, 256,
                     i64::MAX, i64::MIN, 0x7FFF, -32768] {
            let mut buf = [0u8; 32];
            let mut w = BerWriter::new(&mut buf);
            w.write_integer(v).unwrap();
            let mut r = BerReader::new(w.as_bytes());
            let got = r.read_integer().unwrap();
            assert_eq!(got, v, "failed for {v}");
        }
    }

    #[test]
    fn roundtrip_bool() {
        let mut buf = [0u8; 16];
        let mut w = BerWriter::new(&mut buf);
        w.write_bool(true).unwrap();
        w.write_bool(false).unwrap();
        let mut r = BerReader::new(w.as_bytes());
        assert!(r.read_bool().unwrap());
        assert!(!r.read_bool().unwrap());
    }

    #[test]
    fn roundtrip_octet_string() {
        let data = b"hello SLE";
        let mut buf = [0u8; 32];
        let mut w = BerWriter::new(&mut buf);
        w.write_octet_string(data).unwrap();
        let mut r = BerReader::new(w.as_bytes());
        let got = r.read_octet_string().unwrap();
        assert_eq!(got, data);
    }

    #[test]
    fn roundtrip_enum() {
        let mut buf = [0u8; 16];
        let mut w = BerWriter::new(&mut buf);
        w.write_enum(2).unwrap();
        let mut r = BerReader::new(w.as_bytes());
        assert_eq!(r.read_enum().unwrap(), 2);
    }

    #[test]
    fn roundtrip_null() {
        let mut buf = [0u8; 8];
        let mut w = BerWriter::new(&mut buf);
        w.write_null().unwrap();
        let mut r = BerReader::new(w.as_bytes());
        r.read_null().unwrap();
    }

    #[test]
    fn roundtrip_sequence() {
        let mut buf = [0u8; 64];
        let mut w = BerWriter::new(&mut buf);
        let seq = w.begin_sequence().unwrap();
        w.write_integer(42).unwrap();
        w.write_octet_string(b"test").unwrap();
        w.end_sequence(seq).unwrap();

        let mut r = BerReader::new(w.as_bytes());
        let _seq_len = r.read_sequence().unwrap();
        assert_eq!(r.read_integer().unwrap(), 42);
        assert_eq!(r.read_octet_string().unwrap(), b"test");
    }

    #[test]
    fn encode_decode_length_short() {
        let mut buf = [0u8; 8];
        let n = encode_length(42, &mut buf).unwrap();
        assert_eq!(n, 1);
        let (val, consumed) = decode_length(&buf).unwrap();
        assert_eq!(val, 42);
        assert_eq!(consumed, 1);
    }

    #[test]
    fn encode_decode_length_long() {
        let mut buf = [0u8; 8];
        let n = encode_length(300, &mut buf).unwrap();
        assert_eq!(n, 3);
        let (val, consumed) = decode_length(&buf).unwrap();
        assert_eq!(val, 300);
        assert_eq!(consumed, 3);
    }

    #[test]
    fn peek_tag_does_not_advance() {
        let mut buf = [0u8; 8];
        let mut w = BerWriter::new(&mut buf);
        w.write_integer(7).unwrap();
        let r = BerReader::new(w.as_bytes());
        let (tag, class, _) = r.peek_tag().unwrap();
        assert_eq!(tag, tags::INTEGER);
        assert_eq!(class, Class::Universal);
        assert_eq!(r.pos(), 0);
    }
}
