//! Channel Access Data Unit (CADU) — TM Synchronization and Channel Coding
//!
//! Spec: CCSDS 131.0-B-5 (TM Synchronization and Channel Coding)
//!
//! A CADU is the unit produced by the Coding & Synchronization sublayer
//! on the downlink (TM/AOS direction). It consists of:
//!
//! ```text
//! ┌──────────┬────────────────────────┐
//! │  ASM     │  Transfer Frame        │
//! │ (4 bytes)│  (fixed-length)        │
//! └──────────┴────────────────────────┘
//! ```
//!
//! The **Attached Sync Marker (ASM)** is a fixed 32-bit pattern that
//! the receiver uses to locate frame boundaries in the continuous
//! bitstream. The standard ASM for TM/AOS is `0x1ACFFC1D`.
//!
//! Proximity-1 uses a 24-bit ASM (`0xFAF320`) as defined in
//! CCSDS 211.2-B-3.
//!
//! This module provides:
//! - ASM constants for TM, AOS, and Proximity-1
//! - CADU encoding (prepend ASM to a frame)
//! - Frame synchronization (find ASM in a bitstream)

use crate::physical::{PhysicalReader, PhysicalWriter};

/// Standard 32-bit ASM for TM and AOS frames (CCSDS 131.0-B-5).
pub const ASM_TM: [u8; 4] = [0x1A, 0xCF, 0xFC, 0x1D];

/// Inverted 32-bit ASM used for the odd frames when Convolutional
/// coding with ambiguity resolution is employed.
pub const ASM_TM_INVERTED: [u8; 4] = [0xE5, 0x30, 0x03, 0xE2];

/// 24-bit ASM for Proximity-1 links (CCSDS 211.2-B-3).
pub const ASM_PROXIMITY1: [u8; 3] = [0xFA, 0xF3, 0x20];

/// Errors that can occur during CADU operations.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CaduError {
    /// The output buffer is too small for the CADU.
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual buffer size provided.
        provided: usize,
    },
    /// The input is too short to contain an ASM and frame.
    InputTooShort,
    /// The expected ASM was not found at the start of the data.
    AsmMismatch,
}

impl core::fmt::Display for CaduError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall { required, provided } => {
                write!(f, "buffer too small: need {required}, have {provided}")
            }
            Self::InputTooShort => write!(f, "input too short"),
            Self::AsmMismatch => write!(f, "ASM mismatch"),
        }
    }
}

impl core::error::Error for CaduError {}

/// Encodes a transfer frame into a CADU by prepending the ASM.
///
/// Writes `asm | frame` into `output` and returns the total bytes
/// written.
pub fn encode_cadu(
    asm: &[u8],
    frame: &[u8],
    output: &mut [u8],
) -> Result<usize, CaduError> {
    let total = asm.len() + frame.len();
    if output.len() < total {
        return Err(CaduError::BufferTooSmall {
            required: total,
            provided: output.len(),
        });
    }
    output[..asm.len()].copy_from_slice(asm);
    output[asm.len()..total].copy_from_slice(frame);
    Ok(total)
}

/// Strips the ASM from a CADU and returns the frame payload.
///
/// Verifies that the leading bytes match the expected `asm` pattern.
pub fn decode_cadu<'a>(
    asm: &[u8],
    cadu: &'a [u8],
) -> Result<&'a [u8], CaduError> {
    if cadu.len() < asm.len() {
        return Err(CaduError::InputTooShort);
    }
    if &cadu[..asm.len()] != asm {
        return Err(CaduError::AsmMismatch);
    }
    Ok(&cadu[asm.len()..])
}

/// A frame synchronizer that searches for ASM patterns in a byte
/// stream to locate frame boundaries.
///
/// This implements a simple byte-aligned ASM search suitable for
/// simulation. A real receiver would do bit-level correlation with
/// an allowable bit-error threshold.
pub struct FrameSync<'a> {
    asm: &'a [u8],
    frame_len: usize,
}

impl<'a> FrameSync<'a> {
    /// Creates a new frame synchronizer.
    ///
    /// - `asm`: the sync marker pattern to search for
    /// - `frame_len`: expected frame length *excluding* the ASM
    pub fn new(asm: &'a [u8], frame_len: usize) -> Self {
        Self { asm, frame_len }
    }

