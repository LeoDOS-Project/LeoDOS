//! FARM-1 (Frame Acceptance and Reporting Mechanism).
//!
//! CCSDS 232.1-B-2 Section 6. Receiver-side state machine for COP-1.
//! Processes incoming TC transfer frames and generates CLCW reports.
//!
//! Three states: Open (S1), Wait (S2), Lockout (S3).
//! Frame types determined by Bypass and Control Command flags:
//! - AD: Bypass=0, CC=0 — sequence-controlled data
//! - BC: Bypass=1, CC=1 — control commands (Unlock, Set V(R))
//! - BD: Bypass=1, CC=0 — expedited/bypass data

use heapless::Vec;

use crate::ids::Vcid;
use super::clcw::Clcw;

/// Maximum actions per event.
const MAX_ACTIONS: usize = 4;

/// FARM-1 states per CCSDS 232.1-B-2 Table 6-1.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FarmState {
    /// S1: normal operation, accepting in-sequence AD frames.
    Open,
    /// S2: no buffer space, Wait flag is set.
    Wait,
    /// S3: lockout due to out-of-window frame.
    Lockout,
}

/// BC control commands carried in Type-BC frames.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ControlCommand {
    /// Reset FARM-1 to Open state, clear all flags.
    Unlock,
    /// Set V(R) to the specified value.
    SetVr(u8),
}

/// Events that drive the FARM-1 state machine.
#[derive(Debug, Clone)]
pub enum FarmEvent<'a> {
    /// A valid Type-AD transfer frame arrived.
    AdFrame {
        /// Frame Sequence Number N(S).
        seq: u8,
        /// Whether a buffer is available for this frame.
        buffer_available: bool,
        /// Frame Data Unit payload.
        fdu: &'a [u8],
    },

    /// A valid Type-BD transfer frame arrived.
    BdFrame {
        /// Frame Data Unit payload.
        fdu: &'a [u8],
    },

    /// A valid Type-BC transfer frame with a control command.
    BcFrame {
        /// The control command.
        command: ControlCommand,
    },

    /// An invalid frame arrived (failed validation).
    InvalidFrame,

    /// Buffer space became available (flow control release).
    BufferRelease,

    /// Time to generate a CLCW report.
    ClcwReport,
}

/// Actions that FARM-1 requests the driver to perform.
#[derive(Debug, Clone)]
pub enum FarmAction<'a> {
    /// Accept an FDU and deliver it to higher procedures.
    Accept {
        /// The FDU payload to deliver.
        fdu: &'a [u8],
    },

    /// Discard the received frame (no delivery).
    Discard,

    /// A CLCW report is ready; read it from [`FarmMachine::clcw`].
    ClcwReady,
}

/// Collection of actions emitted by FARM-1.
#[derive(Debug)]
pub struct FarmActions<'a> {
    inner: Vec<FarmAction<'a>, MAX_ACTIONS>,
}

