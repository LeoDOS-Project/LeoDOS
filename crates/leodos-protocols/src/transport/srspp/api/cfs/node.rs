
use futures::FutureExt;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::runtime::time::sleep;

use crate::buffer_pool::BufferPool;
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
use super::receiver::ACK_BUFFER_SIZE;
use super::receiver::MultiReceiverState;
use super::receiver::SrsppRxHandle;
use super::receiver::handle_timeouts as receiver_handle_timeouts;
use super::receiver::next_deadline as receiver_next_deadline;
use super::receiver::process_data;
use super::sender::DtnContext;
use super::sender::SenderState;
use super::sender::SrsppTxHandle;
use super::sender::drain_stored;
use super::sender::duration_until;
use super::sender::handle_timeouts as sender_handle_timeouts;
use super::sender::next_deadline as sender_next_deadline;
use super::sender::process_ack;
use super::sender::transmit;

/// Combined SRSPP sender and receiver over a single link.
pub struct SrsppNode<
    'pool,
    E,
    P: BufferPool + 'pool,
    S: MessageStore = NoStore,
    Re: Reachable = AlwaysReachable,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const WIN: usize = 8,
    const MTU: usize = 512,
    const MAX_STREAMS: usize = 1,
> {
    pub(super) sender: SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    pub(super) receiver: SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    dtn: SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
}

impl<
    'pool,
    E: Clone,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    const WIN: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNode<'pool, E, P, S, Re, R, WIN, MTU, MAX_STREAMS>
{
    /// Creates a new node with sender and receiver configurations.
    ///
    /// `buf_size` is the size in bytes of the sender's send buffer
    /// (formerly the `BUF` const generic).
    pub fn new(
        pool: &'pool P,
        buf_size: usize,
        sender_config: SenderConfig,
        receiver_config: ReceiverConfig,
        store: S,
        reachable: Re,
    ) -> Result<Self, P::Error> {
        let origin = sender_config.source_address;
        let ack_delay = Duration::from_millis(receiver_config.ack_delay_ticks);
        Ok(Self {
            sender: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(sender_config, pool, buf_size)?,
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
        })
    }

    /// Splits into separate tx/rx handles and a driver for I/O.
    pub fn split<L: NetworkWrite<Error = E> + NetworkRead<Error = E>, Rto: RtoPolicy>(
        &self,
        link: L,
        rto_policy: Rto,
        pool: &'pool P,
        mtu: usize,
    ) -> Result<(
        SrsppRxHandle<'_, E, R, MAX_STREAMS>,
        SrsppTxHandle<'_, 'pool, E, S, Re, P, WIN, MTU>,
        SrsppNodeDriver<'_, 'pool, L, Rto, E, S, Re, R, P, WIN, MTU, MAX_STREAMS>,
    ), P::Error> {
        Ok((
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
                sender: &self.sender,
                receiver: &self.receiver,
                dtn: &self.dtn,
                origin: self.origin,
                rto_policy,
                tx_buffer: pool.alloc_bytes(mtu)?,
                recv_buffer: pool.alloc_bytes(mtu)?,
                ack_buffer: [0u8; ACK_BUFFER_SIZE],
            },
        ))
    }
}

/// I/O driver for a combined SRSPP sender/receiver node.
///
/// Holds the sender state directly (not as an embedded
/// `SrsppSenderDriver`) and dispatches incoming packets to either the
/// sender's free-function helpers or the receiver driver. This avoids
/// the second `recv_buffer` an embedded sender driver would allocate
/// for its own (unused) standalone read loop.
pub struct SrsppNodeDriver<
    'a,
    'pool,
    L,
    Rto: RtoPolicy,
    E,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> {
    link: L,
    sender: &'a SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    receiver: &'a SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    dtn: &'a SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
    rto_policy: Rto,
    tx_buffer: P::Buf<'pool>,
    recv_buffer: P::Buf<'pool>,
    ack_buffer: [u8; ACK_BUFFER_SIZE],
}

impl<
    'a,
    'pool,
    L: NetworkWrite<Error = E> + NetworkRead<Error = E>,
    Rto: RtoPolicy,
    E: Clone,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppNodeDriver<'a, 'pool, L, Rto, E, S, Re, R, P, WIN, MTU, MAX_STREAMS>
{
    /// Runs the combined send/receive I/O loop.
    pub async fn run(&mut self) -> Result<(), TransportError<E>> {
        loop {
            drain_stored(
                self.sender,
                self.dtn,
                &mut self.tx_buffer[..],
                self.origin,
                &self.rto_policy,
                &mut self.link,
            )
            .await?;

            if let Err(e) = transmit(
                self.sender,
                &mut self.tx_buffer[..],
                &self.rto_policy,
                &mut self.link,
            )
            .await
            {
                self.set_both_errors(e.clone());
                return Err(e);
            }

            let timeout = self.next_timeout();

            let event = {
                let read_fut = self.link.read(&mut self.recv_buffer[..]).fuse();
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
                    if let Err(e) = sender_handle_timeouts(
                        self.sender,
                        &mut self.tx_buffer[..],
                        &self.rto_policy,
                        &mut self.link,
                    )
                    .await
                    {
                        self.set_both_errors(e.clone());
                        return Err(e);
                    }
                    if let Err(e) = receiver_handle_timeouts(
                        self.receiver,
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

    async fn handle_incoming(&mut self, len: usize) -> Result<(), TransportError<E>> {
        let packet = &self.recv_buffer[..len];
        let Ok(parsed) = SrsppPacket::parse(packet) else {
            return Ok(());
        };
        match parsed.srspp_type() {
            Ok(SrsppType::Data) | Ok(SrsppType::Eos) => {
                process_data(self.receiver, &mut self.ack_buffer, packet, &mut self.link).await
            }
            Ok(SrsppType::Ack) => process_ack(self.sender, packet),
            Err(_) => Ok(()),
        }
    }

    fn next_timeout(&self) -> Duration {
        let s = sender_next_deadline(self.sender);
        let r = receiver_next_deadline(self.receiver);
        let deadline = match (s, r) {
            (Some(a), Some(b)) => Some(if a < b { a } else { b }),
            (a, b) => a.or(b),
        };
        duration_until(deadline)
    }

    fn set_both_errors(&self, err: TransportError<E>) {
        self.sender.with_mut(|s| s.error = Some(err.clone()));
        self.receiver.with_mut(|s| s.error = Some(err));
    }
}