    /// Returns the total CADU length (ASM + frame).
    pub fn cadu_len(&self) -> usize {
        self.asm.len() + self.frame_len
    }

    /// Searches `data` for the next ASM-aligned frame.
    ///
    /// Returns `Some((offset, frame))` where `offset` is the byte
    /// position of the ASM in `data` and `frame` is the frame
    /// payload (after the ASM). Returns `None` if no complete frame
    /// is found.
    pub fn find_frame<'b>(
        &self,
        data: &'b [u8],
    ) -> Option<(usize, &'b [u8])> {
        let cadu_len = self.cadu_len();
        if data.len() < cadu_len {
            return None;
        }

        let search_end = data.len() - cadu_len + 1;
        for offset in 0..search_end {
            if &data[offset..offset + self.asm.len()] == self.asm {
                let frame_start = offset + self.asm.len();
                let frame_end = frame_start + self.frame_len;
                return Some((offset, &data[frame_start..frame_end]));
            }
        }
        None
    }

    /// Finds all ASM-aligned frames in `data`.
    ///
    /// Returns an iterator of `(offset, frame_slice)` pairs.
    /// Frames may overlap if the data contains spurious ASM
    /// matches; callers should validate frame contents.
    pub fn find_all_frames<'b>(
        &'b self,
        data: &'b [u8],
    ) -> FrameIter<'b> {
        FrameIter {
            sync: self,
            data,
            pos: 0,
        }
    }
}

/// Iterator over frames found by [`FrameSync::find_all_frames`].
pub struct FrameIter<'a> {
    sync: &'a FrameSync<'a>,
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = (usize, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        let remaining = &self.data[self.pos..];
        let result = self.sync.find_frame(remaining);
        if let Some((rel_offset, frame)) = result {
            let abs_offset = self.pos + rel_offset;
            // Advance past this ASM to avoid finding it again
            self.pos = abs_offset + self.sync.asm.len();
            Some((abs_offset, frame))
        } else {
            None
        }
    }
}

/// ASM framer implementing [`Framer`](crate::coding::Framer).
pub struct AsmFramer {
    asm: &'static [u8],
}

impl AsmFramer {
    /// Creates a TM/AOS ASM framer.
    pub fn tm() -> Self {
        Self { asm: &ASM_TM }
    }

    /// Creates a Proximity-1 ASM framer.
    pub fn proximity1() -> Self {
        Self { asm: &ASM_PROXIMITY1 }
    }
}

impl crate::coding::Framer for AsmFramer {
    type Error = CaduError;

    fn frame(&self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        encode_cadu(self.asm, data, output)
    }
}

/// ASM deframer implementing [`Deframer`](crate::coding::Deframer).
pub struct AsmDeframer {
    asm: &'static [u8],
    frame_len: usize,
}

impl AsmDeframer {
    /// Creates a TM/AOS ASM deframer for the given frame length.
    pub fn tm(frame_len: usize) -> Self {
        Self { asm: &ASM_TM, frame_len }
    }

    /// Creates a Proximity-1 ASM deframer for the given frame length.
    pub fn proximity1(frame_len: usize) -> Self {
        Self { asm: &ASM_PROXIMITY1, frame_len }
    }
}

impl crate::coding::Deframer for AsmDeframer {
    type Error = CaduError;

    fn deframe<'a>(&self, data: &'a [u8], output: &mut [u8]) -> Result<usize, Self::Error> {
        let sync = FrameSync::new(self.asm, self.frame_len);
        let (_offset, frame) = sync.find_frame(data).ok_or(CaduError::AsmMismatch)?;
        let len = frame.len().min(output.len());
        output[..len].copy_from_slice(&frame[..len]);
        Ok(len)
    }
}

/// Wraps an [`PhysicalWriter`] to prepend an ASM before writing.
pub struct AsmWriter<W, const BUF: usize> {
    writer: W,
    asm: &'static [u8],
    buffer: [u8; BUF],
}

/// Errors from ASM writer operations.
#[derive(Debug, Clone)]
pub enum AsmWriterError<E> {
    /// CADU encoding error.
    Cadu(CaduError),
    /// The underlying writer returned an error.
    Writer(E),
}

