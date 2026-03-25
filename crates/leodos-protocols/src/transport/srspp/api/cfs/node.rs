use futures::FutureExt;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::runtime::time::sleep;

use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::transport::srspp::api::cfs::TransportError;
use crate::transport::srspp::dtn::AlwaysReachable;
use crate::transport::srspp::dtn::MessageStore;
use crate::transport::srspp::dtn::NoStore;
use crate::transport::srspp::dtn::Reachable;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::rto::RtoPolicy;
use crate::utils::cell::SyncRefCell;

use super::TimerSet;
use super::receiver::MultiReceiverState;
use super::receiver::SrsppReceiverDriver;
use super::receiver::SrsppRxHandle;
use super::sender::DtnContext;
use super::sender::SenderState;
use super::sender::SrsppSenderDriver;
use super::sender::SrsppTxHandle;
use super::sender::duration_until;

/// Combined SRSPP sender and receiver over a single link.
pub struct SrsppNode<
    E,
    S: MessageStore = NoStore,
    Re: Reachable = AlwaysReachable,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
    const MAX_STREAMS: usize = 1,
> {
    pub(super) sender: SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    pub(super) receiver: SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    dtn: SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
}

impl<
    E: Clone,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNode<E, S, Re, R, WIN, BUF, MTU, MAX_STREAMS>
{
    /// Creates a new node with sender and receiver configurations.
    pub fn new(sender_config: SenderConfig, receiver_config: ReceiverConfig, store: S, reachable: Re) -> Self {
        let origin = sender_config.source_address;
        let ack_delay = Duration::from_millis(receiver_config.ack_delay_ticks);
        Self {
            sender: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(sender_config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
            receiver: SyncRefCell::new(MultiReceiverState {
                config: receiver_config,
                streams: heapless::LinearMap::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
            dtn: SyncRefCell::new(DtnContext { store, reachable }),
            origin,
        }
    }

    /// Splits into separate tx/rx handles and a driver for I/O.
    pub fn split<L: NetworkWrite<Error = E> + NetworkRead<Error = E>, P: RtoPolicy>(
        &self,
        link: L,
        rto_policy: P,
    ) -> (
        SrsppRxHandle<'_, E, R, MAX_STREAMS>,
        SrsppTxHandle<'_, E, S, Re, WIN, BUF, MTU>,
        SrsppNodeDriver<'_, L, P, E, S, Re, R, WIN, BUF, MTU, MAX_STREAMS>,
    ) {
        (
            SrsppRxHandle {
                receiver: &self.receiver,
            },
            SrsppTxHandle {
                sender: &self.sender,
                dtn: &self.dtn,
                origin: self.origin,
            },
            SrsppNodeDriver {
                link,
                sender: SrsppSenderDriver::new(
                    rto_policy,
                    &self.sender,
                    &self.dtn,
                    self.origin,
                ),
                receiver: SrsppReceiverDriver::new(&self.receiver),
                recv_buffer: [0u8; MTU],
            },
        )
    }
}

/// I/O driver for a combined SRSPP sender/receiver node.
pub struct SrsppNodeDriver<
    'a,
    L,
    P: RtoPolicy,
    E,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> {
    link: L,
    sender: SrsppSenderDriver<'a, P, E, S, Re, WIN, BUF, MTU>,
    receiver: SrsppReceiverDriver<'a, E, R, MAX_STREAMS>,
    recv_buffer: [u8; MTU],
}

impl<
    'a,
    L: NetworkWrite<Error = E> + NetworkRead<Error = E>,
    P: RtoPolicy,
    E: Clone,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNodeDriver<'a, L, P, E, S, Re, R, WIN, BUF, MTU, MAX_STREAMS>
{
    /// Runs the combined send/receive I/O loop.
    pub async fn run(&mut self) -> Result<(), TransportError<E>> {
        loop {
            self.sender.drain_stored(&mut self.link).await?;

            if let Err(e) = self.sender.transmit(&mut self.link).await {
                self.set_both_errors(e.clone());
                return Err(e);
            }

            let timeout = self.next_timeout();

            let event = {
                let read_fut = self.link.read(&mut self.recv_buffer).fuse();
                let sleep_fut = sleep(timeout).fuse();
                pin_utils::pin_mut!(read_fut, sleep_fut);
                futures::select_biased! {
                    r = read_fut => Some(r),
                    _ = sleep_fut => None,
                }
            };

            match event {
                Some(Ok(len)) => {
                    if let Err(e) = self.handle_incoming(len).await {
                        self.set_both_errors(e.clone());
                        return Err(e);
                    }
                }
                Some(Err(e)) => {
                    let err = TransportError::Network(e);
                    self.set_both_errors(err.clone());
                    return Err(err);
                }
                None => {
                    if let Err(e) = self.sender.handle_timeouts(&mut self.link).await {
                        self.set_both_errors(e.clone());
                        return Err(e);
                    }
                    if let Err(e) = self.receiver.handle_timeouts(&mut self.link).await {
                        self.set_both_errors(e.clone());
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn handle_incoming(&mut self, len: usize) -> Result<(), TransportError<E>> {
        let packet = &self.recv_buffer[..len];
        let Ok(parsed) = SrsppPacket::parse(packet) else {
            return Ok(());
        };
        match parsed.srspp_type() {
            Ok(SrsppType::Data) => self.receiver.process_data(packet, &mut self.link).await,
            Ok(SrsppType::Eos) => self.receiver.process_data(packet, &mut self.link).await,
            Ok(SrsppType::Ack) => self.sender.process_ack(packet),
            Err(_) => Ok(()),
        }
    }

    fn next_timeout(&self) -> Duration {
        let s = self.sender.next_deadline();
        let r = self.receiver.next_deadline();
        let deadline = match (s, r) {
            (Some(a), Some(b)) => Some(if a < b { a } else { b }),
            (a, b) => a.or(b),
        };
        duration_until(deadline)
    }

    fn set_both_errors(&self, err: TransportError<E>) {
        self.sender.sender.with_mut(|s| s.error = Some(err.clone()));
        self.receiver.state.with_mut(|s| s.error = Some(err));
    }
}
