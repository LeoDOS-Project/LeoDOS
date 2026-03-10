//! FOP-1 (Frame Operation Procedure) sender state machine.
//!
//! CCSDS 232.1-B-2 Section 5. Handles sending of TC transfer frames
//! with go-back-N ARQ using CLCW feedback from FARM-1.
//!
//! Six states:
//! - S1: Active, no pending init
//! - S2: Active, retransmitting
//! - S3: Active, initializing without BC frame
//! - S4: Active, initializing with BC frame
//! - S5: Active, initializing with unlock
//! - S6: Initial (inactive)

use heapless::Vec;

use super::clcw::Clcw;

/// Maximum actions per event.
const MAX_ACTIONS: usize = 16;

/// FOP-1 states per CCSDS 232.1-B-2 Table 5-1.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FopState {
    /// S1: active, transmitting AD frames normally.
    Active,
    /// S2: active, retransmitting (go-back-N triggered).
    Retransmitting,
    /// S3: initializing without BC frame.
    InitNoBC,
    /// S4: initializing with BC frame (Set V(R)).
    InitWithSetVr,
    /// S5: initializing with BC frame (Unlock).
    InitWithUnlock,
    /// S6: initial/inactive state.
    Initial,
}

/// Events that drive the FOP-1 state machine.
#[derive(Debug, Clone)]
pub enum FopEvent<'a> {
    /// Higher procedures request transfer of an AD FDU.
    SendAd {
        /// Data to send in a Type-AD frame.
        fdu: &'a [u8],
    },

    /// Higher procedures request transfer of a BD FDU.
    SendBd {
        /// Data to send in a Type-BD frame.
        fdu: &'a [u8],
    },

    /// A CLCW was received (from the return link).
    ClcwReceived {
        /// The received CLCW.
        clcw: Clcw,
    },

    /// The retransmission timer T1 expired.
    TimerExpired,

    /// Management directive: Initiate AD Service (no CLCW).
    InitAdNoClcw,

    /// Management directive: Initiate AD Service with CLCW.
    InitAdWithClcw,

    /// Management directive: Initiate AD with Set V(R).
    InitAdSetVr {
        /// The V(R) value to set at the receiver.
        vr: u8,
    },

    /// Management directive: Initiate AD with Unlock.
    InitAdUnlock,

    /// Management directive: Terminate AD Service.
    Terminate,

    /// Management directive: Resume AD Service.
    Resume,
}

/// Actions that FOP-1 requests the driver to perform.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FopAction {
    /// Transmit a Type-AD frame with the given sequence number.
    /// The driver should call [`FopMachine::get_fdu`] to get the data.
    TransmitAd {
        /// Frame Sequence Number N(S) to assign.
        seq: u8,
    },

    /// Transmit a Type-BD frame.
    TransmitBd,

    /// Transmit a Type-BC Unlock frame.
    TransmitBcUnlock,

    /// Transmit a Type-BC Set V(R) frame.
    TransmitBcSetVr {
        /// The V(R) value to set.
        vr: u8,
    },

    /// Start (or restart) timer T1 with the configured initial value.
    StartTimer,

    /// Stop timer T1.
    StopTimer,

    /// AD service has been accepted (init complete).
    Accept,

    /// The FDU was rejected (queue full, wrong state, etc).
    Reject,

    /// An alert condition: link has failed.
    Alert,

    /// AD service was successfully terminated.
    Terminated,

    /// An AD frame was acknowledged by the receiver.
    Acknowledged {
        /// Sequence number acknowledged.
        seq: u8,
    },
}

/// Collection of actions emitted by FOP-1.
#[derive(Debug)]
pub struct FopActions {
    inner: Vec<FopAction, MAX_ACTIONS>,
}

impl FopActions {
    /// Create a new empty actions collection.
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    fn push(&mut self, action: FopAction) {
        let _ = self.inner.push(action);
    }

    fn clear(&mut self) {
        self.inner.clear();
    }

