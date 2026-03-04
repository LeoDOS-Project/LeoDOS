use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use zerocopy::{Immutable, IntoBytes};

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::application::spacecomp::io::writer::MessageSender;
use crate::network::NetworkLayer;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_ack_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use crate::transport::srspp::rto::RtoPolicy;

use super::Error;
use super::TimerSet;

pub(super) struct SenderState<E, const WIN: usize, const BUF: usize, const MTU: usize> {
    pub(super) machine: SenderMachine<WIN, BUF, MTU>,
    pub(super) actions: SenderActions,
    pub(super) timers: TimerSet<WIN>,
    pub(super) closed: bool,
    pub(super) error: Option<Error<E>>,
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
        SrsppTxHandle<'_, E, WIN, BUF, MTU>,
        SrsppSenderDriver<'_, L, P, WIN, BUF, MTU>,
    ) {
        (
            SrsppTxHandle { sender: &self.state },
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
            {
                let state = self.channel.state.borrow();
                if state.closed && state.machine.is_idle() {
                    return Ok(());
                }
            }

            if let Err(e) = self.process_transmits().await {
                self.channel.state.borrow_mut().error = Some(e.clone());
                return Err(e);
            }

            let timeout = self.duration_until_next_timeout();

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

/// Handle for sending data over an SRSPP node.
pub struct SrsppTxHandle<'a, E, const WIN: usize, const BUF: usize, const MTU: usize> {
    pub(super) sender: &'a RefCell<SenderState<E, WIN, BUF, MTU>>,
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppTxHandle<'a, E, WIN, BUF, MTU>
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

    /// Signal that no more data will be sent.
    /// Driver will exit once all pending data is acknowledged.
    pub fn close(&mut self) {
        self.sender.borrow_mut().closed = true;
    }

    /// Check available buffer space in bytes.
    pub fn available_bytes(&self) -> usize {
        self.sender.borrow().machine.available_bytes()
    }

    /// Check available window slots.
    pub fn available_window(&self) -> usize {
        self.sender.borrow().machine.available_window()
    }

    /// Check if all data has been acknowledged.
    pub fn is_idle(&self) -> bool {
        self.sender.borrow().machine.is_idle()
    }
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize> MessageSender
    for SrsppTxHandle<'a, E, WIN, BUF, MTU>
{
    type Error = Error<E>;

    async fn send_message(&mut self, target: Address, data: &[u8]) -> Result<(), Self::Error> {
        self.send(target, data).await
    }
}
