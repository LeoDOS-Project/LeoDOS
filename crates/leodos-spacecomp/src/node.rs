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
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

use crate::SpaceCompConfig;
use crate::SpaceCompError;

/// SRSPP handle types used by role functions.
pub type RxHandle<'a> = SrsppRxHandle<'a, CfsError, ReceiverMachine<8, 4096, 8192>, 1>;
pub type TxHandle<'a> = SrsppTxHandle<'a, CfsError, NoStore, AlwaysReachable, 8, 4096, 512>;

/// Shared buffers passed to role functions.
pub struct Buffers {
    pub recv: [u8; 8192],
    pub msg: [u8; 512],
}

/// A SpaceCoMP node that handles SRSPP transport,
/// message dispatch, and coordinator orchestration.
pub struct SpaceCompNode {
    config: SpaceCompConfig,
}

#[bon::bon]
impl SpaceCompNode {
    #[builder]
    pub fn new(config: SpaceCompConfig) -> Self {
        Self { config }
    }
}

/// App-defined computation for each SpaceCoMP role.
///
/// Implement this to define what happens when this node
/// is assigned as a collector, mapper, or reducer.
pub trait SpaceComp {
    /// Collects local data and sends it to the assigned mapper.
    async fn collect(
        &self,
        tx: &mut TxHandle<'_>,
        job_id: u16,
        assign: AssignCollectorPayload,
    ) -> Result<(), SpaceCompError>;

    /// Processes data from collectors and sends results to the reducer.
    async fn map(
        &self,
        rx: &mut RxHandle<'_>,
        tx: &mut TxHandle<'_>,
        job_id: u16,
        assign: AssignMapperPayload,
    ) -> Result<(), SpaceCompError>;

    /// Aggregates results from mappers and sends the final output.
    async fn reduce(
        &self,
        rx: &mut RxHandle<'_>,
        tx: &mut TxHandle<'_>,
        job_id: u16,
        assign: AssignReducerPayload,
    ) -> Result<(), SpaceCompError>;
}

impl SpaceCompNode {
    /// Runs the node with the given app logic.
    pub async fn run(&self, app: &impl SpaceComp) -> Result<(), SpaceCompError> {
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

        let srspp: SrsppNode<CfsError> = SrsppNode::new(sender_config, receiver_config);
        let (mut rx, mut tx, mut driver) = srspp.split(network, FixedRto::new(rto));

        let shell = self.config.shell();
        let mut bufs = Buffers {
            recv: [0u8; 8192],
            msg: [0u8; 512],
        };

        let dispatch = async {
            loop {
                let Ok((_source, len)) = rx.recv(&mut bufs.recv).await else {
                    break;
                };
                let Ok(msg) = SpaceCompMessage::parse(&bufs.recv[..len]) else {
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
                        crate::coordinator::run(&mut tx, &mut bufs.msg, shell, point, job_id, job)
                            .await
                    }
                    OpCode::AssignCollector => {
                        let p = match msg.parse_payload(ParseError::AssignCollector) {
                            Ok(p) => p,
                            Err(e) => {
                                err!("{}", e)?;
                                continue;
                            }
                        };
                        app.collect(&mut tx, job_id, p).await
                    }
                    OpCode::AssignMapper => {
                        let p = match msg.parse_payload(ParseError::AssignMapper) {
                            Ok(p) => p,
                            Err(e) => {
                                err!("{}", e)?;
                                continue;
                            }
                        };
                        app.map(&mut rx, &mut tx, job_id, p).await
                    }
                    OpCode::AssignReducer => {
                        let p = match msg.parse_payload(ParseError::AssignReducer) {
                            Ok(p) => p,
                            Err(e) => {
                                err!("{}", e)?;
                                continue;
                            }
                        };
                        app.reduce(&mut rx, &mut tx, job_id, p).await
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
