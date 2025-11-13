use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::datalink::DataLink;
use crate::network::spp::Apid;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::ReceiverAction;
use crate::transport::srspp::machine::receiver::ReceiverActions;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverError;
use crate::transport::srspp::machine::receiver::ReceiverEvent;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderError;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet;
use crate::transport::srspp::packet::SrspAckPacket;
use crate::transport::srspp::packet::SrspType;
use crate::transport::srspp::packet::parse_ack_packet;
use crate::transport::srspp::packet::parse_data_packet;
use crate::transport::srspp::packet::parse_srsp_type;

// ============================================================================
// Error type
// ============================================================================

#[derive(Debug, Clone)]
pub enum Error<E> {
    Sender(SenderError),
    Receiver(ReceiverError),
    Link(E),
    Packet(packet::SrspPacketError),
}

impl<E> From<SenderError> for Error<E> {
    fn from(e: SenderError) -> Self {
        Error::Sender(e)
    }
}

impl<E> From<ReceiverError> for Error<E> {
    fn from(e: ReceiverError) -> Self {
        Error::Receiver(e)
    }
}

impl<E> From<packet::SrspPacketError> for Error<E> {
    fn from(e: packet::SrspPacketError) -> Self {
        Error::Packet(e)
    }
}

// ============================================================================
// Timer set
// ============================================================================

struct TimerSet<const N: usize> {
    timers: [(u16, Option<SysTime>); N],
}

impl<const N: usize> TimerSet<N> {
    fn new() -> Self {
        Self {
            timers: [(0, None); N],
        }
    }

    fn start(&mut self, seq: u16, deadline: SysTime) {
        for slot in &mut self.timers {
            if slot.1.is_none() {
                *slot = (seq, Some(deadline));
                return;
            }
        }
    }

    fn stop(&mut self, seq: u16) {
        for slot in &mut self.timers {
            if slot.0 == seq && slot.1.is_some() {
                slot.1 = None;
            }
        }
    }

    fn expired(&mut self, now: SysTime) -> impl Iterator<Item = u16> + '_ {
        self.timers.iter_mut().filter_map(move |slot| {
            if let Some(deadline) = slot.1 {
                if now >= deadline {
                    slot.1 = None;
                    return Some(slot.0);
                }
            }
            None
        })
    }

    fn next_deadline(&self) -> Option<SysTime> {
        self.timers
            .iter()
            .filter_map(|(_, deadline)| *deadline)
            .min()
    }
}

// ============================================================================
// Sender
// ============================================================================

struct SenderState<E, const WIN: usize, const BUF: usize, const MTU: usize> {
    machine: SenderMachine<WIN, BUF, MTU>,
    actions: SenderActions,
    timers: TimerSet<WIN>,
    rto: Duration,
    closed: bool,
    error: Option<Error<E>>,
}

/// Channel that owns the sender state. Split into handle + driver.
pub struct SrspSenderChannel<E, const WIN: usize, const BUF: usize, const MTU: usize> {
    state: RefCell<SenderState<E, WIN, BUF, MTU>>,
}

