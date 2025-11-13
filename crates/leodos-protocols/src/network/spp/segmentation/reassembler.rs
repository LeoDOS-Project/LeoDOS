use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use crate::network::spp::SpacePacket;

/// An error that can occur during the reassembly of segmented packets.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum ReassemblyError {
    /// A `Continuation` or `Last` packet was received, but no `First` packet
    /// was processed to start the sequence.
    #[error("Continuation or Last packet received before First packet")]
    ContinuationBeforeFirst,
    /// A `First` packet was received, but a reassembly for this sequence
    /// is already in progress.
    #[error("Duplicate First packet received")]
    DuplicateFirstPacket,
    /// A packet was received with a `SequenceCount` that was not the expected
    /// next value in the sequence, indicating a lost packet.
    #[error("Packet out of order: expected {expected}, got {got}")]
    PacketOutOfOrder { expected: u16, got: u16 },
    /// A `Unsegmented` packet was passed to the reassembler, which only
    /// handles segmented sequences.
    #[error("Unexpected Unsegmented packet received")]
    UnexpectedUnsegmentedPacket,
    /// The provided user buffer is too small to hold the incoming data.
    #[error("User buffer too small for reassembled data")]
    BufferTooSmall,
}

/// The state of a reassembly process.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ReassemblyState<'a> {
    /// More packets are needed to complete the data block.
    InProgress,
    /// The `Last` packet has been received and the data block is complete.
    /// Contains a slice of the user-provided buffer holding the reassembled data.
    Complete(&'a [u8]),
}

/// A stateful helper that reassembles data from segmented packets into a user-provided buffer.
///
/// This version is allocator-free. It does not own its buffer; it only borrows it mutably.
/// An application would typically manage a pool of these `Reassembler` instances.
pub struct Reassembler<'a> {
    buffer: &'a mut [u8],
    write_position: usize,
    expected_sequence_count: SequenceCount,
    is_started: bool,
}

impl<'a> Reassembler<'a> {
    /// Creates a new `Reassembler` that will write into the provided buffer.
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            buffer,
            write_position: 0,
            expected_sequence_count: SequenceCount::new(),
            is_started: false,
        }
    }

    /// Resets the reassembler to its initial state, allowing its buffer to be reused.
    pub fn reset(&mut self) {
        self.write_position = 0;
        self.is_started = false;
        // The buffer itself is not cleared for performance; it will be overwritten.
    }

    /// Processes an incoming `SpacePacket` and writes its data into the buffer.
    pub fn process_packet(
        &'a mut self,
        packet: &SpacePacket,
    ) -> Result<ReassemblyState<'a>, ReassemblyError> {
        let payload = packet.data_field();

        // Check if the payload will fit
        if self.write_position + payload.len() > self.buffer.len() {
            return Err(ReassemblyError::BufferTooSmall);
        }

        match packet.sequence_flag() {
            SequenceFlag::First => {
                if self.is_started {
                    return Err(ReassemblyError::DuplicateFirstPacket);
                }
                self.reset(); // Ensure we're in a clean state
                self.is_started = true;
                self.expected_sequence_count = packet.sequence_count();
                self.expected_sequence_count.increment();
            }
            SequenceFlag::Continuation | SequenceFlag::Last => {
                if !self.is_started {
                    return Err(ReassemblyError::ContinuationBeforeFirst);
                }
                if packet.sequence_count() != self.expected_sequence_count {
                    return Err(ReassemblyError::PacketOutOfOrder {
                        expected: self.expected_sequence_count.value(),
                        got: packet.sequence_count().value(),
                    });
                }
                self.expected_sequence_count.increment();
            }
            SequenceFlag::Unsegmented => return Err(ReassemblyError::UnexpectedUnsegmentedPacket),
        }

        // If all checks pass, write the data
        let write_end = self.write_position + payload.len();
        self.buffer[self.write_position..write_end].copy_from_slice(payload);
        self.write_position = write_end;

        if packet.sequence_flag() == SequenceFlag::Last {
            let data = &self.buffer[..self.write_position];
            Ok(ReassemblyState::Complete(data))
        } else {
            Ok(ReassemblyState::InProgress)
        }
    }
}
