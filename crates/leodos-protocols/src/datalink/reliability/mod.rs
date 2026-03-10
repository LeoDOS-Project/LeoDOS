//! Hop-by-hop reliable frame delivery for the datalink layer.
//!
//! Provides the [`ReliabilityWriter`] / [`ReliabilityReader`]
//! traits, COP-1 wrappers ([`Cop1Writer`], [`Cop1Reader`]), and
//! [`NoReliability`] as a passthrough.

use cop1::farm::{FarmActions, FarmConfig, FarmEvent, FarmMachine};
use cop1::fop::{FopActions, FopConfig, FopEvent, FopMachine};

use crate::datalink::framing::sdlp::tc::{BypassFlag, ControlFlag};

/// COP-1 (CCSDS 232.1-B-2) state machines.
pub mod cop1;

/// COP-1 sender (FOP-1) state machine interface.
pub trait ReliabilityWriter {
    /// Action to take after processing a frame.
    type Action;
    /// Processes an outgoing frame through the reliability layer.
    fn write(&mut self, frame: &[u8]) -> Self::Action;
}

/// COP-1 receiver (FARM-1) state machine interface.
pub trait ReliabilityReader {
    /// Action to take after processing a frame.
    type Action;
    /// Processes an incoming frame through the reliability layer.
    fn read(&mut self, frame: &[u8]) -> Self::Action;
}

/// COP-1 sender-side reliability (FOP-1).
///
/// Wraps a [`FopMachine`] and implements [`ReliabilityWriter`].
/// Each [`write`](ReliabilityWriter::write) call feeds the data
/// as a Type-AD (sequence-controlled) FDU. For Type-BD frames or
/// management directives, use [`fop_mut`](Self::fop_mut).
pub struct Cop1Writer<const WIN: usize, const BUF: usize> {
    fop: FopMachine<WIN, BUF>,
}

impl<const WIN: usize, const BUF: usize> Cop1Writer<WIN, BUF> {
    /// Creates a new COP-1 writer in the Initial state.
    pub fn new(config: FopConfig) -> Self {
        Self {
            fop: FopMachine::new(config),
        }
    }

    /// Returns a reference to the inner FOP-1 state machine.
    pub fn fop(&self) -> &FopMachine<WIN, BUF> {
        &self.fop
    }

    /// Returns a mutable reference for direct FOP-1 control.
    pub fn fop_mut(&mut self) -> &mut FopMachine<WIN, BUF> {
        &mut self.fop
    }
}

impl<const WIN: usize, const BUF: usize> ReliabilityWriter
    for Cop1Writer<WIN, BUF>
{
    type Action = FopActions;

    fn write(&mut self, frame: &[u8]) -> FopActions {
        let mut actions = FopActions::new();
        self.fop
            .handle(FopEvent::SendAd { fdu: frame }, &mut actions);
        actions
    }
}

/// Result of processing a frame through COP-1 FARM-1.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Cop1ReadResult {
    /// The frame was accepted for delivery.
    Accept,
    /// The frame was discarded (out of sequence, lockout, etc).
    Discard,
}

/// COP-1 receiver-side reliability (FARM-1).
///
/// Wraps a [`FarmMachine`] and implements [`ReliabilityReader`].
/// Each [`read`](ReliabilityReader::read) call parses the TC
/// frame header to classify it as AD or BD and feeds the FARM-1
/// state machine. BC (control) frames are treated as invalid in
/// the trait impl — use [`farm_mut`](Self::farm_mut) for direct
/// BC handling.
pub struct Cop1Reader {
    farm: FarmMachine,
}

impl Cop1Reader {
    /// Creates a new COP-1 reader in the Open state.
    pub fn new(config: FarmConfig) -> Self {
        Self {
            farm: FarmMachine::new(config),
        }
    }

    /// Returns a reference to the inner FARM-1 state machine.
    pub fn farm(&self) -> &FarmMachine {
        &self.farm
    }

    /// Returns a mutable reference for direct FARM-1 control.
    pub fn farm_mut(&mut self) -> &mut FarmMachine {
        &mut self.farm
    }
}

impl ReliabilityReader for Cop1Reader {
    type Action = Cop1ReadResult;

