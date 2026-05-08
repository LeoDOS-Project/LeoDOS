//! [`SpaceCompNode`] — the main entry point for running
//! a SpaceCoMP computation on a cFS satellite.
#![allow(async_fn_in_trait)]

use core::time::Duration;
use leodos_libcfs::cfe::es::pool::MemPool;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::join;
use leodos_libcfs::log;

use crate::job::Job;
use crate::packet::AssignCollectorPayload;
use crate::packet::AssignMapperPayload;
use crate::packet::AssignReducerPayload;
use crate::packet::OpCode;
use crate::packet::ParseError;
use crate::packet::SpaceCompMessage;
use crate::transport::Rx;
use crate::transport::Tx;
use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::SpacecraftId;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::transport::srspp::api::cfs::EndpointListener;
use leodos_protocols::transport::srspp::api::cfs::EndpointSender;
use leodos_protocols::transport::srspp::api::cfs::RecvKind;
use leodos_protocols::transport::srspp::api::cfs::SrsppEndpoint;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::MessageStore;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::dtn::Reachable;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

use crate::SpaceCompConfig;
use crate::SpaceCompError;

/// Listener handle alias used by SpaceCoMP role functions.
pub type Listener<
    'a,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const RX_BUF: usize = 8192,
    const MAX_STREAMS: usize = 4,
> = EndpointListener<'a, CfsError, ReceiverMachine<WIN, BUF, RX_BUF>, MAX_STREAMS>;

/// Sender handle alias used by SpaceCoMP role functions.
pub type Sender<
    'a,
    'pool,
    S = NoStore,
    Re = AlwaysReachable,
    const WIN: usize = 8,
    const MTU: usize = 512,
> = EndpointSender<'a, 'pool, CfsError, MemPool, S, Re, WIN, MTU>;

/// A SpaceCoMP node that handles SRSPP transport,
/// message dispatch, and coordinator orchestration.
///
/// Type parameters:
/// - `F`: closure that constructs the app after startup sync
/// - `S`: message store for DTN (default: [`NoStore`])
/// - `R`: reachability oracle for DTN (default: [`AlwaysReachable`])
#[derive(bon::Builder)]
pub struct SpaceCompNode<F, S: MessageStore = NoStore, R: Reachable = AlwaysReachable> {
    app_fn: F,
    config: SpaceCompConfig,
    store: S,
    reachable: R,
}

/// App-defined computation for each SpaceCoMP role.
///
/// Implement this to define what happens when this node
/// is assigned as a collector, mapper, or reducer.
pub trait SpaceComp {
    /// Called after startup sync to initialize hardware.
    fn init(&mut self) -> Result<(), SpaceCompError> {
        Ok(())
    }

    /// Collects local data and sends it to the assigned mapper.
    async fn collect(&mut self, tx: impl Tx) -> Result<(), SpaceCompError>;

    /// Processes one message from a collector. Called once per received chunk.
    async fn map(&mut self, data: &[u8], tx: impl Tx) -> Result<(), SpaceCompError>;

    /// Aggregates results from mappers and sends the final output.
    async fn reduce(&mut self, rx: impl Rx, tx: impl Tx) -> Result<(), SpaceCompError>;
}

impl<F, S: MessageStore, R: Reachable> SpaceCompNode<F, S, R> {
    /// Starts the node with default SRSPP buffer sizes.
    ///
    /// Takes ownership of `pool` — the cFE memory pool used for
    /// SRSPP buffers (sender data, driver tx, driver recv). The
    /// pool's lifetime is tied to this call, which never returns.
    pub fn start<A: SpaceComp>(self, pool: MemPool) -> !
    where
        F: FnOnce() -> Result<A, SpaceCompError>,
    {
        leodos_libcfs::runtime::Runtime::new()
            .run(self.run::<A, 8, 4096, 512, 8192, 4, 4>(pool))
    }

