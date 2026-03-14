#![no_std]

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::error::Error as CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::Runtime;
use leodos_libcfs::{err, info};
use leodos_protocols::application::spacecomp::job::Job;
use leodos_protocols::application::spacecomp::packet::AssignCollectorPayload;
use leodos_protocols::application::spacecomp::packet::AssignMapperPayload;
use leodos_protocols::application::spacecomp::packet::AssignReducerPayload;
use leodos_protocols::application::spacecomp::packet::BuildError;
use leodos_protocols::application::spacecomp::packet::OpCode;
use leodos_protocols::application::spacecomp::packet::ParseError;
use leodos_protocols::application::spacecomp::packet::SpaceCompMessage;
use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::SpacecraftId;
use leodos_protocols::network::isl::routing;
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::isl::routing::algorithm::gateway::GatewayTable;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::utils::clock::MetClock;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::{Direction, Hop};
use leodos_protocols::network::isl::torus::Point;
use leodos_protocols::network::isl::torus::Torus;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::TransportError;
use leodos_protocols::transport::srspp::api::cfs::SrsppNode;
use leodos_protocols::transport::srspp::api::cfs::SrsppRxHandle;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

pub mod data;
mod roles;

pub type RouterError = routing::RouterError<CfsError>;

pub type RxHandle<'a> = SrsppRxHandle<'a, RouterError, ReceiverMachine<8, 4096, 8192>, 4>;
pub type TxHandle<'a> = SrsppTxHandle<'a, RouterError, 8, 4096, 512>;

pub struct Buffers {
    pub recv: [u8; 8192],
    pub msg: [u8; 512],
}

