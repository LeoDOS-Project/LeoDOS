use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use zerocopy::{Immutable, IntoBytes};

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::network::NetworkLayer;
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
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_ack_packet;
use crate::transport::srspp::packet::parse_data_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use crate::transport::srspp::rto::RtoPolicy;

// ============================================================================
// Error type
// ============================================================================

/// Errors from the SRSPP CFS transport layer.
#[derive(Debug, Clone)]
pub enum Error<E> {
    /// The sender state machine reported an error.
    Sender(SenderError),
    /// The receiver state machine reported an error.
    Receiver(ReceiverError),
    /// The underlying network link failed.
    Link(E),
    /// A packet could not be built or parsed.
    Packet(packet::SrsppPacketError),
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

impl<E> From<packet::SrsppPacketError> for Error<E> {
    fn from(e: packet::SrsppPacketError) -> Self {
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
    closed: bool,
    error: Option<Error<E>>,
}

/// Channel that owns the sender state. Split into handle + driver.
pub struct SrsppSender<E, const WIN: usize = 8, const BUF: usize = 4096, const MTU: usize = 512> {
    state: RefCell<SenderState<E, WIN, BUF, MTU>>,
}

impl<E: Clone, const WIN: usize, const BUF: usize, const MTU: usize> SrsppSender<E, WIN, BUF, MTU> {
    /// Creates a new sender with the given configuration.
    pub fn new(config: SenderConfig) -> Self {
        Self {
            state: RefCell::new(SenderState {
                machine: SenderMachine::new(config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
        }
    }

    /// Splits into a handle for sending and a driver for I/O.
    pub fn split<L: NetworkLayer<Error = E>, P: RtoPolicy>(
        &self,
        link: L,
        rto_policy: P,
    ) -> (
        SrsppSenderHandle<'_, E, WIN, BUF, MTU>,
        SrsppSenderDriver<'_, L, P, WIN, BUF, MTU>,
    ) {
        (
            SrsppSenderHandle { channel: self },
            SrsppSenderDriver {
                link,
                rto_policy,
                channel: self,
                recv_buffer: [0u8; MTU],
                tx_buffer: [0u8; MTU],
            },
        )
    }
}

/// Handle for sending data. Used by the application.
pub struct SrsppSenderHandle<'a, E, const WIN: usize, const BUF: usize, const MTU: usize> {
    channel: &'a SrsppSender<E, WIN, BUF, MTU>,
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSenderHandle<'a, E, WIN, BUF, MTU>
{
    /// Send data, waiting for buffer space if needed.
    pub async fn send(
        &mut self,
        target: Address,
        data: &(impl IntoBytes + Immutable + ?Sized),
    ) -> Result<(), Error<E>> {
        let data = data.as_bytes();
        poll_fn(|_cx| {
            let state = self.channel.state.borrow();

            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            if state.machine.available_bytes() >= data.len() && state.machine.available_window() > 0
            {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await?;

        {
            let mut state = self.channel.state.borrow_mut();
            let SenderState {
                machine, actions, ..
            } = &mut *state;
            machine.handle(SenderEvent::SendRequest { target, data }, actions)?;
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
pub struct SrsppSenderDriver<
    'a,
    L: NetworkLayer,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    link: L,
    rto_policy: P,
    channel: &'a SrsppSender<L::Error, WIN, BUF, MTU>,
    recv_buffer: [u8; MTU],
    tx_buffer: [u8; MTU],
}

impl<'a, L: NetworkLayer, P: RtoPolicy, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSenderDriver<'a, L, P, WIN, BUF, MTU>
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

        let (transmits, cfg_clone): (heapless::Vec<SequenceCount, WIN>, SenderConfig) = {
            let state = self.channel.state.borrow();
            let t = state
                .actions
                .iter()
                .filter_map(|a| match a {
                    SenderAction::Transmit { seq, .. } => Some(*seq),
                    _ => None,
                })
                .collect();
            (t, state.machine.config().clone())
        };

        for seq in transmits {
            let packet_len = {
                let state = self.channel.state.borrow();
                if let Some(info) = state.machine.get_payload(seq) {
                    let pkt = SrsppDataPacket::builder()
                        .buffer(&mut self.tx_buffer)
                        .source_address(cfg_clone.source_address)
                        .target(info.target)
                        .apid(cfg_clone.apid)
                        .function_code(cfg_clone.function_code)
                        .message_id(cfg_clone.message_id)
                        .action_code(cfg_clone.action_code)
                        .sequence_count(seq)
                        .sequence_flag(info.flags)
                        .payload_len(info.payload.len())
                        .build()
                        .map_err(Error::Packet)?;
                    pkt.payload.copy_from_slice(info.payload);
                    Some(SrsppDataPacket::HEADER_SIZE + info.payload.len())
                } else {
                    None
                }
            };

            if let Some(packet_len) = packet_len {
                self.link
                    .send(&self.tx_buffer[..packet_len])
                    .await
                    .map_err(Error::Link)?;

                let rto = Duration::from_millis(self.rto_policy.rto_ticks(now.seconds()));

                let mut state = self.channel.state.borrow_mut();
                let SenderState {
                    machine, timers, ..
                } = &mut *state;
                machine.mark_transmitted(seq);
                timers.start(seq.value(), now + SysTime::from(rto));
            }
        }

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

        if let Ok(SrsppType::Ack) = parse_srspp_type(packet) {
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

use crate::network::isl::address::Address;
use heapless::index_map::FnvIndexMap;

struct StreamState<const WIN: usize, const BUF: usize, const REASM: usize> {
    machine: ReceiverMachine<WIN, BUF, REASM>,
    ack_deadline: Option<SysTime>,
    progress_deadline: Option<SysTime>,
}

struct MultiReceiverState<
    E,
    const WIN: usize,
    const BUF: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> {
    config: ReceiverConfig,
    streams: FnvIndexMap<Address, StreamState<WIN, BUF, REASM>, MAX_STREAMS>,
    actions: ReceiverActions,
    ack_delay: Duration,
    closed: bool,
    error: Option<Error<E>>,
}

/// Channel that owns the receiver state. Split into handle + driver.
///
/// Supports receiving from multiple senders simultaneously. Each sender is
/// identified by its source address and has independent stream state.
pub struct SrsppReceiver<
    E,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const REASM: usize = 8192,
    const MAX_STREAMS: usize = 4,
> {
    state: RefCell<MultiReceiverState<E, WIN, BUF, REASM, MAX_STREAMS>>,
}

impl<E: Clone, const WIN: usize, const BUF: usize, const REASM: usize, const MAX_STREAMS: usize>
    SrsppReceiver<E, WIN, BUF, REASM, MAX_STREAMS>
{
    /// Creates a new multi-stream receiver.
    pub fn new(config: ReceiverConfig) -> Self {
        let ack_delay = Duration::from_millis(config.ack_delay_ticks);
        Self {
            state: RefCell::new(MultiReceiverState {
                config,
                streams: FnvIndexMap::new(),
                actions: ReceiverActions::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
        }
    }

    /// Splits into a handle for receiving and a driver for I/O.
    pub fn split<L: NetworkLayer<Error = E>, const MTU: usize>(
        &self,
        link: L,
    ) -> (
        SrsppReceiverHandle<'_, E, WIN, BUF, REASM, MAX_STREAMS>,
        SrsppReceiverDriver<'_, L, WIN, BUF, MTU, REASM, MAX_STREAMS>,
    ) {
        (
            SrsppReceiverHandle { channel: self },
            SrsppReceiverDriver {
                link,
                channel: self,
                recv_buffer: [0u8; MTU],
                ack_buffer: [0u8; 32],
            },
        )
    }
}

/// Handle for receiving data. Used by the application.
pub struct SrsppReceiverHandle<
    'a,
    E,
    const WIN: usize,
    const BUF: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> {
    channel: &'a SrsppReceiver<E, WIN, BUF, REASM, MAX_STREAMS>,
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const REASM: usize, const MAX_STREAMS: usize>
    SrsppReceiverHandle<'a, E, WIN, BUF, REASM, MAX_STREAMS>
{
    /// Receive next message from any sender, waiting if none available.
    /// Returns `(source_address, bytes_written)`.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), Error<E>> {
        poll_fn(|_cx| {
            let mut state = self.channel.state.borrow_mut();

            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }

            for (source, stream) in state.streams.iter_mut() {
                if let Some(msg) = stream.machine.take_message() {
                    let len = msg.len().min(buf.len());
                    buf[..len].copy_from_slice(&msg[..len]);
                    return Poll::Ready(Ok((*source, len)));
                }
            }

            Poll::Pending
        })
        .await
    }

    /// Signal that no more receives are expected.
    /// Driver will exit.
    pub fn close(&mut self) {
        self.channel.state.borrow_mut().closed = true;
    }

    /// Check if there's a message ready from any sender.
    pub fn has_message(&self) -> bool {
        let state = self.channel.state.borrow();
        state.streams.values().any(|s| s.machine.has_message())
    }

    /// Get the number of active streams.
    pub fn stream_count(&self) -> usize {
        self.channel.state.borrow().streams.len()
    }
}

/// Driver that handles I/O. Runs as a concurrent task.
pub struct SrsppReceiverDriver<
    'a,
    L: NetworkLayer,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> {
    link: L,
    channel: &'a SrsppReceiver<L::Error, WIN, BUF, REASM, MAX_STREAMS>,
    recv_buffer: [u8; MTU],
    ack_buffer: [u8; 32],
}

impl<
    'a,
    L: NetworkLayer,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> SrsppReceiverDriver<'a, L, WIN, BUF, MTU, REASM, MAX_STREAMS>
where
    L::Error: Clone,
{
    /// Run the driver loop.
    pub async fn run(&mut self) -> Result<(), Error<L::Error>> {
        loop {
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            let timeout = self.duration_until_next_timeout();

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
        let state = self.channel.state.borrow();
        state
            .streams
            .values()
            .flat_map(|s| [s.ack_deadline, s.progress_deadline])
            .flatten()
            .min()
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

        if let Ok(SrsppType::Data) = parse_srspp_type(packet) {
            if let Ok(data) = parse_data_packet(packet) {
                let source_address = data.srspp_header.source_address();
                let seq = data.primary.sequence_count();
                let flags = data.primary.sequence_flag();

                {
                    let mut state = self.channel.state.borrow_mut();
                    let MultiReceiverState {
                        config,
                        streams,
                        actions,
                        ..
                    } = &mut *state;

                    if !streams.contains_key(&source_address) {
                        let stream_state = StreamState {
                            machine: ReceiverMachine::new(config.clone(), source_address),
                            ack_deadline: None,
                            progress_deadline: None,
                        };
                        let _ = streams.insert(source_address, stream_state);
                    }

                    if let Some(stream) = streams.get_mut(&source_address) {
                        stream.machine.handle(
                            ReceiverEvent::DataReceived {
                                seq,
                                flags,
                                payload: &data.payload,
                            },
                            actions,
                        )?;
                    }
                }

                self.process_actions_for_stream(source_address).await?;
            }
        }

        Ok(())
    }

    async fn handle_timeouts(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        let ack_expired: heapless::Vec<Address, MAX_STREAMS> = {
            let state = self.channel.state.borrow();
            state
                .streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream.ack_deadline.filter(|&d| now >= d).map(|_| *source)
                })
                .collect()
        };

        for source in ack_expired {
            {
                let mut state = self.channel.state.borrow_mut();
                let MultiReceiverState {
                    streams, actions, ..
                } = &mut *state;
                if let Some(stream) = streams.get_mut(&source) {
                    stream.ack_deadline = None;
                    stream.machine.handle(ReceiverEvent::AckTimeout, actions)?;
                }
            }
            self.process_actions_for_stream(source).await?;
        }

        let progress_expired: heapless::Vec<Address, MAX_STREAMS> = {
            let state = self.channel.state.borrow();
            state
                .streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream
                        .progress_deadline
                        .filter(|&d| now >= d)
                        .map(|_| *source)
                })
                .collect()
        };

        for source in progress_expired {
            {
                let mut state = self.channel.state.borrow_mut();
                let MultiReceiverState {
                    streams, actions, ..
                } = &mut *state;
                if let Some(stream) = streams.get_mut(&source) {
                    stream.progress_deadline = None;
                    stream
                        .machine
                        .handle(ReceiverEvent::ProgressTimeout, actions)?;
                }
            }
            self.process_actions_for_stream(source).await?;
        }

        Ok(())
    }

    async fn process_actions_for_stream(&mut self, source: Address) -> Result<(), Error<L::Error>> {
        let (ack_to_send, timer_actions, ack_delay) = {
            let state = self.channel.state.borrow();
            let ack = state.actions.iter().find_map(|a| match a {
                ReceiverAction::SendAck {
                    destination,
                    cumulative_ack,
                    selective_bitmap,
                } => Some((
                    state.config.local_address,
                    state.config.apid,
                    state.config.function_code,
                    state.config.message_id,
                    state.config.action_code,
                    *destination,
                    *cumulative_ack,
                    *selective_bitmap,
                )),
                _ => None,
            });

            let timer: heapless::Vec<ReceiverAction, 8> = state
                .actions
                .iter()
                .filter(|a| {
                    matches!(
                        a,
                        ReceiverAction::StartAckTimer { .. }
                            | ReceiverAction::StopAckTimer
                            | ReceiverAction::StartProgressTimer { .. }
                            | ReceiverAction::StopProgressTimer
                    )
                })
                .copied()
                .collect();

            (ack, timer, state.ack_delay)
        };

        if let Some((
            local_address,
            apid,
            function_code,
            message_id,
            action_code,
            destination,
            cumulative_ack,
            selective_bitmap,
        )) = ack_to_send
        {
            let ack = SrsppAckPacket::builder()
                .buffer(&mut self.ack_buffer)
                .source_address(local_address)
                .target(destination)
                .apid(apid)
                .function_code(function_code)
                .message_id(message_id)
                .action_code(action_code)
                .cumulative_ack(cumulative_ack)
                .selective_bitmap(selective_bitmap)
                .sequence_count(SequenceCount::from(0))
                .build()?;

            self.link
                .send(zerocopy::IntoBytes::as_bytes(ack))
                .await
                .map_err(Error::Link)?;
        }

        {
            let mut state = self.channel.state.borrow_mut();
            for action in timer_actions {
                match action {
                    ReceiverAction::StartAckTimer { .. } => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.ack_deadline = Some(SysTime::now() + SysTime::from(ack_delay));
                        }
                    }
                    ReceiverAction::StopAckTimer => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.ack_deadline = None;
                        }
                    }
                    ReceiverAction::StartProgressTimer { ticks } => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            let delay = Duration::from_millis(ticks);
                            entry.progress_deadline = Some(SysTime::now() + SysTime::from(delay));
                        }
                    }
                    ReceiverAction::StopProgressTimer => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.progress_deadline = None;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

// ============================================================================
// TransportSender / TransportReceiver impls
// ============================================================================

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const REASM: usize, const MAX_STREAMS: usize>
    crate::transport::TransportReceiver
    for SrsppReceiverHandle<'a, E, WIN, BUF, REASM, MAX_STREAMS>
{
    type Error = Error<E>;

    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let (_, len) = self.recv(buf).await?;
        Ok(len)
    }
}

// ============================================================================
// SrsppNode — combined sender + receiver over a single link
// ============================================================================

/// Combined SRSPP sender and receiver over a single link.
pub struct SrsppNode<
    E,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
    const REASM: usize = 8192,
    const MAX_STREAMS: usize = 4,
> {
    sender: RefCell<SenderState<E, WIN, BUF, MTU>>,
    receiver: RefCell<MultiReceiverState<E, WIN, BUF, REASM, MAX_STREAMS>>,
}

impl<
    E: Clone,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> SrsppNode<E, WIN, BUF, MTU, REASM, MAX_STREAMS>
{
    /// Creates a new node with sender and receiver configurations.
    pub fn new(sender_config: SenderConfig, receiver_config: ReceiverConfig) -> Self {
        let ack_delay = Duration::from_millis(receiver_config.ack_delay_ticks);
        Self {
            sender: RefCell::new(SenderState {
                machine: SenderMachine::new(sender_config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
            receiver: RefCell::new(MultiReceiverState {
                config: receiver_config,
                streams: FnvIndexMap::new(),
                actions: ReceiverActions::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
        }
    }

    /// Splits into separate tx/rx handles and a driver for I/O.
    pub fn split<L: NetworkLayer<Error = E>, P: RtoPolicy>(
        &self,
        link: L,
        rto_policy: P,
    ) -> (
        SrsppRxHandle<'_, E, WIN, BUF, MTU, REASM, MAX_STREAMS>,
        SrsppTxHandle<'_, E, WIN, BUF, MTU>,
        SrsppNodeDriver<'_, L, P, E, WIN, BUF, MTU, REASM, MAX_STREAMS>,
    ) {
        (
            SrsppRxHandle { receiver: &self.receiver },
            SrsppTxHandle { sender: &self.sender },
            SrsppNodeDriver {
                link,
                rto_policy,
                node: self,
                recv_buffer: [0u8; MTU],
                tx_buffer: [0u8; MTU],
                ack_buffer: [0u8; 32],
            },
        )
    }
}

/// Handle for sending data over an SRSPP node.
pub struct SrsppTxHandle<
    'a,
    E,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    sender: &'a RefCell<SenderState<E, WIN, BUF, MTU>>,
}

impl<
    'a,
    E: Clone,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> SrsppTxHandle<'a, E, WIN, BUF, MTU>
{
    /// Sends data to the given target, waiting for buffer space.
    pub async fn send(
        &mut self,
        target: impl Into<Address>,
        data: &(impl IntoBytes + Immutable + ?Sized),
    ) -> Result<(), Error<E>> {
        let data = data.as_bytes();
        poll_fn(|_cx| {
            let state = self.sender.borrow();
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }
            if state.machine.available_bytes() >= data.len() && state.machine.available_window() > 0
            {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        })
        .await?;

        {
            let mut state = self.sender.borrow_mut();
            let SenderState {
                machine, actions, ..
            } = &mut *state;
            machine.handle(
                SenderEvent::SendRequest {
                    target: target.into(),
                    data,
                },
                actions,
            )?;
        }
        Ok(())
    }
}

/// Handle for receiving data from an SRSPP node.
pub struct SrsppRxHandle<
    'a,
    E,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> {
    receiver: &'a RefCell<MultiReceiverState<E, WIN, BUF, REASM, MAX_STREAMS>>,
}

impl<
    'a,
    E: Clone,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> SrsppRxHandle<'a, E, WIN, BUF, MTU, REASM, MAX_STREAMS>
{
    /// Receives the next message, returning source address and length.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), Error<E>> {
        poll_fn(|_cx| {
            let mut state = self.receiver.borrow_mut();
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }
            for (source, stream) in state.streams.iter_mut() {
                if let Some(msg) = stream.machine.take_message() {
                    let len = msg.len().min(buf.len());
                    buf[..len].copy_from_slice(&msg[..len]);
                    return Poll::Ready(Ok((*source, len)));
                }
            }
            Poll::Pending
        })
        .await
    }
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>
    crate::application::spacecomp::io::writer::MessageSender
    for SrsppTxHandle<'a, E, WIN, BUF, MTU>
{
    type Error = Error<E>;

    async fn send_message(
        &mut self,
        target: Address,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.send(target, data).await
    }
}

/// I/O driver for a combined SRSPP sender/receiver node.
pub struct SrsppNodeDriver<
    'a,
    L: NetworkLayer,
    P: RtoPolicy,
    E,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> {
    link: L,
    rto_policy: P,
    node: &'a SrsppNode<E, WIN, BUF, MTU, REASM, MAX_STREAMS>,
    recv_buffer: [u8; MTU],
    tx_buffer: [u8; MTU],
    ack_buffer: [u8; 32],
}

impl<
    'a,
    L: NetworkLayer,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const REASM: usize,
    const MAX_STREAMS: usize,
> SrsppNodeDriver<'a, L, P, L::Error, WIN, BUF, MTU, REASM, MAX_STREAMS>
where
    L::Error: Clone,
{
    /// Runs the combined send/receive I/O loop.
    pub async fn run(&mut self) -> Result<(), Error<L::Error>> {
        loop {
            self.process_sender_transmits().await?;

            let timeout = self.next_timeout();

            match select_either(self.link.recv(&mut self.recv_buffer), sleep(timeout)).await {
                Either::Left(result) => match result {
                    Ok(len) => self.handle_incoming(len).await?,
                    Err(e) => {
                        let err = Error::Link(e);
                        self.node.sender.borrow_mut().error = Some(err.clone());
                        self.node.receiver.borrow_mut().error = Some(err.clone());
                        return Err(err);
                    }
                },
                Either::Right(()) => {
                    self.handle_sender_timeouts().await?;
                    self.handle_receiver_timeouts().await?;
                }
            }
        }
    }

    fn next_timeout(&self) -> Duration {
        let now = SysTime::now();
        let sender_deadline = self.node.sender.borrow().timers.next_deadline();
        let receiver_deadline = {
            let state = self.node.receiver.borrow();
            state
                .streams
                .values()
                .flat_map(|s| [s.ack_deadline, s.progress_deadline])
                .flatten()
                .min()
        };
        let deadline = match (sender_deadline, receiver_deadline) {
            (Some(a), Some(b)) => Some(if a < b { a } else { b }),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        deadline
            .map(|d| {
                if d > now {
                    Duration::from(d - now)
                } else {
                    Duration::zero()
                }
            })
            .unwrap_or(Duration::from_secs(60))
    }

    async fn handle_incoming(&mut self, len: usize) -> Result<(), Error<L::Error>> {
        let packet = &self.recv_buffer[..len];
        match parse_srspp_type(packet) {
            Ok(SrsppType::Data) => self.handle_data(len).await,
            Ok(SrsppType::Ack) => {
                self.handle_ack(len)?;
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }

    fn handle_ack(&mut self, len: usize) -> Result<(), Error<L::Error>> {
        let packet = &self.recv_buffer[..len];
        if let Ok(ack) = parse_ack_packet(packet) {
            let mut state = self.node.sender.borrow_mut();
            let SenderState {
                machine,
                actions,
                timers,
                ..
            } = &mut *state;
            machine.handle(
                SenderEvent::AckReceived {
                    cumulative_ack: ack.ack_payload.cumulative_ack(),
                    selective_bitmap: ack.ack_payload.selective_ack_bitmap(),
                },
                actions,
            )?;
            for action in actions.iter() {
                if let SenderAction::StopTimer { seq } = action {
                    timers.stop(seq.value());
                }
            }
        }
        Ok(())
    }

    async fn handle_data(&mut self, len: usize) -> Result<(), Error<L::Error>> {
        let packet = &self.recv_buffer[..len];
        if let Ok(data) = parse_data_packet(packet) {
            let source_address = data.srspp_header.source_address();
            let seq = data.primary.sequence_count();
            let flags = data.primary.sequence_flag();

            {
                let mut state = self.node.receiver.borrow_mut();
                let MultiReceiverState {
                    config,
                    streams,
                    actions,
                    ..
                } = &mut *state;

                if !streams.contains_key(&source_address) {
                    let stream_state = StreamState {
                        machine: ReceiverMachine::new(config.clone(), source_address),
                        ack_deadline: None,
                        progress_deadline: None,
                    };
                    let _ = streams.insert(source_address, stream_state);
                }

                if let Some(stream) = streams.get_mut(&source_address) {
                    stream.machine.handle(
                        ReceiverEvent::DataReceived {
                            seq,
                            flags,
                            payload: &data.payload,
                        },
                        actions,
                    )?;
                }
            }

            self.process_receiver_actions(source_address).await?;
        }
        Ok(())
    }

    async fn process_sender_transmits(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        let (transmits, cfg_clone): (heapless::Vec<SequenceCount, WIN>, SenderConfig) = {
            let state = self.node.sender.borrow();
            let t = state
                .actions
                .iter()
                .filter_map(|a| match a {
                    SenderAction::Transmit { seq, .. } => Some(*seq),
                    _ => None,
                })
                .collect();
            (t, state.machine.config().clone())
        };

        for seq in transmits {
            let packet_len = {
                let state = self.node.sender.borrow();
                if let Some(info) = state.machine.get_payload(seq) {
                    let pkt = SrsppDataPacket::builder()
                        .buffer(&mut self.tx_buffer)
                        .source_address(cfg_clone.source_address)
                        .target(info.target)
                        .apid(cfg_clone.apid)
                        .function_code(cfg_clone.function_code)
                        .message_id(cfg_clone.message_id)
                        .action_code(cfg_clone.action_code)
                        .sequence_count(seq)
                        .sequence_flag(info.flags)
                        .payload_len(info.payload.len())
                        .build()
                        .map_err(Error::Packet)?;
                    pkt.payload.copy_from_slice(info.payload);
                    Some(SrsppDataPacket::HEADER_SIZE + info.payload.len())
                } else {
                    None
                }
            };

            if let Some(packet_len) = packet_len {
                self.link
                    .send(&self.tx_buffer[..packet_len])
                    .await
                    .map_err(Error::Link)?;

                let rto = Duration::from_millis(self.rto_policy.rto_ticks(now.seconds()));

                let mut state = self.node.sender.borrow_mut();
                let SenderState {
                    machine, timers, ..
                } = &mut *state;
                machine.mark_transmitted(seq);
                timers.start(seq.value(), now + SysTime::from(rto));
            }
        }

        {
            let mut state = self.node.sender.borrow_mut();
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

    async fn handle_sender_timeouts(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        let expired: heapless::Vec<u16, WIN> = {
            let mut state = self.node.sender.borrow_mut();
            state.timers.expired(now).collect()
        };

        for seq in expired {
            {
                let mut state = self.node.sender.borrow_mut();
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
            self.process_sender_transmits().await?;
        }

        Ok(())
    }

    async fn handle_receiver_timeouts(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        let ack_expired: heapless::Vec<Address, MAX_STREAMS> = {
            let state = self.node.receiver.borrow();
            state
                .streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream.ack_deadline.filter(|&d| now >= d).map(|_| *source)
                })
                .collect()
        };

        for source in ack_expired {
            {
                let mut state = self.node.receiver.borrow_mut();
                let MultiReceiverState {
                    streams, actions, ..
                } = &mut *state;
                if let Some(stream) = streams.get_mut(&source) {
                    stream.ack_deadline = None;
                    stream.machine.handle(ReceiverEvent::AckTimeout, actions)?;
                }
            }
            self.process_receiver_actions(source).await?;
        }

        let progress_expired: heapless::Vec<Address, MAX_STREAMS> = {
            let state = self.node.receiver.borrow();
            state
                .streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream
                        .progress_deadline
                        .filter(|&d| now >= d)
                        .map(|_| *source)
                })
                .collect()
        };

        for source in progress_expired {
            {
                let mut state = self.node.receiver.borrow_mut();
                let MultiReceiverState {
                    streams, actions, ..
                } = &mut *state;
                if let Some(stream) = streams.get_mut(&source) {
                    stream.progress_deadline = None;
                    stream
                        .machine
                        .handle(ReceiverEvent::ProgressTimeout, actions)?;
                }
            }
            self.process_receiver_actions(source).await?;
        }

        Ok(())
    }

    async fn process_receiver_actions(&mut self, source: Address) -> Result<(), Error<L::Error>> {
        let (ack_to_send, timer_actions, ack_delay) = {
            let state = self.node.receiver.borrow();
            let ack = state.actions.iter().find_map(|a| match a {
                ReceiverAction::SendAck {
                    destination,
                    cumulative_ack,
                    selective_bitmap,
                } => Some((
                    state.config.local_address,
                    state.config.apid,
                    state.config.function_code,
                    state.config.message_id,
                    state.config.action_code,
                    *destination,
                    *cumulative_ack,
                    *selective_bitmap,
                )),
                _ => None,
            });

            let timer: heapless::Vec<ReceiverAction, 8> = state
                .actions
                .iter()
                .filter(|a| {
                    matches!(
                        a,
                        ReceiverAction::StartAckTimer { .. }
                            | ReceiverAction::StopAckTimer
                            | ReceiverAction::StartProgressTimer { .. }
                            | ReceiverAction::StopProgressTimer
                    )
                })
                .copied()
                .collect();

            (ack, timer, state.ack_delay)
        };

        if let Some((
            local_address,
            apid,
            function_code,
            message_id,
            action_code,
            destination,
            cumulative_ack,
            selective_bitmap,
        )) = ack_to_send
        {
            let ack = SrsppAckPacket::builder()
                .buffer(&mut self.ack_buffer)
                .source_address(local_address)
                .target(destination)
                .apid(apid)
                .function_code(function_code)
                .message_id(message_id)
                .action_code(action_code)
                .cumulative_ack(cumulative_ack)
                .selective_bitmap(selective_bitmap)
                .sequence_count(SequenceCount::from(0))
                .build()?;

            self.link
                .send(zerocopy::IntoBytes::as_bytes(ack))
                .await
                .map_err(Error::Link)?;
        }

        {
            let mut state = self.node.receiver.borrow_mut();
            for action in timer_actions {
                match action {
                    ReceiverAction::StartAckTimer { .. } => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.ack_deadline = Some(SysTime::now() + SysTime::from(ack_delay));
                        }
                    }
                    ReceiverAction::StopAckTimer => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.ack_deadline = None;
                        }
                    }
                    ReceiverAction::StartProgressTimer { ticks } => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            let delay = Duration::from_millis(ticks);
                            entry.progress_deadline = Some(SysTime::now() + SysTime::from(delay));
                        }
                    }
                    ReceiverAction::StopProgressTimer => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.progress_deadline = None;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}
