//! Proximity-1 Coding and Synchronization (CCSDS 211.2-B-3).
//!
//! Composes existing coding primitives into the Proximity-1 coding
//! pipeline: randomizer → convolutional FEC (rate 1/2, K=7) →
//! 24-bit ASM framing.
//!
//! The write pipeline accepts a Proximity-1 transfer frame and
//! produces a coded PLTU (Proximity Link Transmission Unit) ready
//! for GMSK modulation. The read pipeline reverses this.

use crate::coding::fec::convolutional::ConvolutionalEncoder;
use crate::coding::fec::convolutional::ViterbiDecoder;
use crate::coding::framing::cadu::AsmDeframer;
use crate::coding::framing::cadu::AsmFramer;
use crate::coding::pipeline::CodingReader;
use crate::coding::pipeline::CodingWriter;
use crate::coding::randomizer::TcRandomizer;
use crate::physical::PhysicalRead;
use crate::physical::PhysicalWrite;

/// Maximum Proximity-1 transfer frame size (2048 octets per spec).
pub const MAX_FRAME_SIZE: usize = 2048;

/// PLTU buffer size: frame (2048) × 2 (rate 1/2 FEC) + ASM (3) + margin.
const PLTU_BUF: usize = 8192;

/// Default hard-decision LLR magnitude for Viterbi decoding.
const LLR_MAG: i16 = 127;

/// Creates a Proximity-1 coding write pipeline.
///
/// Pipeline: randomizer → convolutional (rate 1/2) → 24-bit ASM → writer.
pub fn writer<W: PhysicalWrite>(
    writer: W,
) -> CodingWriter<TcRandomizer, ConvolutionalEncoder, AsmFramer, W, PLTU_BUF> {
    CodingWriter::new(
        TcRandomizer::new(),
        ConvolutionalEncoder,
        AsmFramer::proximity1(),
        writer,
    )
}

/// Creates a Proximity-1 coding read pipeline.
///
/// Pipeline: reader → 24-bit ASM sync → Viterbi → derandomizer.
pub fn reader<R: PhysicalRead>(
    reader: R,
    frame_len: usize,
) -> CodingReader<TcRandomizer, AsmDeframer, ViterbiDecoder, R, PLTU_BUF> {
    CodingReader::new(
        TcRandomizer::new(),
        AsmDeframer::proximity1(frame_len),
        ViterbiDecoder::new(LLR_MAG),
        reader,
    )
}