impl<'a> FarmActions<'a> {
    /// Create a new empty actions collection.
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    fn push(&mut self, action: FarmAction<'a>) {
        let _ = self.inner.push(action);
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterate over the actions.
    pub fn iter(&self) -> impl Iterator<Item = &FarmAction<'a>> {
        self.inner.iter()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Number of actions.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for FarmActions<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'b, 'a> IntoIterator for &'b FarmActions<'a> {
    type Item = &'b FarmAction<'a>;
    type IntoIter = core::slice::Iter<'b, FarmAction<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

/// Configuration for FARM-1.
#[derive(Debug, Clone)]
#[derive(bon::Builder)]
pub struct FarmConfig {
    /// Virtual Channel Identifier for CLCW reports.
    pub vcid: Vcid,
    /// FARM Sliding Window Width (W). Even, 2..=254.
    pub window_width: u8,
}

/// FARM-1 state machine.
///
/// Completely synchronous — no I/O, no async.
/// Generates CLCW reports from its internal state.
pub struct FarmMachine {
    state: FarmState,
    /// V(R): next expected frame sequence number.
    vr: u8,
    lockout_flag: bool,
    wait_flag: bool,
    retransmit_flag: bool,
    /// 2-bit counter incremented on BD/BC frames.
    farm_b_counter: u8,
    /// Positive window half-width: PW = W/2.
    pw: u8,
    /// Negative window half-width: NW = W/2.
    nw: u8,
    /// Virtual Channel ID for CLCW.
    vcid: Vcid,
}

impl FarmMachine {
    /// Create a new FARM-1 state machine.
    pub fn new(config: FarmConfig) -> Self {
        let pw = config.window_width / 2;
        let nw = config.window_width / 2;
        Self {
            state: FarmState::Open,
            vr: 0,
            lockout_flag: false,
            wait_flag: false,
            retransmit_flag: false,
            farm_b_counter: 0,
            pw,
            nw,
            vcid: config.vcid,
        }
    }

    /// Current FARM-1 state.
    pub fn state(&self) -> FarmState {
        self.state
    }

    /// Current V(R) (next expected sequence number).
    pub fn vr(&self) -> u8 {
        self.vr
    }

    /// Generate the current CLCW from internal state.
    pub fn clcw(&self) -> Clcw {
        let mut clcw = Clcw::new();
        clcw.set_cop_in_effect(1);
        clcw.set_vcid(self.vcid);
        clcw.set_lockout(self.lockout_flag);
        clcw.set_wait(self.wait_flag);
        clcw.set_retransmit(self.retransmit_flag);
        clcw.set_farm_b_counter(self.farm_b_counter);
        clcw.set_report_value(self.vr);
        clcw
    }

    /// Process an event and emit actions.
    pub fn handle<'a>(
        &mut self,
        event: FarmEvent<'a>,
        actions: &mut FarmActions<'a>,
    ) {
        actions.clear();

        match event {
            FarmEvent::AdFrame {
                seq,
                buffer_available,
                fdu,
            } => {
                self.handle_ad_frame(seq, buffer_available, fdu, actions);
            }
            FarmEvent::BdFrame { fdu } => {
                self.handle_bd_frame(fdu, actions);
            }
            FarmEvent::BcFrame { command } => {
                self.handle_bc_frame(command, actions);
            }
            FarmEvent::InvalidFrame => {
                // E9: discard in all states, no state change.
                actions.push(FarmAction::Discard);
            }
            FarmEvent::BufferRelease => {
                self.handle_buffer_release(actions);
            }
            FarmEvent::ClcwReport => {
                // E11: report CLCW in all states, no state change.
                actions.push(FarmAction::ClcwReady);
            }
        }
    }

    /// Handle a Type-AD frame (Table 6-1 events E1-E5).
    fn handle_ad_frame<'a>(
        &mut self,
        seq: u8,
        buffer_available: bool,
        fdu: &'a [u8],
        actions: &mut FarmActions<'a>,
    ) {
        let zone = self.classify_seq(seq);

        match self.state {
            FarmState::Open => match zone {
                SeqZone::Expected => {
                    if buffer_available {
                        // E1/S1: accept, advance V(R), clear retransmit
                        self.vr = self.vr.wrapping_add(1);
                        self.retransmit_flag = false;
                        actions.push(FarmAction::Accept { fdu });
                    } else {
                        // E2/S1: no buffer, discard, set flags, → Wait
                        self.retransmit_flag = true;
                        self.wait_flag = true;
                        self.state = FarmState::Wait;
                        actions.push(FarmAction::Discard);
                    }
                }
                SeqZone::PositiveWindow => {
                    // E3/S1: in positive window but N(S) > V(R)
                    self.retransmit_flag = true;
                    actions.push(FarmAction::Discard);
                }
                SeqZone::NegativeWindow => {
                    // E4/S1: retransmitted frame already accepted
                    actions.push(FarmAction::Discard);
                }
                SeqZone::Lockout => {
                    // E5/S1: outside sliding window → Lockout
                    self.lockout_flag = true;
                    self.state = FarmState::Lockout;
                    actions.push(FarmAction::Discard);
                }
            },
            FarmState::Wait => match zone {
                SeqZone::Expected => {
                    // E1/S2: not applicable (spec says N/A)
                    actions.push(FarmAction::Discard);
                }
                SeqZone::PositiveWindow => {
                    // E3/S2: discard, stay in Wait
                    actions.push(FarmAction::Discard);
                }
                SeqZone::NegativeWindow => {
                    // E4/S2: discard, stay in Wait
                    actions.push(FarmAction::Discard);
                }
                SeqZone::Lockout => {
                    // E5/S2: → Lockout
                    self.lockout_flag = true;
                    self.state = FarmState::Lockout;
                    actions.push(FarmAction::Discard);
                }
            },
            FarmState::Lockout => {
                // E1-E5/S3: discard everything
                actions.push(FarmAction::Discard);
            }
        }
    }

    /// Handle a Type-BD frame (Table 6-1 event E6).
    fn handle_bd_frame<'a>(
        &mut self,
        fdu: &'a [u8],
        actions: &mut FarmActions<'a>,
    ) {
        // E6: accept in all states, increment FARM-B counter
        self.farm_b_counter = (self.farm_b_counter + 1) & 0x03;
        actions.push(FarmAction::Accept { fdu });
    }

