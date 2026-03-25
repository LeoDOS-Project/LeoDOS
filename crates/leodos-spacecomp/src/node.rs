//! [`SpaceCompNode`] — the main entry point for running
//! a SpaceCoMP computation on a cFS satellite.

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::err;
use leodos_libcfs::info;
use leodos_libcfs::join;

use leodos_protocols::application::spacecomp::job::Job;
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
use crate::SpaceCompJob;

type RxHandle<'a> = SrsppRxHandle<'a, leodos_libcfs::error::CfsError, ReceiverMachine<8, 4096, 8192>, 1>;
type TxHandle<'a> = SrsppTxHandle<'a, leodos_libcfs::error::CfsError, NoStore, AlwaysReachable, 8, 4096, 512>;

/// A SpaceCoMP node that participates in distributed
/// computation across the constellation.
pub struct SpaceCompNode<J> {
    job: J,
    config: SpaceCompConfig,
}

#[bon::bon]
impl<J: SpaceCompJob> SpaceCompNode<J> {
    #[builder]
    pub fn new(job: J, config: SpaceCompConfig) -> Self {
        Self { job, config }
    }
}

impl<J: SpaceCompJob> SpaceCompNode<J> {
    /// Runs the SpaceCoMP node.
    pub async fn run(&mut self) -> Result<(), SpaceCompError> {
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

        let srspp: SrsppNode<leodos_libcfs::error::CfsError> =
            SrsppNode::new(sender_config, receiver_config);
        let (mut rx, mut tx, mut driver) = srspp.split(network, FixedRto::new(rto));

        let shell = self.config.shell();

        let mut recv_buf = [0u8; 8192];
        let mut msg_buf = [0u8; 512];

        let dispatch = async {
            loop {
                let Ok((_source, len)) = rx.recv(&mut recv_buf).await else {
                    break;
                };
                let msg = match SpaceCompMessage::parse(&recv_buf[..len]) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let Ok(op) = msg.op_code() else { continue };
                let job_id = msg.job_id();

                match op {
                    OpCode::SubmitJob => {
                        let job: Job = match msg.parse_payload(ParseError::SubmitJob) {
                            Ok(j) => j,
                            Err(e) => {
                                err!("SubmitJob parse: {}", e)?;
                                continue;
                            }
                        };
                        if let Err(e) =
                            crate::coordinator::run(&mut tx, &mut msg_buf, shell, point, job_id, job).await
                        {
                            err!("Coordinator: {}", e)?;
                        }
                    }
                    // TODO: AssignCollector, AssignMapper, AssignReducer
                    // These will wire the user's SpaceCompJob trait impls
                    // to SrsppSource/SrsppSink via the runners.
                    _ => {}
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
