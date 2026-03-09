//! Communications Link Transmission Unit (CLTU) Protocol
//!
//! Spec: https://ccsds.org/Pubs/131x0b5.pdf
//!
//! A CLTU is a highly robust data structure that wraps a TC Transfer Frame.
//! It adds error-correction coding and special sequences to ensure reliable
//! reception of commands by the spacecraft.

use crate::physical::{AsyncPhysicalWriter};

const START_SEQUENCE: &[u8] = &[0xEB, 0x90];
const TAIL_SEQUENCE: &[u8] = &[0xC5; 8];

/// An error that can occur during CLTU encoding.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CltuError {
    /// The provided output buffer is too small to hold the encoded CLTU.
    OutputBufferTooSmall {
        /// Minimum number of bytes needed for the encoded CLTU.
        required: usize,
        /// Actual size of the provided output buffer.
        provided: usize,
    },
}

/// Computes the required buffer size for encoding a TC frame of a given length into a CLTU.
///
/// This is a helper function to allow users to allocate a correctly-sized buffer
/// before calling `encode_cltu`.
pub fn encoded_cltu_len(tc_frame_len: usize) -> usize {
    let num_blocks = (tc_frame_len + 6) / 7; // Ceiling division
    START_SEQUENCE.len() + num_blocks * 8 + TAIL_SEQUENCE.len()
}

/// Encodes a TC Transfer Frame byte slice into a CLTU in the provided buffer.
///
/// This function performs two critical operations for uplink reliability:
/// 1.  It applies a (63, 56) Bose-Chaudhuri-Hocquhem (BCH) forward error-correction code,
///     which allows the spacecraft to automatically correct bit errors.
/// 2.  It wraps the encoded data with a standard Start Sequence and Tail Sequence to
///     ensure the spacecraft's radio can reliably detect the beginning and end of the command.
///
/// The provided `tc_frame_bytes` should typically be randomized before calling this function.
///
/// Returns the total number of bytes written to the output buffer.
pub fn encode_cltu(tc_frame_bytes: &[u8], output_buffer: &mut [u8]) -> Result<usize, CltuError> {
    let required_len = encoded_cltu_len(tc_frame_bytes.len());
    if output_buffer.len() < required_len {
        return Err(CltuError::OutputBufferTooSmall {
            required: required_len,
            provided: output_buffer.len(),
        });
    }

    let mut writer_idx = 0;

    // Write Start Sequence
    output_buffer[writer_idx..writer_idx + START_SEQUENCE.len()].copy_from_slice(START_SEQUENCE);
    writer_idx += START_SEQUENCE.len();

    // Write BCH-encoded blocks
    for chunk in tc_frame_bytes.chunks(7) {
        let mut block = [0x55; 7]; // Pad with alternating 01010101 pattern
        block[..chunk.len()].copy_from_slice(chunk);

        let parity = bch::compute_bch_parity(&block);

        output_buffer[writer_idx..writer_idx + 7].copy_from_slice(&block);
        writer_idx += 7;
        output_buffer[writer_idx] = parity;
        writer_idx += 1;
    }

    // Write Tail Sequence
    output_buffer[writer_idx..writer_idx + TAIL_SEQUENCE.len()].copy_from_slice(TAIL_SEQUENCE);
    writer_idx += TAIL_SEQUENCE.len();

    Ok(writer_idx)
}

mod bch {
    const fn generate_lookup_table() -> [u8; 256] {
        const CCSDS_POLYNOMIAL: u8 = 0b1011_0101; // x^7 + x^6 + x^4 + x^2 + 1 (transposed)
        let mut table = [0u8; 256];
        let mut i = 0;
        while i < 256 {
            let mut val = i as u8;
            let mut bit = 0;
            while bit < 8 {
                val = if val & 1 == 1 {
                    (val >> 1) ^ CCSDS_POLYNOMIAL
                } else {
                    val >> 1
                };
                bit += 1;
            }
            table[i] = val;
            i += 1;
        }
        table
    }