    /// Iterate over the actions.
    pub fn iter(&self) -> impl Iterator<Item = &FopAction> {
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

impl Default for FopActions {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a FopActions {
    type Item = &'a FopAction;
    type IntoIter = core::slice::Iter<'a, FopAction>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.iter()
    }
}

/// Configuration for FOP-1.
#[derive(Debug, Clone)]
#[derive(bon::Builder)]
pub struct FopConfig {
    /// FOP Sliding Window Width (K), 1..=255.
    pub window_width: u8,
    /// Maximum transmission attempts before alerting.
    pub transmission_limit: u8,
    /// Timeout type: 0 = timer off after each new ACK,
    /// 1 = timer always running.
    pub timeout_type: u8,
}

/// An FDU slot in the sent queue.
#[derive(Clone)]
struct FduSlot {
    /// Whether this slot is occupied.
    occupied: bool,
    /// Frame Sequence Number assigned.
    seq: u8,
    /// Start offset in the data buffer.
    offset: usize,
    /// Length of the FDU data.
    len: usize,
    /// Number of times this frame has been transmitted.
    transmission_count: u8,
}

impl Default for FduSlot {
    fn default() -> Self {
        Self {
            occupied: false,
            seq: 0,
            offset: 0,
            len: 0,
            transmission_count: 0,
        }
    }
}

/// FOP-1 state machine.
///
/// Implements the sender side of COP-1 go-back-N ARQ.
/// Completely synchronous — no I/O, no async.
///
/// # Type Parameters
///
/// * `WIN` — Maximum sent queue depth (sliding window).
/// * `BUF` — Total FDU buffer size in bytes.
pub struct FopMachine<const WIN: usize, const BUF: usize> {
    state: FopState,
    config: FopConfig,

    /// V(S): Transmitter Frame Sequence Number.
    vs: u8,
    /// NN(R): Expected ACK frame sequence number (last N(R) seen).
    nnr: u8,

    /// Sent queue: frames transmitted but not yet acknowledged.
    sent_queue: [FduSlot; WIN],
    /// FDU data buffer.
    data: [u8; BUF],
    /// Current write position in data buffer.
    write_pos: usize,

    /// Transmission count for the current init BC frame.
    bc_transmission_count: u8,
    /// V(R) value for Set V(R) init directive.
    set_vr_value: u8,

    /// Pending BD FDU data range (offset, len) in data buffer.
    pending_bd: Option<(usize, usize)>,
}

impl<const WIN: usize, const BUF: usize> FopMachine<WIN, BUF> {
    /// Create a new FOP-1 state machine in the Initial (S6) state.
    pub fn new(config: FopConfig) -> Self {
        Self {
            state: FopState::Initial,
            config,
            vs: 0,
            nnr: 0,
            sent_queue: core::array::from_fn(|_| FduSlot::default()),
            data: [0u8; BUF],
            write_pos: 0,
            bc_transmission_count: 0,
            set_vr_value: 0,
            pending_bd: None,
        }
    }

    /// Current FOP-1 state.
    pub fn state(&self) -> FopState {
        self.state
    }

    /// Current V(S).
    pub fn vs(&self) -> u8 {
        self.vs
    }

    /// Get FDU payload for a given sequence number.
    pub fn get_fdu(&self, seq: u8) -> Option<&[u8]> {
        for slot in &self.sent_queue {
            if slot.occupied && slot.seq == seq {
                return Some(&self.data[slot.offset..slot.offset + slot.len]);
            }
        }
        None
    }

    /// Get the pending BD FDU payload (if any).
    pub fn get_bd_fdu(&self) -> Option<&[u8]> {
        self.pending_bd
            .map(|(off, len)| &self.data[off..off + len])
    }

    /// Number of frames in the sent queue.
    pub fn sent_count(&self) -> usize {
        self.sent_queue.iter().filter(|s| s.occupied).count()
    }

    /// Process an event and emit actions.
    pub fn handle(
        &mut self,
        event: FopEvent<'_>,
        actions: &mut FopActions,
    ) {
        actions.clear();

        match event {
            FopEvent::SendAd { fdu } => {
                self.handle_send_ad(fdu, actions);
            }
            FopEvent::SendBd { fdu } => {
                self.handle_send_bd(fdu, actions);
            }
            FopEvent::ClcwReceived { clcw } => {
                self.handle_clcw(clcw, actions);
            }
            FopEvent::TimerExpired => {
                self.handle_timer_expired(actions);
            }
            FopEvent::InitAdNoClcw => {
                self.handle_init_no_clcw(actions);
            }
            FopEvent::InitAdWithClcw => {
                self.handle_init_with_clcw(actions);
            }
            FopEvent::InitAdSetVr { vr } => {
                self.handle_init_set_vr(vr, actions);
            }
            FopEvent::InitAdUnlock => {
                self.handle_init_unlock(actions);
            }
            FopEvent::Terminate => {
                self.handle_terminate(actions);
            }
            FopEvent::Resume => {
                self.handle_resume(actions);
            }
        }
    }

    /// Handle SendAd: queue an AD frame for transmission.
    fn handle_send_ad(&mut self, fdu: &[u8], actions: &mut FopActions) {
        match self.state {
            FopState::Active | FopState::Retransmitting => {
                let Some(slot_idx) = self.find_empty_slot() else {
                    actions.push(FopAction::Reject);
                    return;
                };
                let Some(offset) = self.buffer_fdu(fdu) else {
                    actions.push(FopAction::Reject);
                    return;
                };

                let seq = self.vs;
                self.sent_queue[slot_idx] = FduSlot {
                    occupied: true,
                    seq,
                    offset,
                    len: fdu.len(),
                    transmission_count: 0,
                };
                self.vs = self.vs.wrapping_add(1);

                // Only transmit immediately in Active state
                if self.state == FopState::Active {
                    self.sent_queue[slot_idx].transmission_count = 1;
                    actions.push(FopAction::TransmitAd { seq });
                    actions.push(FopAction::StartTimer);
                }
            }
            _ => {
                actions.push(FopAction::Reject);
            }
        }
    }

    /// Handle SendBd: transmit a BD frame (bypass, no sequencing).
    fn handle_send_bd(&mut self, fdu: &[u8], actions: &mut FopActions) {
        // BD can be sent in any active state
        let Some(offset) = self.buffer_fdu(fdu) else {
            actions.push(FopAction::Reject);
            return;
        };
        self.pending_bd = Some((offset, fdu.len()));
        actions.push(FopAction::TransmitBd);
    }

    /// Handle CLCW received from the return link.
    fn handle_clcw(&mut self, clcw: Clcw, actions: &mut FopActions) {
        let nr = clcw.report_value();

        match self.state {
            FopState::Active => {
                if clcw.lockout() {
                    // Lockout → alert
                    self.alert(actions);
                    return;
                }

                // Remove acknowledged frames
                self.remove_acknowledged(nr, actions);
                self.nnr = nr;

                if clcw.retransmit() {
                    // Go-back-N: retransmit all from N(R)
                    self.initiate_retransmission(actions);
                    self.state = FopState::Retransmitting;
                } else if self.sent_count() == 0 {
                    actions.push(FopAction::StopTimer);
                }
            }
            FopState::Retransmitting => {
                if clcw.lockout() {
                    self.alert(actions);
                    return;
                }

                self.remove_acknowledged(nr, actions);
                self.nnr = nr;

                if !clcw.retransmit() {
                    // Retransmit flag cleared → back to Active
                    self.state = FopState::Active;
                    if self.sent_count() == 0 {
                        actions.push(FopAction::StopTimer);
                    }
                }
            }
            FopState::InitWithSetVr => {
                if nr == self.set_vr_value
                    && !clcw.lockout()
                    && !clcw.retransmit()
                {
                    // Init succeeded
                    self.vs = self.set_vr_value;
                    self.nnr = nr;
                    self.purge_sent_queue();
                    self.state = FopState::Active;
                    actions.push(FopAction::StopTimer);
                    actions.push(FopAction::Accept);
                }
            }
            FopState::InitWithUnlock => {
                if !clcw.lockout() && !clcw.retransmit() {
                    // Unlock accepted
                    self.nnr = nr;
                    self.vs = nr;
                    self.purge_sent_queue();
                    self.state = FopState::Active;
                    actions.push(FopAction::StopTimer);
                    actions.push(FopAction::Accept);
                }
            }
            FopState::InitNoBC | FopState::Initial => {
                // CLCW ignored in these states
            }
        }
    }

    /// Handle T1 timer expiration.
    fn handle_timer_expired(&mut self, actions: &mut FopActions) {
        match self.state {
            FopState::Active | FopState::Retransmitting => {
                // Check transmission limit on oldest unacked frame
                let limit_exceeded = self.sent_queue.iter().any(|s| {
                    s.occupied
                        && s.transmission_count >= self.config.transmission_limit
                });

                if limit_exceeded {
                    self.alert(actions);
                } else {
                    self.initiate_retransmission(actions);
                    self.state = FopState::Retransmitting;
                }
            }
            FopState::InitWithSetVr => {
                self.bc_transmission_count += 1;
                if self.bc_transmission_count >= self.config.transmission_limit
                {
                    self.alert(actions);
                } else {
                    actions.push(FopAction::TransmitBcSetVr {
                        vr: self.set_vr_value,
                    });
                    actions.push(FopAction::StartTimer);
                }
            }
            FopState::InitWithUnlock => {
                self.bc_transmission_count += 1;
                if self.bc_transmission_count >= self.config.transmission_limit
                {
                    self.alert(actions);
                } else {
                    actions.push(FopAction::TransmitBcUnlock);
                    actions.push(FopAction::StartTimer);
                }
            }
            FopState::InitNoBC | FopState::Initial => {}
        }
    }

    /// Initiate AD service without CLCW (direct to Active).
    fn handle_init_no_clcw(&mut self, actions: &mut FopActions) {
        match self.state {
            FopState::Initial => {
                self.purge_sent_queue();
                self.state = FopState::Active;
                actions.push(FopAction::Accept);
            }
            _ => {
                actions.push(FopAction::Reject);
            }
        }
    }

    /// Initiate AD service, wait for a matching CLCW.
    fn handle_init_with_clcw(&mut self, actions: &mut FopActions) {
        match self.state {
            FopState::Initial => {
                self.purge_sent_queue();
                self.state = FopState::InitNoBC;
                actions.push(FopAction::StartTimer);
            }
            _ => {
                actions.push(FopAction::Reject);
            }
        }
    }

    /// Initiate AD service by sending BC Set V(R).
    fn handle_init_set_vr(&mut self, vr: u8, actions: &mut FopActions) {
        match self.state {
            FopState::Initial => {
                self.set_vr_value = vr;
                self.bc_transmission_count = 1;
                self.purge_sent_queue();
                self.state = FopState::InitWithSetVr;
                actions.push(FopAction::TransmitBcSetVr { vr });
                actions.push(FopAction::StartTimer);
            }
            _ => {
                actions.push(FopAction::Reject);
            }
        }
    }

    /// Initiate AD service by sending BC Unlock.
    fn handle_init_unlock(&mut self, actions: &mut FopActions) {
        match self.state {
            FopState::Initial => {
                self.bc_transmission_count = 1;
                self.purge_sent_queue();
                self.state = FopState::InitWithUnlock;
                actions.push(FopAction::TransmitBcUnlock);
                actions.push(FopAction::StartTimer);
            }
            _ => {
                actions.push(FopAction::Reject);
            }
        }
    }

    /// Terminate AD service.
    fn handle_terminate(&mut self, actions: &mut FopActions) {
        actions.push(FopAction::StopTimer);
        self.purge_sent_queue();
        self.state = FopState::Initial;
        actions.push(FopAction::Terminated);
    }

    /// Resume AD service (go back to Active from Initial).
    fn handle_resume(&mut self, actions: &mut FopActions) {
        match self.state {
            FopState::Initial => {
                self.state = FopState::Active;
                actions.push(FopAction::Accept);
            }
            _ => {
                actions.push(FopAction::Reject);
            }
        }
    }

    /// Remove all frames acknowledged by N(R).
    fn remove_acknowledged(&mut self, nr: u8, actions: &mut FopActions) {
        for slot in &mut self.sent_queue {
            if !slot.occupied {
                continue;
            }
            // Frame is acknowledged if its seq is "before" N(R)
            // in the modulo-256 sequence space.
            let dist = nr.wrapping_sub(slot.seq);
            if dist > 0 && dist < 128 {
                let seq = slot.seq;
                slot.occupied = false;
                actions.push(FopAction::Acknowledged { seq });
            }
        }
    }

    /// Go-back-N: retransmit all unacknowledged frames in order.
    fn initiate_retransmission(&mut self, actions: &mut FopActions) {
        // Collect occupied slots sorted by sequence number
        let mut seqs: Vec<u8, WIN> = Vec::new();
        for slot in &self.sent_queue {
            if slot.occupied {
                let _ = seqs.push(slot.seq);
            }
        }

        // Sort by distance from NN(R) to get transmission order
        // (closest to NN(R) first)
        let nnr = self.nnr;
        // Simple insertion sort (WIN is small)
        for i in 1..seqs.len() {
            let key = seqs[i];
            let key_dist = key.wrapping_sub(nnr);
            let mut j = i;
            while j > 0 {
                let prev_dist = seqs[j - 1].wrapping_sub(nnr);
                if prev_dist <= key_dist {
                    break;
                }
                seqs[j] = seqs[j - 1];
                j -= 1;
            }
            seqs[j] = key;
        }

        for &seq in &seqs {
            for slot in &mut self.sent_queue {
                if slot.occupied && slot.seq == seq {
                    slot.transmission_count += 1;
                    break;
                }
            }
            actions.push(FopAction::TransmitAd { seq });
        }

        if !seqs.is_empty() {
            actions.push(FopAction::StartTimer);
        }
    }

    /// Enter alert state: stop timer, purge queue, go to Initial.
    fn alert(&mut self, actions: &mut FopActions) {
        actions.push(FopAction::StopTimer);
        self.purge_sent_queue();
        self.state = FopState::Initial;
        actions.push(FopAction::Alert);
    }

    /// Clear the sent queue and reset write position.
    fn purge_sent_queue(&mut self) {
        for slot in &mut self.sent_queue {
            slot.occupied = false;
        }
        self.write_pos = 0;
    }

    /// Find an empty slot in the sent queue.
    fn find_empty_slot(&self) -> Option<usize> {
        self.sent_queue.iter().position(|s| !s.occupied)
    }

    /// Buffer an FDU, returning the offset into the data buffer.
    fn buffer_fdu(&mut self, fdu: &[u8]) -> Option<usize> {
        if self.write_pos + fdu.len() > BUF {
            return None;
        }
        let offset = self.write_pos;
        self.data[offset..offset + fdu.len()].copy_from_slice(fdu);
        self.write_pos += fdu.len();
        Some(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> FopConfig {
        FopConfig::builder()
            .window_width(4)
            .transmission_limit(3)
            .timeout_type(0)
            .build()
    }

    #[test]
    fn starts_in_initial_state() {
        let fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        assert_eq!(fop.state(), FopState::Initial);
    }

    #[test]
    fn init_no_clcw_activates() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);

        assert_eq!(fop.state(), FopState::Active);
        assert!(actions.iter().any(|a| matches!(a, FopAction::Accept)));
    }

    #[test]
    fn send_ad_in_active_state() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendAd { fdu: &[1, 2, 3] },
            &mut actions,
        );

        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitAd { seq: 0 })));
        assert_eq!(fop.vs(), 1);
        assert_eq!(fop.sent_count(), 1);
    }

    #[test]
    fn send_ad_rejected_in_initial() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(
            FopEvent::SendAd { fdu: &[1, 2, 3] },
            &mut actions,
        );

        assert!(actions.iter().any(|a| matches!(a, FopAction::Reject)));
    }

    #[test]
    fn clcw_acknowledges_frames() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);

        // Send two frames
        fop.handle(
            FopEvent::SendAd { fdu: &[1] },
            &mut actions,
        );
        fop.handle(
            FopEvent::SendAd { fdu: &[2] },
            &mut actions,
        );
        assert_eq!(fop.sent_count(), 2);

        // CLCW with N(R)=2 acknowledges both
        let mut clcw = Clcw::new();
        clcw.set_report_value(2);
        fop.handle(
            FopEvent::ClcwReceived { clcw },
            &mut actions,
        );

        assert_eq!(fop.sent_count(), 0);
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::Acknowledged { seq: 0 })));
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::Acknowledged { seq: 1 })));
    }

    #[test]
    fn retransmit_flag_triggers_go_back_n() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendAd { fdu: &[1] },
            &mut actions,
        );
        fop.handle(
            FopEvent::SendAd { fdu: &[2] },
            &mut actions,
        );

        // CLCW with retransmit flag set, N(R)=0
        let mut clcw = Clcw::new();
        clcw.set_retransmit(true);
        clcw.set_report_value(0);
        fop.handle(
            FopEvent::ClcwReceived { clcw },
            &mut actions,
        );

        assert_eq!(fop.state(), FopState::Retransmitting);
        // Should retransmit both frames
        let transmit_count = actions
            .iter()
            .filter(|a| matches!(a, FopAction::TransmitAd { .. }))
            .count();
        assert_eq!(transmit_count, 2);
    }

    #[test]
    fn lockout_clcw_causes_alert() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);

        let mut clcw = Clcw::new();
        clcw.set_lockout(true);
        fop.handle(
            FopEvent::ClcwReceived { clcw },
            &mut actions,
        );

        assert_eq!(fop.state(), FopState::Initial);
        assert!(actions.iter().any(|a| matches!(a, FopAction::Alert)));
    }

    #[test]
    fn timer_expired_retransmits() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendAd { fdu: &[1] },
            &mut actions,
        );

        fop.handle(FopEvent::TimerExpired, &mut actions);

        assert_eq!(fop.state(), FopState::Retransmitting);
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitAd { seq: 0 })));
    }

    #[test]
    fn timer_expired_alerts_after_limit() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendAd { fdu: &[1] },
            &mut actions,
        );

        // Exhaust transmission limit (3)
        for _ in 0..3 {
            fop.handle(FopEvent::TimerExpired, &mut actions);
        }

        assert_eq!(fop.state(), FopState::Initial);
        assert!(actions.iter().any(|a| matches!(a, FopAction::Alert)));
    }

    #[test]
    fn init_with_unlock() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdUnlock, &mut actions);

        assert_eq!(fop.state(), FopState::InitWithUnlock);
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitBcUnlock)));

        // Simulate successful CLCW response
        let clcw = Clcw::new(); // lockout=false, retransmit=false
        fop.handle(
            FopEvent::ClcwReceived { clcw },
            &mut actions,
        );

        assert_eq!(fop.state(), FopState::Active);
        assert!(actions.iter().any(|a| matches!(a, FopAction::Accept)));
    }

    #[test]
    fn init_with_set_vr() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(
            FopEvent::InitAdSetVr { vr: 42 },
            &mut actions,
        );

        assert_eq!(fop.state(), FopState::InitWithSetVr);
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitBcSetVr { vr: 42 })));

        // CLCW confirms V(R)=42
        let mut clcw = Clcw::new();
        clcw.set_report_value(42);
        fop.handle(
            FopEvent::ClcwReceived { clcw },
            &mut actions,
        );

        assert_eq!(fop.state(), FopState::Active);
        assert_eq!(fop.vs(), 42);
    }

    #[test]
    fn terminate_resets_to_initial() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendAd { fdu: &[1] },
            &mut actions,
        );

        fop.handle(FopEvent::Terminate, &mut actions);

        assert_eq!(fop.state(), FopState::Initial);
        assert_eq!(fop.sent_count(), 0);
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::Terminated)));
    }

    #[test]
    fn bd_frame_can_be_sent() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendBd { fdu: &[0xAA, 0xBB] },
            &mut actions,
        );

        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitBd)));
        let bd_data = fop.get_bd_fdu().unwrap();
        assert_eq!(bd_data, &[0xAA, 0xBB]);
    }

    #[test]
    fn get_fdu_returns_correct_data() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);
        fop.handle(
            FopEvent::SendAd { fdu: &[10, 20, 30] },
            &mut actions,
        );

        let fdu = fop.get_fdu(0).unwrap();
        assert_eq!(fdu, &[10, 20, 30]);
    }

    #[test]
    fn wrapping_sequence_numbers() {
        let mut fop: FopMachine<8, 1024> = FopMachine::new(make_config());
        let mut actions = FopActions::new();

        fop.handle(FopEvent::InitAdNoClcw, &mut actions);

        // Set V(S) to 254 via init
        fop.handle(FopEvent::Terminate, &mut actions);
        fop.handle(
            FopEvent::InitAdSetVr { vr: 254 },
            &mut actions,
        );
        let mut clcw = Clcw::new();
        clcw.set_report_value(254);
        fop.handle(
            FopEvent::ClcwReceived { clcw },
            &mut actions,
        );
        assert_eq!(fop.vs(), 254);

        // Send frames across the wrap boundary
        fop.handle(
            FopEvent::SendAd { fdu: &[1] },
            &mut actions,
        );
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitAd { seq: 254 })));

        fop.handle(
            FopEvent::SendAd { fdu: &[2] },
            &mut actions,
        );
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitAd { seq: 255 })));

        fop.handle(
            FopEvent::SendAd { fdu: &[3] },
            &mut actions,
        );
        assert!(actions
            .iter()
            .any(|a| matches!(a, FopAction::TransmitAd { seq: 0 })));

        assert_eq!(fop.vs(), 1);
    }
}
