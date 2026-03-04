use core::cell::RefCell;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::network::NetworkLayer;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::ReceiverAction;
use crate::transport::srspp::machine::receiver::ReceiverActions;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverEvent;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_ack_packet;
use crate::transport::srspp::packet::parse_data_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use crate::transport::srspp::rto::RtoPolicy;
use heapless::index_map::FnvIndexMap;

use super::Error;
use super::TimerSet;
use super::receiver::{MultiReceiverState, StreamState, SrsppRxHandle};
use super::sender::{SenderState, SrsppTxHandle};

/// Combined SRSPP sender and receiver over a single link.
pub struct SrsppNode<
    E,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
    const MAX_STREAMS: usize = 4,
> {
    sender: RefCell<SenderState<E, WIN, BUF, MTU>>,
    receiver: RefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
}

impl<
    E: Clone,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNode<E, R, WIN, BUF, MTU, MAX_STREAMS>
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
        SrsppRxHandle<'_, E, R, MAX_STREAMS>,
        SrsppTxHandle<'_, E, WIN, BUF, MTU>,
        SrsppNodeDriver<'_, L, P, E, R, WIN, BUF, MTU, MAX_STREAMS>,
    ) {
        (
            SrsppRxHandle {
                receiver: &self.receiver,
            },
            SrsppTxHandle {
                sender: &self.sender,
            },
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

/// I/O driver for a combined SRSPP sender/receiver node.
pub struct SrsppNodeDriver<
    'a,
    L: NetworkLayer,
    P: RtoPolicy,
    E,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> {
    link: L,
    rto_policy: P,
    node: &'a SrsppNode<E, R, WIN, BUF, MTU, MAX_STREAMS>,
    recv_buffer: [u8; MTU],
    tx_buffer: [u8; MTU],
    ack_buffer: [u8; 32],
}

impl<
    'a,
    L: NetworkLayer,
    P: RtoPolicy,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNodeDriver<'a, L, P, L::Error, R, WIN, BUF, MTU, MAX_STREAMS>
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
                        machine: R::new(config.clone(), source_address),
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
