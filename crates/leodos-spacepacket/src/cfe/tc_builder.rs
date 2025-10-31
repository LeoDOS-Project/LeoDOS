use crate::Apid;
use crate::PacketSequenceCount;
use crate::SpacePacketData;
use crate::builder::Vacant;
use crate::cfe::tc::Telecommand;
use crate::cfe::tc::TelecommandError;

/// A builder specialized for constructing a CFE Command Packet.
/// This builder follows the typestate pattern to ensure all required fields are provided.
pub struct TelecommandBuilder<A, B, C, D> {
    apid: A,
    sequence_count: PacketSequenceCount,
    function_code: B,
    buffer: C,
    payload: D,
}

impl TelecommandBuilder<Vacant, Vacant, Vacant, Vacant> {
    /// Creates a new builder for a CFE command.
    pub fn new() -> Self {
        Self {
            apid: Vacant,
            sequence_count: PacketSequenceCount::new(),
            function_code: Vacant,
            buffer: Vacant,
            payload: Vacant,
        }
    }
}

impl<A, B, C, D> TelecommandBuilder<A, B, C, D> {
    /// Overrides the default sequence count (0).
    pub fn sequence_count(mut self, count: PacketSequenceCount) -> Self {
        self.sequence_count = count;
        self
    }
}

impl<A, B, D> TelecommandBuilder<A, B, Vacant, D> {
    /// Provides the buffer where the packet will be built. This is a required step.
    pub fn buffer<'a>(self, buffer: &'a mut [u8]) -> TelecommandBuilder<A, B, &'a mut [u8], D> {
        TelecommandBuilder {
            apid: self.apid,
            sequence_count: self.sequence_count,
            function_code: self.function_code,
            buffer,
            payload: self.payload,
        }
    }
}

impl<A, C, D> TelecommandBuilder<A, Vacant, C, D> {
    /// Provides the function code for the command. This is a required step.
    pub fn function_code(self, function_code: u8) -> TelecommandBuilder<A, u8, C, D> {
        TelecommandBuilder {
            apid: self.apid,
            sequence_count: self.sequence_count,
            function_code,
            buffer: self.buffer,
            payload: self.payload,
        }
    }
}

impl<B, C, D> TelecommandBuilder<Vacant, B, C, D> {
    /// Provides the APID for the command. This is a required step.
    pub fn apid(self, apid: Apid) -> TelecommandBuilder<Apid, B, C, D> {
        TelecommandBuilder {
            apid,
            sequence_count: self.sequence_count,
            function_code: self.function_code,
            buffer: self.buffer,
            payload: self.payload,
        }
    }
}

impl<A, B, C> TelecommandBuilder<A, B, C, Vacant> {
    /// Provides the payload for the command. This is a required step.
    pub fn payload<P: SpacePacketData>(self, payload: &P) -> TelecommandBuilder<A, B, C, &P> {
        TelecommandBuilder {
            apid: self.apid,
            sequence_count: self.sequence_count,
            function_code: self.function_code,
            buffer: self.buffer,
            payload,
        }
    }
}

impl<'a, 'b, P: SpacePacketData> TelecommandBuilder<Apid, u8, &'a mut [u8], &'b P> {
    pub fn build(self) -> Result<&'a mut Telecommand<P>, TelecommandError> {
        Telecommand::new(
            self.buffer,
            self.apid,
            self.sequence_count,
            self.function_code,
            self.payload,
        )
    }
}
