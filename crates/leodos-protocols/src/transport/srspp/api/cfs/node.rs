use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::transport::srspp::api::cfs::TransportError;
use crate::transport::srspp::machine::receiver::ReceiverActions;
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
use heapless::index_map::FnvIndexMap;

use super::TimerSet;
use super::receiver::MultiReceiverState;
use super::receiver::SrsppRxHandle;
use super::receiver::drive_data;
use super::receiver::drive_receiver_timeouts;
use super::receiver::receiver_next_deadline;
use super::sender::DtnContext;
use super::sender::SenderState;
use super::sender::SrsppTxHandle;
use super::sender::drive_ack;
use super::sender::drive_sender_timeouts;
use super::sender::drive_transmits;
use super::sender::duration_until;
use super::sender::sender_next_deadline;
use crate::transport::srspp::dtn::AlwaysReachable;
use crate::transport::srspp::dtn::NoStore;

/// Combined SRSPP sender and receiver over a single link.
pub struct SrsppNode<
    E,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
    const MAX_STREAMS: usize = 4,
> {
    /// Interior-mutable sender state.
    pub(super) sender: SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    /// Interior-mutable multi-stream receiver state.
    pub(super) receiver: SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    /// No-op DTN context (node doesn't use DTN).
    noop_dtn: SyncRefCell<DtnContext<NoStore, AlwaysReachable>>,
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
            sender: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(sender_config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
            receiver: SyncRefCell::new(MultiReceiverState {
                config: receiver_config,
                streams: FnvIndexMap::new(),
                actions: ReceiverActions::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
            noop_dtn: SyncRefCell::new(DtnContext {
                store: NoStore,
                reachable: AlwaysReachable,
            }),
        }
    }

    /// Splits into separate tx/rx handles and a driver for I/O.
    pub fn split<L: NetworkWrite<Error = E> + NetworkRead<Error = E>, P: RtoPolicy>(
        &self,
        link: L,
        rto_policy: P,
    ) -> (
        SrsppRxHandle<'_, E, R, MAX_STREAMS>,
        SrsppTxHandle<'_, E, NoStore, AlwaysReachable, WIN, BUF, MTU>,
        SrsppNodeDriver<'_, L, P, E, R, WIN, BUF, MTU, MAX_STREAMS>,
    ) {
        (
            SrsppRxHandle {
                receiver: &self.receiver,
            },
            SrsppTxHandle {
                sender: &self.sender,
                dtn: &self.noop_dtn,
                origin: Address::ground(0),
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
    L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>,
    P: RtoPolicy,
    E,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> {
    /// Network link for bidirectional packet I/O.
    link: L,
    /// Policy for computing retransmission timeouts.
    rto_policy: P,
    /// Reference to the owning node.
    node: &'a SrsppNode<E, R, WIN, BUF, MTU, MAX_STREAMS>,
    /// Buffer for receiving packets from the link.
    recv_buffer: [u8; MTU],
    /// Buffer for building outgoing data packets.
    tx_buffer: [u8; MTU],
    /// Buffer for building outgoing ACK packets.
    ack_buffer: [u8; 32],
}

impl<
    'a,
    L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>,
    P: RtoPolicy,
    R: ReceiverBackend,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNodeDriver<'a, L, P, <L as NetworkWrite>::Error, R, WIN, BUF, MTU, MAX_STREAMS>
where
    <L as NetworkWrite>::Error: Clone,
{
    /// Runs the combined send/receive I/O loop.
    pub async fn run(&mut self) -> Result<(), TransportError<<L as NetworkWrite>::Error>> {
        loop {
            if let Err(e) = drive_transmits(
                &self.node.sender,
                &mut self.tx_buffer,
                &mut self.link,
                &self.rto_policy,
            )
            .await
            {
                self.set_both_errors(e.clone());
                return Err(e);
            }

            let timeout = self.next_timeout();

            match select_either(self.link.read(&mut self.recv_buffer), sleep(timeout)).await {
                Either::Left(result) => match result {
                    Ok(len) => {
                        if let Err(e) = self.handle_incoming(len).await {
                            self.set_both_errors(e.clone());
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        let err = TransportError::Network(e);
                        self.set_both_errors(err.clone());
                        return Err(err);
                    }
                },
                Either::Right(()) => {
                    if let Err(e) = drive_sender_timeouts(
                        &self.node.sender,
                        &mut self.tx_buffer,
                        &mut self.link,
                        &self.rto_policy,
                    )
                    .await
                    {
                        self.set_both_errors(e.clone());
                        return Err(e);
                    }
                    if let Err(e) = drive_receiver_timeouts(
                        &self.node.receiver,
                        &mut self.ack_buffer,
                        &mut self.link,
                    )
                    .await
                    {
                        self.set_both_errors(e.clone());
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Dispatches an incoming packet to the ACK or data handler.
    async fn handle_incoming(
        &mut self,
        len: usize,
    ) -> Result<(), TransportError<<L as NetworkWrite>::Error>> {
        let packet = &self.recv_buffer[..len];
        let Ok(parsed) = SrsppPacket::parse(packet) else {
            return Ok(());
        };
        match parsed.srspp_type() {
            Ok(SrsppType::Data) => {
                drive_data(
                    &self.node.receiver,
                    packet,
                    &mut self.ack_buffer,
                    &mut self.link,
                )
                .await
            }
            Ok(SrsppType::Ack) => drive_ack(&self.node.sender, packet),
            Err(_) => Ok(()),
        }
    }

    fn next_timeout(&self) -> Duration {
        let s = sender_next_deadline(&self.node.sender);
        let r = receiver_next_deadline(&self.node.receiver);
        let deadline = match (s, r) {
            (Some(a), Some(b)) => Some(if a < b { a } else { b }),
            (a, b) => a.or(b),
        };
        duration_until(deadline)
    }

    fn set_both_errors(&self, err: TransportError<<L as NetworkWrite>::Error>) {
        self.node.sender.with_mut(|s| s.error = Some(err.clone()));
        self.node.receiver.with_mut(|s| s.error = Some(err));
    }
}