    /// Handle a Type-BC frame (Table 6-1 events E7, E8).
    fn handle_bc_frame(&mut self, command: ControlCommand, _actions: &mut FarmActions<'_>) {
        match command {
            ControlCommand::Unlock => {
                // E7: Unlock resets FARM-1 to Open in all states
                self.farm_b_counter = (self.farm_b_counter + 1) & 0x03;
                self.retransmit_flag = false;
                match self.state {
                    FarmState::Open => {
                        // Already open, just clear flags
                    }
                    FarmState::Wait => {
                        self.wait_flag = false;
                        self.state = FarmState::Open;
                    }
                    FarmState::Lockout => {
                        self.wait_flag = false;
                        self.lockout_flag = false;
                        self.state = FarmState::Open;
                    }
                }
            }
            ControlCommand::SetVr(new_vr) => {
                // E8: Set V(R) to V*(R)
                self.farm_b_counter = (self.farm_b_counter + 1) & 0x03;
                self.retransmit_flag = false;
                match self.state {
                    FarmState::Open => {
                        self.vr = new_vr;
                    }
                    FarmState::Wait => {
                        self.vr = new_vr;
                        self.wait_flag = false;
                        self.state = FarmState::Open;
                    }
                    FarmState::Lockout => {
                        // E8/S3: just increment FARM-B counter,
                        // do NOT execute the Set V(R). Stay locked.
                    }
                }
            }
        }
    }

    /// Handle buffer release signal (Table 6-1 event E10).
    fn handle_buffer_release(&mut self, actions: &mut FarmActions<'_>) {
        match self.state {
            FarmState::Open => {
                // E10/S1: ignore
            }
            FarmState::Wait => {
                // E10/S2: clear wait flag, → Open
                self.wait_flag = false;
                self.state = FarmState::Open;
            }
            FarmState::Lockout => {
                // E10/S3: clear wait flag, stay Lockout
                self.wait_flag = false;
            }
        }
        let _ = actions;
    }

    /// Classify a frame sequence number into a sliding window zone.
    ///
    /// All arithmetic is modulo 256 per the spec.
    fn classify_seq(&self, seq: u8) -> SeqZone {
        if seq == self.vr {
            return SeqZone::Expected;
        }

        // Distance from V(R) in the positive direction (mod 256)
        let pos_dist = seq.wrapping_sub(self.vr);
        // Distance from V(R) in the negative direction (mod 256)
        let neg_dist = self.vr.wrapping_sub(seq);

        if pos_dist > 0 && pos_dist < self.pw {
            SeqZone::PositiveWindow
        } else if neg_dist > 0 && neg_dist <= self.nw {
            SeqZone::NegativeWindow
        } else {
            SeqZone::Lockout
        }
    }
}