impl<E: core::fmt::Display> core::fmt::Display for AsmWriterError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cadu(e) => write!(f, "ASM: {e:?}"),
            Self::Writer(e) => write!(f, "writer: {e}"),
        }
    }
}

impl<E: core::error::Error> core::error::Error for AsmWriterError<E> {}

impl<W, const BUF: usize> AsmWriter<W, BUF> {
    /// Creates a writer that prepends the TM ASM (`1ACFFC1D`).
    pub fn tm(writer: W) -> Self {
        Self {
            writer,
            asm: &ASM_TM,
            buffer: [0u8; BUF],
        }
    }

    /// Creates a writer that prepends the Proximity-1 ASM.
    pub fn proximity1(writer: W) -> Self {
        Self {
            writer,
            asm: &ASM_PROXIMITY1,
            buffer: [0u8; BUF],
        }
    }
}

impl<W: PhysicalWriter, const BUF: usize> PhysicalWriter
    for AsmWriter<W, BUF>
{
    type Error = AsmWriterError<W::Error>;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        let len = encode_cadu(self.asm, data, &mut self.buffer)
            .map_err(AsmWriterError::Cadu)?;
        self.writer
            .write(&self.buffer[..len])
            .await
            .map_err(AsmWriterError::Writer)
    }
}

/// Wraps an [`PhysicalReader`] to find and strip ASM from
/// incoming data.
pub struct FrameSyncReader<R, const BUF: usize> {
    reader: R,
    asm: &'static [u8],
    frame_len: usize,
    buffer: [u8; BUF],
}

/// Errors from frame sync reader operations.
#[derive(Debug, Clone)]
pub enum FrameSyncReaderError<E> {
    /// No valid frame found in received data.
    NoFrame,
    /// The underlying reader returned an error.
    Reader(E),
}

impl<E: core::fmt::Display> core::fmt::Display
    for FrameSyncReaderError<E>
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoFrame => write!(f, "no ASM-aligned frame found"),
            Self::Reader(e) => write!(f, "reader: {e}"),
        }
    }
}

impl<E: core::error::Error> core::error::Error
    for FrameSyncReaderError<E>
{
}

impl<R, const BUF: usize> FrameSyncReader<R, BUF> {
    /// Creates a reader that searches for TM ASM and extracts
    /// frames of the given length.
    pub fn tm(reader: R, frame_len: usize) -> Self {
        Self {
            reader,
            asm: &ASM_TM,
            frame_len,
            buffer: [0u8; BUF],
        }
    }

    /// Creates a reader for Proximity-1 ASM.
    pub fn proximity1(reader: R, frame_len: usize) -> Self {
        Self {
            reader,
            asm: &ASM_PROXIMITY1,
            frame_len,
            buffer: [0u8; BUF],
        }
    }
}