impl<E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>
    SrspSenderChannel<E, WIN, BUF, MTU>
{
    pub fn new(config: SenderConfig) -> Self {
        let rto = Duration::from_millis(config.rto_ticks);
        Self {
            state: RefCell::new(SenderState {
                machine: SenderMachine::new(config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                rto,
                closed: false,
                error: None,
            }),
        }
    }

    pub fn split<L: DataLink<Error = E>>(
        &self,
        link: L,
    ) -> (
        SrspSenderHandle<'_, E, WIN, BUF, MTU>,
        SrspSenderDriver<'_, L, WIN, BUF, MTU>,
    ) {
        (
            SrspSenderHandle { channel: self },
            SrspSenderDriver {
                link,
                channel: self,
                recv_buffer: [0u8; MTU],
            },
        )
    }
}

/// Handle for sending data. Used by the application.
pub struct SrspSenderHandle<'a, E, const WIN: usize, const BUF: usize, const MTU: usize> {
    channel: &'a SrspSenderChannel<E, WIN, BUF, MTU>,
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>
    SrspSenderHandle<'a, E, WIN, BUF, MTU>
{
    /// Send data, waiting for buffer space if needed.
    pub async fn send(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        // Wait for space or error
        poll_fn(|_cx| {
            let state = self.channel.state.borrow();

            // Check for driver error
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            // Check for space
            if state.machine.available_bytes() >= data.len() && state.machine.available_window() > 0
            {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await?;

        // Queue the data
        {
            let mut state = self.channel.state.borrow_mut();
            let SenderState {
                machine, actions, ..
            } = &mut *state;
            machine.handle(SenderEvent::SendRequest { data }, actions)?;
        }
        Ok(())
    }

    /// Signal that no more data will be sent.
    /// Driver will exit once all pending data is acknowledged.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Check available buffer space in bytes.
    pub fn available_bytes(&self) -> usize {
        self.channel.state.borrow().machine.available_bytes()
    }

    /// Check available window slots.
    pub fn available_window(&self) -> usize {
        self.channel.state.borrow().machine.available_window()
    }

    /// Check if all data has been acknowledged.
    pub fn is_idle(&self) -> bool {
        self.channel.state.borrow().machine.is_idle()
    }
}

/// Driver that handles I/O. Runs as a concurrent task.
pub struct SrspSenderDriver<'a, L: DataLink, const WIN: usize, const BUF: usize, const MTU: usize> {
    link: L,
    channel: &'a SrspSenderChannel<L::Error, WIN, BUF, MTU>,
    recv_buffer: [u8; MTU],
}

impl<'a, L: DataLink, const WIN: usize, const BUF: usize, const MTU: usize>
    SrspSenderDriver<'a, L, WIN, BUF, MTU>
where
    L::Error: Clone,
{
    /// Run the driver loop.
    pub async fn run(&mut self) -> Result<(), Error<L::Error>> {
        loop {
            // Check if done
            {
                let state = self.channel.state.borrow();
                if state.closed && state.machine.is_idle() {
                    return Ok(());
                }
            }

            // Process pending transmits
            if let Err(e) = self.process_transmits().await {
                self.channel.state.borrow_mut().error = Some(e.clone());
                return Err(e);
            }

            // Calculate timeout
            let timeout = self.duration_until_next_timeout();

            // Wait for ACK or timeout
            match select_either(self.link.recv(&mut self.recv_buffer), sleep(timeout)).await {
                Either::Left(result) => match result {
                    Ok(len) => self.handle_ack(len)?,
                    Err(e) => {
                        let err = Error::Link(e);
                        self.channel.state.borrow_mut().error = Some(err.clone());
                        return Err(err);
                    }
                },
                Either::Right(()) => {
                    if let Err(e) = self.handle_timeouts().await {
                        self.channel.state.borrow_mut().error = Some(e.clone());
                        return Err(e);
                    }
                }
            }
        }
    }

    fn duration_until_next_timeout(&self) -> Duration {
        let now = SysTime::now();
        self.channel
            .state
            .borrow()
            .timers
            .next_deadline()
            .map(|deadline| {
                if deadline > now {
                    Duration::from(deadline - now)
                } else {
                    Duration::zero()
                }
            })
            .unwrap_or(Duration::from_secs(60))
    }

    async fn process_transmits(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        // Collect transmit actions
        let transmits: heapless::Vec<SequenceCount, WIN> = {
            let state = self.channel.state.borrow();
            state
                .actions
                .iter()
                .filter_map(|a| match a {
                    SenderAction::Transmit { seq, .. } => Some(*seq),
                    _ => None,
                })
                .collect()
        };

        // Send each packet
        for seq in transmits {
            let packet_data: Option<heapless::Vec<u8, MTU>> = {
                let state = self.channel.state.borrow();
                state
                    .machine
                    .get_packet(seq)
                    .map(|p| p.iter().copied().collect())
            };

            if let Some(packet_data) = packet_data {
                self.link.send(&packet_data).await.map_err(Error::Link)?;

                let mut state = self.channel.state.borrow_mut();
                let SenderState {
                    machine,
                    timers,
                    rto,
                    ..
                } = &mut *state;
                machine.mark_transmitted(seq);
                timers.start(seq.value(), now + SysTime::from(*rto));
            }
        }

        // Process stop timer actions
        {
            let mut state = self.channel.state.borrow_mut();
            let SenderState {
                actions, timers, ..
            } = &mut *state;
            for action in actions.iter() {
                if let SenderAction::StopTimer { seq } = action {
                    timers.stop(seq.value());
                }
            }
        }

        Ok(())
    }

    fn handle_ack(&mut self, len: usize) -> Result<(), Error<L::Error>> {
        let packet = &self.recv_buffer[..len];

        if let Ok(SrspType::Ack) = parse_srsp_type(packet) {
            if let Ok(ack) = parse_ack_packet(packet) {
                let SenderState {
                    machine,
                    actions,
                    timers,
                    ..
                } = &mut *self.channel.state.borrow_mut();

                machine.handle(
                    SenderEvent::AckReceived {
                        cumulative_ack: ack.ack_payload.cumulative_ack(),
                        selective_bitmap: ack.ack_payload.selective_ack_bitmap(),
                    },
                    actions,
                )?;

                // Process stop timer actions
                for action in actions.iter() {
                    if let SenderAction::StopTimer { seq } = action {
                        timers.stop(seq.value());
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_timeouts(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        let expired: heapless::Vec<u16, WIN> = {
            let mut state = self.channel.state.borrow_mut();
            state.timers.expired(now).collect()
        };

        for seq in expired {
            {
                let mut state = self.channel.state.borrow_mut();
                let SenderState {
                    machine, actions, ..
                } = &mut *state;
                machine.handle(
                    SenderEvent::RetransmitTimeout {
                        seq: SequenceCount::from(seq),
                    },
                    actions,
                )?;
            }

            self.process_transmits().await?;
        }

        Ok(())
    }
}

// ============================================================================
// Receiver
// ============================================================================

struct ReceiverState<E, const WIN: usize, const BUF: usize, const REASM: usize> {
    machine: ReceiverMachine<WIN, BUF, REASM>,
    actions: ReceiverActions,
    ack_deadline: Option<SysTime>,
    ack_delay: Duration,
    apid: Apid,
    closed: bool,
    error: Option<Error<E>>,
}

/// Channel that owns the receiver state. Split into handle + driver.
pub struct SrspReceiverChannel<E, const WIN: usize, const BUF: usize, const REASM: usize> {
    state: RefCell<ReceiverState<E, WIN, BUF, REASM>>,
}

impl<E: Clone, const WIN: usize, const BUF: usize, const REASM: usize>
    SrspReceiverChannel<E, WIN, BUF, REASM>
{
    pub fn new(config: ReceiverConfig) -> Self {
        let apid = config.apid;
        let ack_delay = Duration::from_millis(config.ack_delay_ticks);
        Self {
            state: RefCell::new(ReceiverState {
                machine: ReceiverMachine::new(config),
                actions: ReceiverActions::new(),
                ack_deadline: None,
                ack_delay,
                apid,
                closed: false,
                error: None,
            }),
        }
    }

    pub fn split<L: DataLink<Error = E>, const MTU: usize>(
        &self,
        link: L,
    ) -> (
        SrspReceiverHandle<'_, E, WIN, BUF, REASM>,
        SrspReceiverDriver<'_, L, WIN, BUF, MTU, REASM>,
    ) {
        (
            SrspReceiverHandle { channel: self },
            SrspReceiverDriver {
                link,
                channel: self,
                recv_buffer: [0u8; MTU],
                ack_buffer: [0u8; 16],
            },
        )
    }
}

/// Handle for receiving data. Used by the application.
pub struct SrspReceiverHandle<'a, E, const WIN: usize, const BUF: usize, const REASM: usize> {
    channel: &'a SrspReceiverChannel<E, WIN, BUF, REASM>,
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const REASM: usize>
    SrspReceiverHandle<'a, E, WIN, BUF, REASM>
{
    /// Receive next message, waiting if none available.
    pub async fn recv(&mut self) -> Result<heapless::Vec<u8, REASM>, Error<E>> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            // Check for driver error
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            // Check for message
            if let Some(msg) = state.machine.take_message() {
                Poll::Ready(Ok(msg.iter().copied().collect()))
            } else {
                Poll::Pending
            }
        })
        .await
    }

    /// Signal that no more receives are expected.
    /// Driver will exit.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Check if there's a message ready.
    pub fn has_message(&self) -> bool {
        self.channel.state.borrow().machine.has_message()
    }
}

/// Driver that handles I/O. Runs as a concurrent task.
pub struct SrspReceiverDriver<
    'a,
    L: DataLink,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
> {
    link: L,
    channel: &'a SrspReceiverChannel<L::Error, WIN, BUF, REASM>,
    recv_buffer: [u8; MTU],
    ack_buffer: [u8; 16],
}

impl<'a, L: DataLink, const WIN: usize, const BUF: usize, const MTU: usize, const REASM: usize>
    SrspReceiverDriver<'a, L, WIN, BUF, MTU, REASM>
where
    L::Error: Clone,
{
    /// Run the driver loop.
    pub async fn run(&mut self) -> Result<(), Error<L::Error>> {
        loop {
            // Check if closed
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            // Calculate timeout
            let timeout = self.duration_until_ack_timeout();

            // Wait for data or ACK timeout
            match select_either(self.link.recv(&mut self.recv_buffer), sleep(timeout)).await {
                Either::Left(result) => match result {
                    Ok(len) => {
                        if let Err(e) = self.handle_data(len).await {
                            self.channel.state.borrow_mut().error = Some(e.clone());
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        let err = Error::Link(e);
                        self.channel.state.borrow_mut().error = Some(err.clone());
                        return Err(err);
                    }
                },
                Either::Right(()) => {
                    if let Err(e) = self.handle_ack_timeout().await {
                        self.channel.state.borrow_mut().error = Some(e.clone());
                        return Err(e);
                    }
                }
            }
        }
    }

    fn duration_until_ack_timeout(&self) -> Duration {
        let now = SysTime::now();
        self.channel
            .state
            .borrow()
            .ack_deadline
            .map(|deadline| {
                if deadline > now {
                    Duration::from(deadline - now)
                } else {
                    Duration::zero()
                }
            })
            .unwrap_or(Duration::from_secs(60))
    }

    async fn handle_data(&mut self, len: usize) -> Result<(), Error<L::Error>> {
        let packet = &self.recv_buffer[..len];

        if let Ok(SrspType::Data) = parse_srsp_type(packet) {
            if let Ok(data) = parse_data_packet(packet) {
                {
                    let mut state = self.channel.state.borrow_mut();
                    let ReceiverState {
                        machine, actions, ..
                    } = &mut *state;
                    machine.handle(
                        ReceiverEvent::DataReceived {
                            seq: data.primary.sequence_count(),
                            flags: data.primary.sequence_flag(),
                            payload: &data.payload,
                        },
                        actions,
                    )?;
                }

                self.process_actions().await?;
            }
        }

        Ok(())
    }

    async fn handle_ack_timeout(&mut self) -> Result<(), Error<L::Error>> {
        let should_handle = {
            let state = self.channel.state.borrow();
            state
                .ack_deadline
                .map(|d| SysTime::now() >= d)
                .unwrap_or(false)
        };

        if should_handle {
            {
                let mut state = self.channel.state.borrow_mut();
                let ReceiverState {
                    machine,
                    actions,
                    ack_deadline,
                    ..
                } = &mut *state;
                *ack_deadline = None;
                machine.handle(ReceiverEvent::AckTimeout, actions)?;
            }

            self.process_actions().await?;
        }

        Ok(())
    }

    async fn process_actions(&mut self) -> Result<(), Error<L::Error>> {
        // Collect ACK to send
        let ack_to_send: Option<(SequenceCount, u16)> = {
            let state = self.channel.state.borrow();
            state.actions.iter().find_map(|a| match a {
                ReceiverAction::SendAck {
                    cumulative_ack,
                    selective_bitmap,
                } => Some((*cumulative_ack, *selective_bitmap)),
                _ => None,
            })
        };

        // Send ACK if needed
        if let Some((cumulative_ack, selective_bitmap)) = ack_to_send {
            let apid = self.channel.state.borrow().apid;

            let ack = SrspAckPacket::builder()
                .buffer(&mut self.ack_buffer)
                .apid(apid)
                .cumulative_ack(cumulative_ack)
                .selective_bitmap(selective_bitmap)
                .sequence_count(SequenceCount::from(0))
                .build()?;

            self.link
                .send(zerocopy::IntoBytes::as_bytes(ack))
                .await
                .map_err(Error::Link)?;
        }

        // Process timer actions
        {
            let mut state = self.channel.state.borrow_mut();
            let ReceiverState {
                actions,
                ack_deadline,
                ack_delay,
                ..
            } = &mut *state;
            for action in actions.iter() {
                match action {
                    ReceiverAction::StartAckTimer { ticks: _ } => {
                        *ack_deadline = Some(SysTime::now() + SysTime::from(*ack_delay));
                    }
                    ReceiverAction::StopAckTimer => {
                        *ack_deadline = None;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