    /// Runs the node with custom SRSPP buffer sizes.
    pub async fn run<
        A: SpaceComp,
        const WIN: usize,
        const BUF: usize,
        const MTU: usize,
        const RX_BUF: usize,
        const MAX_TX: usize,
        const MAX_STREAMS: usize,
    >(
        self,
        pool: MemPool,
    ) -> Result<(), SpaceCompError>
    where
        F: FnOnce() -> Result<A, SpaceCompError>,
    {
        event::register(&[])?;
        log!("SpaceCoMP node starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let Some(address) = scid.to_address(self.config.num_orbits, self.config.num_sats) else {
            log!("Invalid spacecraft ID")?;
            return Ok(());
        };
        let Address::Satellite(point) = address else { unreachable!() };

        let recv_mid = MsgId::local_cmd(self.config.router_recv_topic);
        let send_mid = MsgId::local_cmd(self.config.router_send_topic);
        let sb_link = SbDatalink::new("SPCOMP_ISL", 32, recv_mid, send_mid)?;
        let network = PointToPoint::new(sb_link);

        let apid = self.config.apid;
        let rto = self.config.rto_ms;

        let sender_config = SenderConfig::builder()
            .source_address(address)
            .apid(apid)
            .function_code(0)
            .rto_ticks(rto)
            .max_retransmits(3)
            .header_overhead(SrsppDataPacket::HEADER_SIZE)
            .build();

        let receiver_config = ReceiverConfig::builder()
            .local_address(address)
            .apid(apid)
            .function_code(0)
            .immediate_ack(true)
            .ack_delay_ticks(100)
            .build();

        let endpoint: SrsppEndpoint<
            '_,
            CfsError,
            MemPool,
            S,
            R,
            ReceiverMachine<WIN, BUF, RX_BUF>,
            WIN,
            MTU,
            MAX_TX,
            MAX_STREAMS,
        > = SrsppEndpoint::new(
            &pool,
            BUF,
            sender_config,
            receiver_config,
            self.store,
            self.reachable,
        )?;

        let mut listener = endpoint
            .listener()
            .map_err(|_| SpaceCompError::Cfs(CfsError::ExternalResourceFail))?;

        system::wait_for_startup_sync(Duration::from_millis(10_000));

        let mut app = (self.app_fn)()?;
        let shell = self.config.shell();
        let mut recv_buf = [0u8; SpaceCompMessage::MAX_DISPATCH_SIZE];
        let mut msg_buf = [0u8; SpaceCompMessage::MAX_ASSIGN_SIZE];

        let dispatch = async {
            let mut initialized = false;
            loop {
                let Ok((_source, kind)) = listener.recv(&mut recv_buf).await else {
                    break;
                };
                let len = match kind {
                    RecvKind::Data(n) => n,
                    // SpaceComp tracks completion via app-level PhaseDone;
                    // the SRSPP-level EOS is not load-bearing here.
                    RecvKind::Eos => continue,
                };
                if !initialized {
                    app.init()?;
                    initialized = true;
                }
                let Ok(msg) = SpaceCompMessage::parse(&recv_buf[..len]) else {
                    continue;
                };
                let Ok(op) = msg.op_code() else { continue };
                let job_id = msg.job_id();

                let result = match op {
                    OpCode::SubmitJob => {
                        let job: Job = match msg.parse_payload(ParseError::SubmitJob) {
                            Ok(j) => j,
                            Err(e) => {
                                log!("SubmitJob: {}", e)?;
                                continue;
                            }
                        };
                        crate::coordinator::run(
                            &endpoint, &mut msg_buf, shell, point, job_id, job,
                        )
                        .await
                    }
                    OpCode::AssignCollector => {
                        let p: AssignCollectorPayload =
                            match msg.parse_payload(ParseError::AssignCollector) {
                                Ok(p) => p,
                                Err(e) => {
                                    log!("{}", e)?;
                                    continue;
                                }
                            };
                        let mapper_tx = match endpoint.sender(p.mapper_addr()) {
                            Ok(s) => s,
                            Err(_) => {
                                log!("AssignCollector: sender allocation failed")?;
                                continue;
                            }
                        };
                        let stx = crate::transport::SpaceCompTx::new(
                            mapper_tx,
                            job_id,
                            p.partition_id(),
                        );
                        app.collect(stx).await
                    }
                    OpCode::AssignMapper => {
                        let p: AssignMapperPayload =
                            match msg.parse_payload(ParseError::AssignMapper) {
                                Ok(p) => p,
                                Err(e) => {
                                    log!("{}", e)?;
                                    continue;
                                }
                            };
                        let reducer_tx = match endpoint.sender(p.reducer_addr()) {
                            Ok(s) => s,
                            Err(_) => {
                                log!("AssignMapper: sender allocation failed")?;
                                continue;
                            }
                        };
                        let mut srx = crate::transport::SpaceCompRx::new(
                            &mut listener,
                            job_id,
                            p.collector_count(),
                        );
                        let mut stx =
                            crate::transport::SpaceCompTx::new(reducer_tx, job_id, 0);
                        let mut map_buf = [0u8; 8192];
                        let result = loop {
                            match srx.recv(&mut map_buf).await {
                                None => break Ok(()),
                                Some(Err(e)) => break Err(e),
                                Some(Ok(len)) => {
                                    if let Err(e) = app.map(&map_buf[..len], &mut stx).await {
                                        break Err(e);
                                    }
                                }
                            }
                        };
                        stx.done().await?;
                        result
                    }
                    OpCode::AssignReducer => {
                        let p: AssignReducerPayload =
                            match msg.parse_payload(ParseError::AssignReducer) {
                                Ok(p) => p,
                                Err(e) => {
                                    log!("{}", e)?;
                                    continue;
                                }
                            };
                        let los_tx = match endpoint.sender(p.los_addr()) {
                            Ok(s) => s,
                            Err(_) => {
                                log!("AssignReducer: sender allocation failed")?;
                                continue;
                            }
                        };
                        let srx = crate::transport::SpaceCompRx::new(
                            &mut listener,
                            job_id,
                            p.mapper_count(),
                        );
                        let stx = crate::transport::SpaceCompTx::new(los_tx, job_id, 0);
                        app.reduce(srx, stx).await
                    }
                    _ => continue,
                };

                if let Err(e) = result {
                    log!("SpaceCoMP: {}", e)?;
                }
            }
            Ok::<(), SpaceCompError>(())
        };

        let (d, dr) = join!(dispatch, endpoint.run(network, FixedRto::new(rto))).await;
        d?;
        dr.map_err(SpaceCompError::Transport)?;

        Ok(())
    }
}