#[derive(Debug, thiserror::Error)]
pub enum SpaceCompError {
    #[error("failed to parse: {0}")]
    Parse(#[from] ParseError),
    #[error("failed to plan job: {0}")]
    Plan(&'static str),
    #[error("failed to build message: {0}")]
    Build(#[from] BuildError),
    #[error("transport: {0}")]
    Transport(#[from] TransportError<RouterError>),
}

const NUM_ORBITS: u8 = bindings::SPACECOMP_NUM_ORBITS as u8;
const NUM_SATS: u8 = bindings::SPACECOMP_NUM_SATS as u8;
const MAX_SATELLITES: usize = 64;
const ALTITUDE_M: f32 = 550_000.0;
const INCLINATION_DEG: f32 = 87.0;

const APID: u16 = bindings::SPACECOMP_APID as u16;
const PORT_BASE: u16 = 6000;
const RTO_MS: u32 = 1000;

pub const TORUS: Torus = Torus::new(NUM_ORBITS, NUM_SATS);
pub const SHELL: Shell = Shell::new(TORUS, ALTITUDE_M, INCLINATION_DEG);

const LOCALHOST: &str = "127.0.0.1";

const PORTS_PER_SAT: u16 = 10;

fn port_offset(hop: Hop) -> u16 {
    match hop {
        Hop::Isl(Direction::North) => 0,
        Hop::Isl(Direction::South) => 2,
        Hop::Isl(Direction::East) => 4,
        Hop::Isl(Direction::West) => 6,
        Hop::Ground => 8,
        Hop::Local => unreachable!(),
    }
}

fn send_port(point: Point, hop: Hop) -> u16 {
    PORT_BASE + point.sat as u16 * PORTS_PER_SAT + port_offset(hop)
}

fn recv_port(point: Point, hop: Hop) -> u16 {
    send_port(point, hop) + 1
}

fn orbit_ip(orbit: u8, out: &mut [u8; 16]) -> Result<usize, core::fmt::Error> {
    leodos_protocols::fmt!(out, "172.20.{orbit}.10")
}

fn local_link(local_port: u16, remote_port: u16) -> Result<UdpDatalink, CfsError> {
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn remote_link(local_port: u16, remote_orbit: u8, remote_port: u16) -> Result<UdpDatalink, CfsError> {
    let mut ip_buf = [0u8; 16];
    let len = orbit_ip(remote_orbit, &mut ip_buf).map_err(|_| CfsError::CfeStatusValidationFailure)?;
    let ip = core::str::from_utf8(&ip_buf[..len]).map_err(|_| CfsError::CfeStatusValidationFailure)?;
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(ip, remote_port)?;
    UdpDatalink::bind(local, remote)
}

#[no_mangle]
pub extern "C" fn SPACECOMP_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        info!("SpaceCoMP app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
        let Address::Satellite(point) = address else {
            err!("Invalid spacecraft ID")?;
            return Ok(());
        };

        let north_point = TORUS.neighbor(point, Direction::North);
        let south_point = TORUS.neighbor(point, Direction::South);
        let east_point = TORUS.neighbor(point, Direction::East);
        let west_point = TORUS.neighbor(point, Direction::West);

        let n = Hop::Isl(Direction::North);
        let s = Hop::Isl(Direction::South);
        let e = Hop::Isl(Direction::East);
        let w = Hop::Isl(Direction::West);
        let g = Hop::Ground;

        let north_link = local_link(
            send_port(point, n),
            recv_port(north_point, s),
        )?;
        let south_link = local_link(
            send_port(point, s),
            recv_port(south_point, n),
        )?;
        let east_link = remote_link(
            send_port(point, e),
            east_point.orb,
            recv_port(east_point, w),
        )?;
        let west_link = remote_link(
            send_port(point, w),
            west_point.orb,
            recv_port(west_point, e),
        )?;
        let ground_link = local_link(
            send_port(point, g),
            recv_port(point, g),
        )?;

        let mut gateway_table = GatewayTable::<4>::new(5.0);
        gateway_table.add_station(0, LatLon::new(67.86, 20.22));  // Kiruna
        gateway_table.add_station(1, LatLon::new(78.23, 15.39));  // Svalbard
        gateway_table.add_station(2, LatLon::new(64.86, -147.72)); // Fairbanks
        let algorithm = DistanceMinimizing::new(SHELL, gateway_table);
        let router = Router::builder()
            .north(north_link)
            .south(south_link)
            .east(east_link)
            .west(west_link)
            .ground(ground_link)
            .address(address)
            .algorithm(algorithm)
            .clock(MetClock::new())
            .build();

        let apid = Apid::new(APID).unwrap();

        let sender_config = SenderConfig::builder()
            .source_address(address)
            .apid(apid)
            .function_code(0)
            .rto_ticks(RTO_MS)
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

        let node = SrsppNode::new(sender_config, receiver_config);
        let (mut rx, mut tx, mut driver) = node.split(router, FixedRto::new(RTO_MS));

        let mut bufs = Buffers {
            recv: [0u8; 8192],
            msg: [0u8; 512],
        };

        let app_task = async move {
            loop {
                let Ok((source, len)) = rx.recv(&mut bufs.recv).await else {
                    break;
                };
                if let Err(e) = handle(&mut rx, &mut tx, &mut bufs, point, source, len).await {
                    err!("{}", e).ok();
                }
            }
        };

        let _ = join(app_task, driver.run()).await;

        Ok(())
    });
}

async fn handle(
    rx: &mut RxHandle<'_>,
    tx: &mut TxHandle<'_>,
    bufs: &mut Buffers,
    point: Point,
    source: Address,
    len: usize,
) -> Result<(), SpaceCompError> {
    let msg = SpaceCompMessage::parse(&bufs.recv[..len])?;
    let op_code = msg.op_code()?;
    let job_id = msg.job_id();

    match op_code {
        OpCode::SubmitJob => {
            let job: Job = msg.parse_payload(ParseError::SubmitJob)?;
            roles::coordinator::run(rx, tx, bufs, point, job_id, job, source).await?
        }
        OpCode::AssignCollector => {
            let p: AssignCollectorPayload = msg.parse_payload(ParseError::AssignCollector)?;
            roles::collector::run(tx, bufs, job_id, p).await?
        }
        OpCode::AssignMapper => {
            let p: AssignMapperPayload = msg.parse_payload(ParseError::AssignMapper)?;
            roles::mapper::run(rx, tx, bufs, job_id, p).await?
        }
        OpCode::AssignReducer => {
            let p: AssignReducerPayload = msg.parse_payload(ParseError::AssignReducer)?;
            roles::reducer::run(rx, tx, bufs, job_id, p).await?
        }
        _ => {}
    }

    Ok(())
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
