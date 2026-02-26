#![no_std]

use core::mem::size_of;

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::error::Error;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::datalink::link::cfs::UdpDataLink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::SpacecraftId;
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::local::LocalChannel;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::torus::Direction;
use leodos_protocols::network::isl::torus::Point;
use leodos_protocols::network::isl::torus::Torus;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppNode;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
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

#[macro_use]
mod fmt;
pub mod data;
mod isl;
mod roles;

pub const NUM_ORBITS: u8 = bindings::SPACECOMP_NUM_ORBITS as u8;
pub const NUM_SATS: u8 = bindings::SPACECOMP_NUM_SATS as u8;
pub const INCLINATION_RAD: f32 = 87.0 * (core::f32::consts::PI / 180.0);

const APID: u16 = bindings::SPACECOMP_APID as u16;
const PORT_BASE: u16 = 6000;
const RTO_MS: u32 = 1000;

const LOCALHOST: &str = "127.0.0.1";

const PORTS_PER_SAT: u16 = 10;

fn port_offset(direction: Direction) -> u16 {
    match direction {
        Direction::North => 0,
        Direction::South => 2,
        Direction::East => 4,
        Direction::West => 6,
        Direction::Ground => 8,
        Direction::Local => unreachable!(),
    }
}

fn send_port(point: Point, direction: Direction) -> u16 {
    PORT_BASE + point.sat as u16 * PORTS_PER_SAT + port_offset(direction)
}

fn recv_port(point: Point, direction: Direction) -> u16 {
    send_port(point, direction) + 1
}

fn orbit_ip(orbit: u8, out: &mut [u8; 16]) -> Result<usize, core::fmt::Error> {
    fmt!(out, "172.20.{orbit}.10")
}

fn local_link(local_port: u16, remote_port: u16) -> Result<UdpDataLink, Error> {
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDataLink::bind(local, remote)
}

fn remote_link(local_port: u16, remote_orbit: u8, remote_port: u16) -> Result<UdpDataLink, Error> {
    let mut ip_buf = [0u8; 16];
    let len = orbit_ip(remote_orbit, &mut ip_buf).map_err(|_| Error::CfeStatusValidationFailure)?;
    let ip = core::str::from_utf8(&ip_buf[..len]).map_err(|_| Error::CfeStatusValidationFailure)?;
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(ip, remote_port)?;
    UdpDataLink::bind(local, remote)
}

#[no_mangle]
pub extern "C" fn SPACECOMP_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        event::info(0, "SpaceCoMP app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
        let Address::Satellite(point) = address else {
            event::info(0, "Invalid spacecraft ID")?;
            return Ok(());
        };

        let torus = Torus::new(NUM_ORBITS, NUM_SATS);
        let north_point = torus.neighbor(point, Direction::North);
        let south_point = torus.neighbor(point, Direction::South);
        let east_point = torus.neighbor(point, Direction::East);
        let west_point = torus.neighbor(point, Direction::West);

        let north_link = local_link(
            send_port(point, Direction::North),
            recv_port(north_point, Direction::South),
        )?;
        let south_link = local_link(
            send_port(point, Direction::South),
            recv_port(south_point, Direction::North),
        )?;
        let east_link = remote_link(
            send_port(point, Direction::East),
            east_point.orb,
            recv_port(east_point, Direction::West),
        )?;
        let west_link = remote_link(
            send_port(point, Direction::West),
            west_point.orb,
            recv_port(west_point, Direction::East),
        )?;
        let ground_link = local_link(
            send_port(point, Direction::Ground),
            recv_port(point, Direction::Ground),
        )?;

        let local_channel: LocalChannel<8, 1024> = LocalChannel::new();
        let (app_link, router_link) = local_channel.split();
        let algorithm = DistanceMinimizing::new(INCLINATION_RAD);
        let mut router = Router::builder()
            .north(north_link)
            .south(south_link)
            .east(east_link)
            .west(west_link)
            .ground(ground_link)
            .local(router_link)
            .address(address)
            .torus(torus)
            .algorithm(algorithm)
            .build();

        let apid = Apid::new(APID).unwrap();

        let sender_config = SenderConfig::builder()
            .source_address(address)
            .apid(apid)
            .function_code(0)
            .message_id(0)
            .action_code(0)
            .rto_ticks(RTO_MS)
            .max_retransmits(3)
            .header_overhead(SrsppDataPacket::HEADER_SIZE)
            .build();

        let receiver_config = ReceiverConfig::builder()
            .local_address(address)
            .apid(apid)
            .function_code(0)
            .message_id(0)
            .action_code(0)
            .immediate_ack(true)
            .ack_delay_ticks(100)
            .build();

        let node = SrsppNode::new(sender_config, receiver_config);
        let (mut handle, mut driver) = node.split(app_link, FixedRto::new(RTO_MS));

        let app_task = async move {
            use leodos_protocols::mission::compute::packet::{
                AssignCollectorPayload, AssignMapperPayload, AssignReducerPayload, OpCode,
            };
            use zerocopy::FromBytes;

            let mut buf = [0u8; 512];

            loop {
                let Ok(msg) = handle.recv().await else { break };
                let Some(cmd) = isl::parse(&msg.data, &mut buf) else {
                    continue;
                };

                match cmd.op_code {
                    OpCode::SubmitJob => {
                        roles::coordinator::run(&mut handle, point, cmd.job_id).await;
                    }
                    OpCode::AssignCollector => {
                        if let Some(p) = buf
                            .get(..size_of::<AssignCollectorPayload>())
                            .and_then(|b| AssignCollectorPayload::read_from_bytes(b).ok())
                        {
                            roles::collector::run(
                                &mut handle,
                                p.mapper_addr.parse(),
                                p.partition_id,
                                cmd.job_id,
                            )
                            .await;
                        }
                    }
                    OpCode::AssignMapper => {
                        if let Some(p) = buf
                            .get(..size_of::<AssignMapperPayload>())
                            .and_then(|b| AssignMapperPayload::read_from_bytes(b).ok())
                        {
                            roles::mapper::run(
                                &mut handle,
                                p.reducer_addr.parse(),
                                cmd.job_id,
                                p.collector_count,
                            )
                            .await;
                        }
                    }
                    OpCode::AssignReducer => {
                        if let Some(p) = buf
                            .get(..size_of::<AssignReducerPayload>())
                            .and_then(|b| AssignReducerPayload::read_from_bytes(b).ok())
                        {
                            roles::reducer::run(
                                &mut handle,
                                p.los_addr.parse(),
                                cmd.job_id,
                                p.mapper_count,
                            )
                            .await;
                        }
                    }
                    _ => {}
                }
            }
        };

        let _ = join(router.run(), join(app_task, driver.run())).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