impl<R: PhysicalReader, const BUF: usize> PhysicalReader
    for FrameSyncReader<R, BUF>
{
    type Error = FrameSyncReaderError<R::Error>;

    async fn read(
        &mut self,
        output: &mut [u8],
    ) -> Result<usize, Self::Error> {
        let len = self
            .reader
            .read(&mut self.buffer)
            .await
            .map_err(FrameSyncReaderError::Reader)?;

        let sync = FrameSync::new(self.asm, self.frame_len);
        let Some((_offset, frame)) =
            sync.find_frame(&self.buffer[..len])
        else {
            return Err(FrameSyncReaderError::NoFrame);
        };

        let copy_len = frame.len().min(output.len());
        output[..copy_len].copy_from_slice(&frame[..copy_len]);
        Ok(copy_len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_tm_cadu() {
        let frame = [0xAA; 16];
        let mut buf = [0u8; 20];
        let len = encode_cadu(&ASM_TM, &frame, &mut buf).unwrap();

        assert_eq!(len, 20);
        assert_eq!(&buf[..4], &ASM_TM);
        assert_eq!(&buf[4..20], &frame);
    }

    #[test]
    fn encode_proximity1_cadu() {
        let frame = [0xBB; 10];
        let mut buf = [0u8; 13];
        let len =
            encode_cadu(&ASM_PROXIMITY1, &frame, &mut buf).unwrap();

        assert_eq!(len, 13);
        assert_eq!(&buf[..3], &ASM_PROXIMITY1);
        assert_eq!(&buf[3..13], &frame);
    }

    #[test]
    fn encode_buffer_too_small() {
        let frame = [0u8; 16];
        let mut buf = [0u8; 10]; // need 20
        let err = encode_cadu(&ASM_TM, &frame, &mut buf);
        assert!(matches!(
            err,
            Err(CaduError::BufferTooSmall {
                required: 20,
                provided: 10,
            })
        ));
    }

    #[test]
    fn decode_tm_cadu() {
        let mut cadu = [0u8; 20];
        cadu[..4].copy_from_slice(&ASM_TM);
        cadu[4..].fill(0xCC);

        let frame = decode_cadu(&ASM_TM, &cadu).unwrap();
        assert_eq!(frame.len(), 16);
        assert!(frame.iter().all(|&b| b == 0xCC));
    }

    #[test]
    fn decode_asm_mismatch() {
        let cadu = [0u8; 20]; // all zeros, not ASM_TM
        let err = decode_cadu(&ASM_TM, &cadu);
        assert!(matches!(err, Err(CaduError::AsmMismatch)));
    }

    #[test]
    fn decode_input_too_short() {
        let cadu = [0x1A, 0xCF]; // only 2 bytes
        let err = decode_cadu(&ASM_TM, &cadu);
        assert!(matches!(err, Err(CaduError::InputTooShort)));
    }

    #[test]
    fn frame_sync_find_single() {
        let frame_len = 8;
        let sync = FrameSync::new(&ASM_TM, frame_len);

        // Build: garbage + ASM + frame data
        let mut data = [0u8; 32];
        data[5..9].copy_from_slice(&ASM_TM);
        data[9..17].fill(0xDD);

        let (offset, frame) = sync.find_frame(&data).unwrap();
        assert_eq!(offset, 5);
        assert_eq!(frame.len(), 8);
        assert!(frame.iter().all(|&b| b == 0xDD));
    }

    #[test]
    fn frame_sync_find_multiple() {
        let frame_len = 4;
        let sync = FrameSync::new(&ASM_TM, frame_len);
        let cadu_len = sync.cadu_len(); // 8

        // Two back-to-back CADUs
        let mut data = [0u8; 16];
        // First CADU at offset 0
        data[0..4].copy_from_slice(&ASM_TM);
        data[4..8].fill(0x11);
        // Second CADU at offset 8
        data[8..12].copy_from_slice(&ASM_TM);
        data[12..16].fill(0x22);

        let frames: heapless::Vec<(usize, &[u8]), 4> =
            sync.find_all_frames(&data).collect();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].0, 0);
        assert!(frames[0].1.iter().all(|&b| b == 0x11));
        assert_eq!(frames[1].0, cadu_len);
        assert!(frames[1].1.iter().all(|&b| b == 0x22));
    }

    #[test]
    fn frame_sync_no_match() {
        let sync = FrameSync::new(&ASM_TM, 8);
        let data = [0u8; 32]; // no ASM present
        assert!(sync.find_frame(&data).is_none());
    }

    #[test]
    fn frame_sync_incomplete_frame() {
        let sync = FrameSync::new(&ASM_TM, 100);
        // ASM present but not enough data for full frame
        let mut data = [0u8; 20];
        data[0..4].copy_from_slice(&ASM_TM);
        assert!(sync.find_frame(&data).is_none());
    }

    #[test]
    fn roundtrip_encode_decode() {
        let frame = [0x42; 64];
        let mut cadu_buf = [0u8; 68];

        let len =
            encode_cadu(&ASM_TM, &frame, &mut cadu_buf).unwrap();
        let decoded = decode_cadu(&ASM_TM, &cadu_buf[..len]).unwrap();

        assert_eq!(decoded, &frame);
    }

    #[test]
    fn proximity1_roundtrip() {
        let frame = [0x77; 32];
        let mut cadu_buf = [0u8; 35];

        let len = encode_cadu(&ASM_PROXIMITY1, &frame, &mut cadu_buf)
            .unwrap();
        let decoded =
            decode_cadu(&ASM_PROXIMITY1, &cadu_buf[..len]).unwrap();

        assert_eq!(decoded, &frame);
    }
}
