//! SRSPP ↔ Source/Sink adapters.
//!
//! Bridges SRSPP transport handles to the generic
//! [`Source`] and [`Sink`] traits from `leodos-protocols`.

use crate::Schema;
use crate::SpaceCompError;

use leodos_protocols::application::spacecomp::io::sink::Sink;
use leodos_protocols::application::spacecomp::io::writer::BufWriter;
use leodos_protocols::application::spacecomp::io::writer::MessageSender;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::network::isl::address::Address;

use core::marker::PhantomData;

/// Receives `DataChunk` messages over SRSPP and yields
/// typed records. Returns `None` after `expected_phases`
/// `PhaseDone` messages have arrived.
pub struct SrsppSource<T> {
    expected_phases: u8,
    phases_done: u8,
    _record: PhantomData<T>,
}

impl<T> SrsppSource<T> {
    pub fn new(expected_phases: u8) -> Self {
        Self {
            expected_phases,
            phases_done: 0,
            _record: PhantomData,
        }
    }
}

/// Wraps a [`BufWriter`] to implement [`Sink`].
pub struct SrsppSink<'a, T: Schema, Tx: MessageSender> {
    writer: BufWriter<'a, T, Tx>,
}

impl<'a, T: Schema, Tx: MessageSender> SrsppSink<'a, T, Tx> {
    pub fn new(tx: &'a mut Tx, buf: &'a mut [u8], target: Address, job_id: u16) -> Self {
        Self {
            writer: BufWriter::new(tx, buf, target, job_id, OpCode::DataChunk),
        }
    }
}

impl<T: Schema, Tx: MessageSender> Sink for SrsppSink<'_, T, Tx> {
    type Input = T;
    type Error = leodos_protocols::application::spacecomp::packet::BuildError;

    async fn write(&mut self, val: &T) -> Result<(), Self::Error> {
        self.writer.write(val).await
    }
}