    const LOOKUP_TABLE: [u8; 256] = generate_lookup_table();

    /// Computes the (63, 56) BCH codeword as defined in CCSDS 231.0-B-4.
    pub fn compute_bch_parity(bytes: &[u8; 7]) -> u8 {
        let remainder = bytes
            .iter()
            .fold(0, |acc, &val| LOOKUP_TABLE[(acc ^ val) as usize]);
        !remainder & 0x7F
    }
}

/// Errors that can occur when writing CLTU-encoded frames.
#[derive(Debug, Clone)]
pub enum CltuWriterError<E> {
    /// A CLTU encoding error occurred.
    Cltu(CltuError),
    /// The underlying writer returned an error.
    Writer(E),
}

impl<E: core::fmt::Display> core::fmt::Display for CltuWriterError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cltu(e) => write!(f, "CLTU encoding error: {e:?}"),
            Self::Writer(e) => write!(f, "writer error: {e}"),
        }
    }
}

impl<E: core::error::Error> core::error::Error for CltuWriterError<E> {}

/// Wraps an [`AsyncPhysicalWriter`] to CLTU-encode TC frames
/// before writing.
pub struct CltuWriter<W, const BUF: usize> {
    writer: W,
    buffer: [u8; BUF],
}

impl<W, const BUF: usize> CltuWriter<W, BUF> {
    /// Creates a new CLTU writer wrapping the given writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: [0u8; BUF],
        }
    }

    /// Consumes this wrapper, returning the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: AsyncPhysicalWriter, const BUF: usize> CltuWriter<W, BUF> {
    /// Encodes a TC frame as a CLTU and writes it downstream.
    pub async fn write_frame(
        &mut self,
        tc_frame: &[u8],
    ) -> Result<(), CltuWriterError<W::Error>> {
        let required = encoded_cltu_len(tc_frame.len());
        if required > BUF {
            return Err(CltuWriterError::Cltu(
                CltuError::OutputBufferTooSmall {
                    required,
                    provided: BUF,
                },
            ));
        }

        let len = encode_cltu(tc_frame, &mut self.buffer)
            .map_err(CltuWriterError::Cltu)?;
        self.writer
            .write(&self.buffer[..len])
            .await
            .map_err(CltuWriterError::Writer)
    }
}

impl<W: AsyncPhysicalWriter, const BUF: usize> AsyncPhysicalWriter
    for CltuWriter<W, BUF>
{
    type Error = CltuWriterError<W::Error>;

    async fn write(
        &mut self,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.write_frame(data).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bch_test_vectors() {
        assert_eq!(
            bch::compute_bch_parity(&[0x22, 0xF6, 0x00, 0xFF, 0x00, 0x42, 0x1A]),
            0x4A
        );
        assert_eq!(
            bch::compute_bch_parity(&[0x8C, 0xC0, 0x0E, 0x01, 0x0D, 0x19, 0x06]),
            0x16
        );
    }

    #[test]
    fn cltu_encoding() {
        let tc_frame: &[u8] = &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let mut cltu_buffer = [0u8; 34]; // 2 (start) + 2*8 (blocks) + 8 (tail) = 26 bytes needed

        let len = encode_cltu(tc_frame, &mut cltu_buffer).unwrap();
        assert_eq!(len, 26);

        let expected_start = &cltu_buffer[0..2];
        assert_eq!(expected_start, &[0xEB, 0x90]);

        // First block: 01,02,03,04,05,06,07 -> Parity 0x1F
        assert_eq!(
            cltu_buffer[2..9],
            [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]
        );
        assert_eq!(cltu_buffer[9], 0x1F);

        // Second block: 08,55,55,55,55,55,55 -> Parity 0x47
        assert_eq!(
            cltu_buffer[10..17],
            [0x08, 0x55, 0x55, 0x55, 0x55, 0x55, 0x55]
        );
        assert_eq!(cltu_buffer[17], 0x47);

        let expected_tail = &cltu_buffer[18..26];
        assert_eq!(expected_tail, &[0xC5; 8]);
    }
}
