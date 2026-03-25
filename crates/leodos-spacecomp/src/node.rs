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
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::ParseError;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::SpacecraftId;
use leodos_protocols::network::isl::torus::Point;
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

/// Shared buffers passed to the dispatch handler.
pub struct Buffers {
    pub recv: [u8; 8192],
    pub msg: [u8; 512],
}

/// Dispatch context passed to the app's handler for each
/// received message. The app matches on `op` and runs the
/// appropriate role logic.
pub struct Dispatch<'a> {
    pub rx: &'a mut RxHandle<'a>,
    pub tx: &'a mut TxHandle<'a>,
    pub bufs: &'a mut Buffers,
    pub point: Point,
    pub source: Address,
    pub op: OpCode,
    pub job_id: u16,
    pub msg: &'a SpaceCompMessage,
}

/// A SpaceCoMP node that handles SRSPP transport,
/// message dispatch, and coordinator orchestration.
///
/// The library sets up the transport and dispatch loop.
/// The coordinator (SubmitJob) is handled automatically.
/// For collector/mapper/reducer assignments, call the
/// provided `dispatch` callback.
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

impl SpaceCompNode {
    /// Sets up SRSPP and runs the dispatch loop.
    ///
    /// Returns the rx/tx handles, buffers, and node point
    /// so the caller can run their own dispatch logic.
    /// The coordinator is handled automatically for
    /// SubmitJob messages.
    pub async fn setup(
        &self,
    ) -> Result<
        (
            RxHandle<'_>,
            TxHandle<'_>,
            Buffers,
            Point,
        ),
        SpaceCompError,
    > {
        // This approach won't work — the SRSPP node and
        // network need to live longer than the returned
        // handles. Let's use a different pattern.
        todo!()
    }

    /// Runs the SpaceCoMP node. The caller provides a
    /// `handler` async function that processes each
    /// non-coordinator message (collector/mapper/reducer
    /// assignments).
    ///
    /// The handler receives rx, tx, bufs, the local point,
    /// source address, opcode, job_id, and the parsed
    /// message. It should match on the opcode and run the
    /// corresponding role logic.
    ///
    /// # Example
    ///
    /// ```ignore
    /// node.run(|rx, tx, bufs, point, source, op, job_id, msg| async move {
    ///     match op {
    ///         OpCode::AssignCollector => { ... }
    ///         OpCode::AssignMapper => { ... }
    ///         OpCode::AssignReducer => { ... }
    ///         _ => {}
    ///     }
    ///     Ok(())
    /// }).await?;
    /// ```
    pub async fn run_with<H, HF>(&self, mut handler: H) -> Result<(), SpaceCompError>
    where
        H: FnMut(
            &mut RxHandle<'_>,
            &mut TxHandle<'_>,
            &mut Buffers,
            Point,
            Address,
            usize,
        ) -> HF,
        HF: core::future::Future<Output = Result<(), SpaceCompError>>,
    {
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
                let Ok((source, len)) = rx.recv(&mut bufs.recv).await else {
                    break;
                };
                let msg = match SpaceCompMessage::parse(&bufs.recv[..len]) {
                    Ok(m) => m,
                    Err(_) => continue,
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
                    _ => handler(&mut rx, &mut tx, &mut bufs, point, source, len).await,
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
