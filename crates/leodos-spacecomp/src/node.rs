//! [`SpaceCompNode`] — the main entry point for running
//! a SpaceCoMP computation on a cFS satellite.

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::err;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::info;
use leodos_libcfs::join;

use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::packet::AssignCollectorPayload;
use leodos_protocols::application::spacecomp::packet::AssignMapperPayload;
use leodos_protocols::application::spacecomp::packet::AssignReducerPayload;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::ParseError;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
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
    S: MessageStore = NoStore,
    Re: Reachable = AlwaysReachable,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
> = SrsppTxHandle<'a, CfsError, S, Re, WIN, BUF, MTU>;

/// A SpaceCoMP node that handles SRSPP transport,
/// message dispatch, and coordinator orchestration.
///
/// Type parameters:
/// - `S`: message store for DTN (default: [`NoStore`])
/// - `R`: reachability oracle for DTN (default: [`AlwaysReachable`])
///
/// Const parameters control SRSPP buffer sizes (all have defaults).
pub struct SpaceCompNode<
    S: MessageStore = NoStore,
    R: Reachable = AlwaysReachable,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
    const RX_BUF: usize = 8192,
    const MAX_STREAMS: usize = 1,
> {
    config: SpaceCompConfig,
    store: S,
    reachable: R,
}

#[bon::bon]
impl<S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize, const RX_BUF: usize, const MAX_STREAMS: usize>
    SpaceCompNode<S, R, WIN, BUF, MTU, RX_BUF, MAX_STREAMS>
{
    #[builder]
    pub fn new(config: SpaceCompConfig, store: S, reachable: R) -> Self {
        Self {
            config,
            store,
            reachable,
        }
    }
}

/// App-defined computation for each SpaceCoMP role.
///
/// Implement this to define what happens when this node
/// is assigned as a collector, mapper, or reducer.
pub trait SpaceComp<
    S: MessageStore = NoStore,
    Re: Reachable = AlwaysReachable,
    const WIN: usize = 8,
    const BUF: usize = 4096,
    const MTU: usize = 512,
    const RX_BUF: usize = 8192,
    const MAX_STREAMS: usize = 1,
> {
    /// Collects local data and sends it to the assigned mapper.
    async fn collect(
        &self,
        tx: &mut TxHandle<'_, S, Re, WIN, BUF, MTU>,
        job_id: u16,
        mapper_addr: Address,
        partition_id: u8,
    ) -> Result<(), SpaceCompError>;

    /// Processes data from collectors and sends results to the reducer.
    async fn map(
        &self,
        rx: &mut RxHandle<'_, WIN, BUF, RX_BUF, MAX_STREAMS>,
        tx: &mut TxHandle<'_, S, Re, WIN, BUF, MTU>,
        job_id: u16,
        reducer_addr: Address,
        collector_count: u8,
    ) -> Result<(), SpaceCompError>;

    /// Aggregates results from mappers and sends the final output.
    async fn reduce(
        &self,
        rx: &mut RxHandle<'_, WIN, BUF, RX_BUF, MAX_STREAMS>,
        tx: &mut TxHandle<'_, S, Re, WIN, BUF, MTU>,
        job_id: u16,
        los_addr: Address,
        mapper_count: u8,
    ) -> Result<(), SpaceCompError>;
}

impl<
        S: MessageStore,
        R: Reachable,
        const WIN: usize,
        const BUF: usize,
        const MTU: usize,
        const RX_BUF: usize,
        const MAX_STREAMS: usize,
    > SpaceCompNode<S, R, WIN, BUF, MTU, RX_BUF, MAX_STREAMS>
{
    /// Runs the node with the given app logic.
    pub async fn run(self, app: &impl SpaceComp<S, R, WIN, BUF, MTU, RX_BUF, MAX_STREAMS>) -> Result<(), SpaceCompError> {
        event::register(&[])?;
        info!("SpaceCoMP node starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(self.config.num_sats);
        let Address::Satellite(point) = address else {
            err!("Invalid spacecraft ID")?;
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

        let srspp: SrsppNode<CfsError, S, R, ReceiverMachine<WIN, BUF, RX_BUF>, WIN, BUF, MTU, MAX_STREAMS> =
            SrsppNode::new(sender_config, receiver_config, self.store, self.reachable);
        let (mut rx, mut tx, mut driver) = srspp.split(network, FixedRto::new(rto));

        let shell = self.config.shell();
        // Max dispatch message: SpaceComp header (4) + Job payload (41)
        const MAX_DISPATCH_MSG: usize = SpaceCompMessage::HEADER_SIZE + core::mem::size_of::<Job>();
        // Max coordinator send: SpaceComp header (4) + assignment payload (5)
        const MAX_ASSIGN_MSG: usize = SpaceCompMessage::HEADER_SIZE + 5;

        let mut recv_buf = [0u8; MAX_DISPATCH_MSG];
        let mut msg_buf = [0u8; MAX_ASSIGN_MSG];

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
                                err!("SubmitJob: {}", e)?;
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
                                    err!("{}", e)?;
                                    continue;
                                }
                            };
                        app.collect(&mut tx, job_id, p.mapper_addr(), p.partition_id())
                            .await
                    }
                    OpCode::AssignMapper => {
                        let p: AssignMapperPayload =
                            match msg.parse_payload(ParseError::AssignMapper) {
                                Ok(p) => p,
                                Err(e) => {
                                    err!("{}", e)?;
                                    continue;
                                }
                            };
                        app.map(
                            &mut rx,
                            &mut tx,
                            job_id,
                            p.reducer_addr(),
                            p.collector_count(),
                        )
                        .await
                    }
                    OpCode::AssignReducer => {
                        let p: AssignReducerPayload =
                            match msg.parse_payload(ParseError::AssignReducer) {
                                Ok(p) => p,
                                Err(e) => {
                                    err!("{}", e)?;
                                    continue;
                                }
                            };
                        app.reduce(&mut rx, &mut tx, job_id, p.los_addr(), p.mapper_count())
                            .await
                    }
                    _ => continue,
                };

                if let Err(e) = result {
                    err!("SpaceCoMP: {}", e)?;
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