    fn read(&mut self, frame: &[u8]) -> Cop1ReadResult {
        use cop1::farm::FarmAction;
        use crate::datalink::framing::sdlp::tc::TelecommandTransferFrame;

        let mut actions = FarmActions::new();

        let Ok(tc_frame) = TelecommandTransferFrame::parse(frame)
        else {
            self.farm
                .handle(FarmEvent::InvalidFrame, &mut actions);
            return Cop1ReadResult::Discard;
        };

        let bypass = tc_frame.header().bypass_flag();
        let control = tc_frame.header().control_flag();
        let seq = tc_frame.header().sequence_num();
        let fdu = tc_frame.data_field();

        match (bypass, control) {
            (BypassFlag::TypeA, ControlFlag::TypeD) => {
                // AD frame: sequence-controlled data
                self.farm.handle(
                    FarmEvent::AdFrame {
                        seq,
                        buffer_available: true,
                        fdu,
                    },
                    &mut actions,
                );
            }
            (BypassFlag::TypeB, ControlFlag::TypeD) => {
                // BD frame: bypass data
                self.farm.handle(
                    FarmEvent::BdFrame { fdu },
                    &mut actions,
                );
            }
            _ => {
                // BC (TypeB+TypeC) or invalid (TypeA+TypeC):
                // use farm_mut() for direct BC handling.
                self.farm
                    .handle(FarmEvent::InvalidFrame, &mut actions);
                return Cop1ReadResult::Discard;
            }
        }

        for action in &actions {
            if matches!(action, FarmAction::Accept { .. }) {
                return Cop1ReadResult::Accept;
            }
        }

        Cop1ReadResult::Discard
    }
}

/// No-op reliability layer (passthrough).
///
/// Accepts all frames without sequencing or retransmission.
pub struct NoReliability;

impl ReliabilityWriter for NoReliability {
    type Action = ();

    fn write(&mut self, _frame: &[u8]) {}
}

impl ReliabilityReader for NoReliability {
    type Action = ();

    fn read(&mut self, _frame: &[u8]) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use cop1::fop::{FopAction, FopState};

    fn fop_config() -> FopConfig {
        FopConfig::builder()
            .window_width(4)
            .transmission_limit(3)
            .timeout_type(0)
            .build()
    }

    fn farm_config() -> FarmConfig {
        FarmConfig::builder().vcid(0).window_width(10).build()
    }

    #[test]
    fn cop1_writer_rejects_in_initial_state() {
        let mut writer: Cop1Writer<8, 1024> =
            Cop1Writer::new(fop_config());

        let actions = writer.write(&[1, 2, 3]);
        assert!(actions.iter().any(|a| matches!(a, FopAction::Reject)));
    }

    #[test]
    fn cop1_writer_sends_ad_when_active() {
        let mut writer: Cop1Writer<8, 1024> =
            Cop1Writer::new(fop_config());

        let mut init_actions = FopActions::new();
        writer
            .fop_mut()
            .handle(FopEvent::InitAdNoClcw, &mut init_actions);
        assert_eq!(writer.fop().state(), FopState::Active);

        let actions = writer.write(&[0xAA, 0xBB]);
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitAd { seq: 0 })));
    }

    #[test]
    fn cop1_reader_accepts_in_sequence_ad() {
        let mut reader = Cop1Reader::new(farm_config());

        let mut buf = [0u8; 64];
        let frame =
            crate::datalink::framing::sdlp::tc::TelecommandTransferFrame::builder()
                .buffer(&mut buf)
                .scid(1)
                .vcid(0)
                .bypass_flag(BypassFlag::TypeA)
                .control_flag(ControlFlag::TypeD)
                .seq(0)
                .data_field_len(4)
                .build()
                .unwrap();
        frame.data_field_mut().copy_from_slice(&[1, 2, 3, 4]);
        let frame_len = frame.frame_len();

        let result = reader.read(&buf[..frame_len]);
        assert_eq!(result, Cop1ReadResult::Accept);
        assert_eq!(reader.farm().vr(), 1);
    }

    #[test]
    fn cop1_reader_accepts_bd_frame() {
        let mut reader = Cop1Reader::new(farm_config());

        let mut buf = [0u8; 64];
        let frame =
            crate::datalink::framing::sdlp::tc::TelecommandTransferFrame::builder()
                .buffer(&mut buf)
                .scid(1)
                .vcid(0)
                .bypass_flag(BypassFlag::TypeB)
                .control_flag(ControlFlag::TypeD)
                .seq(0)
                .data_field_len(2)
                .build()
                .unwrap();
        frame.data_field_mut().copy_from_slice(&[0xAA, 0xBB]);
        let frame_len = frame.frame_len();

        let result = reader.read(&buf[..frame_len]);
        assert_eq!(result, Cop1ReadResult::Accept);
    }

    #[test]
    fn no_reliability_passthrough() {
        let mut writer = NoReliability;
        let mut reader = NoReliability;

        writer.write(&[1, 2, 3]);
        reader.read(&[1, 2, 3]);
    }
}
