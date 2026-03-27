//! [`SpaceCompNode`] — the main entry point for running
//! a SpaceCoMP computation on a cFS satellite.
#![allow(async_fn_in_trait)]

use core::time::Duration;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::log;
use leodos_libcfs::join;

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
use leodos_protocols::transport::srspp::api::cfs::SrsppNode;
use leodos_protocols::transport::srspp::api::cfs::SrsppRxHandle;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
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

/// SRSPP receive handle.
pub type RxHandle<
    'a,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const RX_BUF: usize = 8192,
    const MAX_STREAMS: usize = 1,
> = SrsppRxHandle<'a, CfsError, ReceiverMachine<WIN, BUF, RX_BUF>, MAX_STREAMS>;

/// SRSPP transmit handle.
pub type TxHandle<
    'a,
    S = NoStore,
    Re = AlwaysReachable,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
> = SrsppTxHandle<'a, CfsError, S, Re, WIN, BUF, MTU>;

/// A SpaceCoMP node that handles SRSPP transport,
/// message dispatch, and coordinator orchestration.
///
/// Type parameters:
/// - `F`: closure that constructs the app after startup sync
/// - `S`: message store for DTN (default: [`NoStore`])
/// - `R`: reachability oracle for DTN (default: [`AlwaysReachable`])
///
/// Const parameters control SRSPP buffer sizes (all have defaults).
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
    /// Collects local data and sends it to the assigned mapper.
    async fn collect(&mut self, tx: impl Tx) -> Result<(), SpaceCompError>;

    /// Processes one message from a collector. Called once per received chunk.
    async fn map(&mut self, data: &[u8], tx: impl Tx) -> Result<(), SpaceCompError>;

    /// Aggregates results from mappers and sends the final output.
    async fn reduce(&mut self, rx: impl Rx, tx: impl Tx) -> Result<(), SpaceCompError>;
}

impl<F, S: MessageStore, R: Reachable> SpaceCompNode<F, S, R> {
    /// Starts the node with default SRSPP buffer sizes.
    pub fn start<A: SpaceComp>(self) -> !
    where
        F: FnOnce() -> Result<A, SpaceCompError>,
    {
        leodos_libcfs::runtime::Runtime::new().run(self.run::<A, 8, 4096, 512, 8192, 1>())
    }

    /// Runs the node with custom SRSPP buffer sizes.
    pub async fn run<
        A: SpaceComp,
        const WIN: usize,
        const BUF: usize,
        const MTU: usize,
        const RX_BUF: usize,
        const MAX_STREAMS: usize,
    >(
        self,
    ) -> Result<(), SpaceCompError>
    where
        F: FnOnce() -> Result<A, SpaceCompError>,
    {
        event::register(&[])?;
        log!("SpaceCoMP node starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(self.config.num_sats);
        let Address::Satellite(point) = address else {
            log!("Invalid spacecraft ID")?;
            return Ok(());
        };

        let recv_mid = MsgId::local_tlm(self.config.apid.value());
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

        let srspp: SrsppNode<
            CfsError,
            S,
            R,
            ReceiverMachine<WIN, BUF, RX_BUF>,
            WIN,
            BUF,
            MTU,
            MAX_STREAMS,
        > = SrsppNode::new(sender_config, receiver_config, self.store, self.reachable);
        let (mut rx, mut tx, mut driver) = srspp.split(network, FixedRto::new(rto));

        system::wait_for_startup_sync(Duration::from_millis(10_000));

        let mut app = (self.app_fn)()?;
        let shell = self.config.shell();
        let mut recv_buf = [0u8; SpaceCompMessage::MAX_DISPATCH_SIZE];
        let mut msg_buf = [0u8; SpaceCompMessage::MAX_ASSIGN_SIZE];

        let dispatch = async {
            loop {
                let Ok((_source, len)) = rx.recv(&mut recv_buf).await else {
                    break;
                };
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
                        crate::coordinator::run(&mut tx, &mut msg_buf, shell, point, job_id, job)
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
                        let stx = crate::transport::SpaceCompTx::new(
                            tx,
                            p.mapper_addr(),
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
                        let mut srx = crate::transport::SpaceCompRx::new(
                            &mut rx,
                            job_id,
                            p.collector_count(),
                        );
                        let mut stx =
                            crate::transport::SpaceCompTx::new(tx, p.reducer_addr(), job_id, 0);
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
                        let srx =
                            crate::transport::SpaceCompRx::new(&mut rx, job_id, p.mapper_count());
                        let stx = crate::transport::SpaceCompTx::new(tx, p.los_addr(), job_id, 0);
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

        let (d, dr) = join!(dispatch, driver.run()).await;
        d?;
        dr.map_err(SpaceCompError::Transport)?;

        Ok(())
    }
}