/// Which zone of the FARM sliding window a sequence number falls in.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum SeqZone {
    /// N(S) == V(R): the expected next frame.
    Expected,
    /// N(S) > V(R) and within positive window (gap detected).
    PositiveWindow,
    /// N(S) < V(R) and within negative window (retransmit).
    NegativeWindow,
    /// Outside the sliding window entirely.
    Lockout,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::Vcid;

    fn make_farm() -> FarmMachine {
        FarmMachine::new(
            FarmConfig::builder()
                .vcid(Vcid::new(0))
                .window_width(10)
                .build(),
        )
    }

    #[test]
    fn accept_in_sequence_frame() {
        let mut farm = make_farm();
        let fdu = [1, 2, 3];
        let mut actions = FarmActions::new();

        farm.handle(
            FarmEvent::AdFrame {
                seq: 0,
                buffer_available: true,
                fdu: &fdu,
            },
            &mut actions,
        );

        assert_eq!(actions.len(), 1);
        assert!(matches!(
            actions.iter().next().unwrap(),
            FarmAction::Accept { .. }
        ));
        assert_eq!(farm.vr(), 1);
        assert!(!farm.clcw().retransmit());
    }

    #[test]
    fn no_buffer_transitions_to_wait() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        farm.handle(
            FarmEvent::AdFrame {
                seq: 0,
                buffer_available: false,
                fdu: &[],
            },
            &mut actions,
        );

        assert_eq!(farm.state(), FarmState::Wait);
        assert!(farm.clcw().wait());
        assert!(farm.clcw().retransmit());
    }

    #[test]
    fn out_of_window_causes_lockout() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // With W=10, PW=5: seq 200 is way outside the window
        farm.handle(
            FarmEvent::AdFrame {
                seq: 200,
                buffer_available: true,
                fdu: &[],
            },
            &mut actions,
        );

        assert_eq!(farm.state(), FarmState::Lockout);
        assert!(farm.clcw().lockout());
    }

    #[test]
    fn unlock_resets_from_lockout() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // Force lockout
        farm.handle(
            FarmEvent::AdFrame {
                seq: 200,
                buffer_available: true,
                fdu: &[],
            },
            &mut actions,
        );
        assert_eq!(farm.state(), FarmState::Lockout);

        // Unlock
        farm.handle(
            FarmEvent::BcFrame {
                command: ControlCommand::Unlock,
            },
            &mut actions,
        );

        assert_eq!(farm.state(), FarmState::Open);
        assert!(!farm.clcw().lockout());
    }

    #[test]
    fn set_vr_updates_sequence() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        farm.handle(
            FarmEvent::BcFrame {
                command: ControlCommand::SetVr(42),
            },
            &mut actions,
        );

        assert_eq!(farm.vr(), 42);
        assert_eq!(farm.clcw().report_value(), 42);
    }

    #[test]
    fn set_vr_ignored_in_lockout() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // Force lockout
        farm.handle(
            FarmEvent::AdFrame {
                seq: 200,
                buffer_available: true,
                fdu: &[],
            },
            &mut actions,
        );

        // Set V(R) should NOT work in Lockout (E8/S3)
        farm.handle(
            FarmEvent::BcFrame {
                command: ControlCommand::SetVr(42),
            },
            &mut actions,
        );

        assert_eq!(farm.state(), FarmState::Lockout);
        assert_eq!(farm.vr(), 0); // unchanged
    }

    #[test]
    fn bd_frame_accepted_in_all_states() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // Force lockout
        farm.handle(
            FarmEvent::AdFrame {
                seq: 200,
                buffer_available: true,
                fdu: &[],
            },
            &mut actions,
        );

        // BD should still be accepted
        farm.handle(
            FarmEvent::BdFrame { fdu: &[0xAA] },
            &mut actions,
        );

        assert!(matches!(
            actions.iter().next().unwrap(),
            FarmAction::Accept { .. }
        ));
    }

    #[test]
    fn farm_b_counter_increments_on_bd_bc() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        farm.handle(
            FarmEvent::BdFrame { fdu: &[] },
            &mut actions,
        );
        assert_eq!(farm.clcw().farm_b_counter(), 1);

        farm.handle(
            FarmEvent::BcFrame {
                command: ControlCommand::Unlock,
            },
            &mut actions,
        );
        assert_eq!(farm.clcw().farm_b_counter(), 2);
    }

    #[test]
    fn buffer_release_transitions_wait_to_open() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // Go to Wait
        farm.handle(
            FarmEvent::AdFrame {
                seq: 0,
                buffer_available: false,
                fdu: &[],
            },
            &mut actions,
        );
        assert_eq!(farm.state(), FarmState::Wait);

        // Buffer release
        farm.handle(FarmEvent::BufferRelease, &mut actions);

        assert_eq!(farm.state(), FarmState::Open);
        assert!(!farm.clcw().wait());
    }

    #[test]
    fn positive_window_sets_retransmit() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // Seq 3 while V(R)=0, within PW=5
        farm.handle(
            FarmEvent::AdFrame {
                seq: 3,
                buffer_available: true,
                fdu: &[],
            },
            &mut actions,
        );

        assert_eq!(farm.state(), FarmState::Open);
        assert!(farm.clcw().retransmit());
        assert!(matches!(
            actions.iter().next().unwrap(),
            FarmAction::Discard
        ));
    }

    #[test]
    fn negative_window_silently_discards() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        // Accept frames 0..3 to advance V(R) to 3
        for i in 0..3u8 {
            farm.handle(
                FarmEvent::AdFrame {
                    seq: i,
                    buffer_available: true,
                    fdu: &[],
                },
                &mut actions,
            );
        }
        assert_eq!(farm.vr(), 3);

        // Seq 1 is in the negative window (retransmit of old)
        farm.handle(
            FarmEvent::AdFrame {
                seq: 1,
                buffer_available: true,
                fdu: &[],
            },
            &mut actions,
        );

        assert_eq!(farm.state(), FarmState::Open);
        assert!(!farm.clcw().retransmit());
        assert!(matches!(
            actions.iter().next().unwrap(),
            FarmAction::Discard
        ));
    }

    #[test]
    fn clcw_report_action() {
        let mut farm = make_farm();
        let mut actions = FarmActions::new();

        farm.handle(FarmEvent::ClcwReport, &mut actions);

        assert!(matches!(
            actions.iter().next().unwrap(),
            FarmAction::ClcwReady
        ));
    }

    #[test]
    fn wrapping_sequence_acceptance() {
        let mut farm = FarmMachine::new(
            FarmConfig::builder()
                .vcid(Vcid::new(0))
                .window_width(10)
                .build(),
        );
        let fdu_buf = [254u8, 255, 0, 1];
        let mut actions = FarmActions::new();

        // Set V(R) to 254 so we test wrapping around 255 → 0
        farm.handle(
            FarmEvent::BcFrame {
                command: ControlCommand::SetVr(254),
            },
            &mut actions,
        );

        // Accept 254, 255, 0, 1
        for (i, &expected_seq) in fdu_buf.iter().enumerate() {
            farm.handle(
                FarmEvent::AdFrame {
                    seq: expected_seq,
                    buffer_available: true,
                    fdu: &fdu_buf[i..i + 1],
                },
                &mut actions,
            );
            assert!(
                matches!(
                    actions.iter().next().unwrap(),
                    FarmAction::Accept { .. }
                ),
                "should accept seq {expected_seq}"
            );
        }
        assert_eq!(farm.vr(), 2);
    }
}
