use crate::cfe::tm::TelemetryError;
use crate::Apid;
use crate::PacketSequenceCount;
use crate::SpacePacketData;
use crate::builder::Vacant;
use crate::cfe::tm::Telemetry;

/// A builder specialized for constructing a CFE Telemetry Packet.
/// This builder follows the typestate pattern.
pub struct TelemetryBuilder<A, B, C> {
    apid: A,
    sequence_count: PacketSequenceCount,
    time: [u8; 6],
    buffer: B,
    payload: C,
}

impl TelemetryBuilder<Vacant, Vacant, Vacant> {
    /// Creates a new builder for a CFE telemetry packet.
    pub fn new() -> Self {
        Self {
            apid: Vacant,
            sequence_count: PacketSequenceCount::new(),
            time: [0; 6], // Default time is zero
            buffer: Vacant,
            payload: Vacant,
        }
    }
}

impl<A, C, D> TelemetryBuilder<A, C, D> {
    /// Overrides the default sequence count (0).
    pub fn sequence_count(mut self, count: PacketSequenceCount) -> Self {
        self.sequence_count = count;
        self
    }

    /// Sets the 6-byte CCSDS timestamp for the telemetry packet.
    /// If not called, the timestamp will be all zeros.
    pub fn time(mut self, time: [u8; 6]) -> Self {
        self.time = time;
        self
    }
}

impl<C, D> TelemetryBuilder<Vacant, C, D> {
    /// Provides the APID for the telemetry packet. This is a required step.
    pub fn apid(self, apid: Apid) -> TelemetryBuilder<Apid, C, D> {
        TelemetryBuilder {
            apid,
            sequence_count: self.sequence_count,
            time: self.time,
            buffer: self.buffer,
            payload: self.payload,
        }
    }
}

impl<A, D> TelemetryBuilder<A, Vacant, D> {
    /// Provides the buffer where the packet will be built. This is a required step.
    pub fn buffer<'a>(self, buffer: &'a mut [u8]) -> TelemetryBuilder<A, &'a mut [u8], D> {
        TelemetryBuilder {
            apid: self.apid,
            sequence_count: self.sequence_count,
            time: self.time,
            buffer,
            payload: self.payload,
        }
    }
}

impl<A, C> TelemetryBuilder<A, C, Vacant> {
    /// Provides the payload data for the packet. This is a required step.
    pub fn payload<'a, P: SpacePacketData>(self, payload: &'a P) -> TelemetryBuilder<A, C, &'a P> {
        TelemetryBuilder {
            apid: self.apid,
            sequence_count: self.sequence_count,
            time: self.time,
            buffer: self.buffer,
            payload,
        }
    }
}

impl<'a, 'b, P: SpacePacketData + Copy> TelemetryBuilder<Apid, &'a mut [u8], &'b P> {
    /// Builds the CFE telemetry packet into the provided buffer and returns
    /// a mutable view of the final packet.
    pub fn build(self) -> Result<&'a mut Telemetry<P>, TelemetryError> {
        Telemetry::new(
            self.buffer,
            self.apid,
            self.sequence_count,
            self.time,
            self.payload,
        )
    }
}
