#![no_std]

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::error::Error as CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::Runtime;
use leodos_libcfs::{err, info};

use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::bridge::Bridge;
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::algorithm::gateway::GatewayTable;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::{Direction, Point, Torus};
use leodos_protocols::utils::clock::MetClock;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const NUM_ORBS: u8 = bindings::ROUTER_NUM_ORBS as u8;
const NUM_SATS: u8 = bindings::ROUTER_NUM_SATS as u8;
const ALTITUDE_M: f32 = 550_000.0;
const INCLINATION_DEG: f32 = 87.0;

const TORUS: Torus = Torus::new(NUM_ORBS, NUM_SATS);
const SHELL: Shell = Shell::new(TORUS, ALTITUDE_M, INCLINATION_DEG);

const LOCALHOST: &str = "127.0.0.1";
const PORT_BASE: u16 = 6000;
const PORTS_PER_SAT: u16 = 10;
const MTU: usize = 1024;

fn isl_port_offset(dir: Direction) -> u16 {
    match dir {
        Direction::North => 0,
        Direction::South => 2,
        Direction::East => 4,
        Direction::West => 6,
    }
}

/// Returns (send_port, recv_port) for an ISL direction.
fn isl_ports(point: Point, dir: Direction) -> (u16, u16) {
    let base = PORT_BASE + point.sat as u16 * PORTS_PER_SAT + isl_port_offset(dir);
    (base, base + 1)
}

const GROUND_OFFSET: u16 = 8;

/// Returns (send_port, recv_port) for the ground link.
fn ground_ports(point: Point) -> (u16, u16) {
    let base = PORT_BASE + point.sat as u16 * PORTS_PER_SAT + GROUND_OFFSET;
    (base, base + 1)
}

fn orb_ip(orb: u8, out: &mut [u8; 16]) -> Result<&str, CfsError> {
    leodos_protocols::fmt!(out, "172.20.{orb}.10")
        .ok()
        .and_then(|len| core::str::from_utf8(&out[..len]).ok())
        .ok_or_else(|| CfsError::CfeStatusValidationFailure)
}

fn local_link(local_port: u16, remote_port: u16) -> Result<UdpDatalink, CfsError> {
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn remote_link(local_port: u16, remote_orb: u8, remote_port: u16) -> Result<UdpDatalink, CfsError> {
    let mut buf = [0u8; 16];
    let ip = orb_ip(remote_orb, &mut buf)?;
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(ip, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn isl_link(point: Point, dir: Direction) -> Result<UdpDatalink, CfsError> {
    let neighbor = TORUS.neighbor(point, dir);
    let (send, _) = isl_ports(point, dir);
    let (_, recv) = isl_ports(neighbor, dir.opposite());
    if point.orb == neighbor.orb {
        local_link(send, recv)
    } else {
        remote_link(send, neighbor.orb, recv)
    }
}

fn ground_link(point: Point) -> Result<UdpDatalink, CfsError> {
    let (send, recv) = ground_ports(point);
    local_link(send, recv)
}

#[no_mangle]
pub extern "C" fn ROUTER_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        info!("Router app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
        let Address::Satellite(point) = address else {
            err!("Invalid spacecraft ID")?;
            return Ok(());
        };

        let mut gateway_table = GatewayTable::<4>::new(5.0);
        gateway_table.add_station(0, LatLon::new(67.86, 20.22));
        gateway_table.add_station(1, LatLon::new(78.23, 15.39));
        gateway_table.add_station(2, LatLon::new(64.86, -147.72));

        let router: Router<_, _, _, _, MTU, 2048> = Router::builder()
            .north(isl_link(point, Direction::North)?)
            .south(isl_link(point, Direction::South)?)
            .east(isl_link(point, Direction::East)?)
            .west(isl_link(point, Direction::West)?)
            .ground(ground_link(point)?)
            .address(address)
            .algorithm(DistanceMinimizing::new(SHELL, gateway_table))
            .clock(MetClock::new())
            .build();

        let send_mid = MsgId::from_local_cmd(bindings::ROUTER_SEND_TOPICID as u16);
        let recv_mid = MsgId::from_local_tlm(bindings::ROUTER_RECV_TOPICID as u16);

        let sb = SbDatalink::new("ROUTER_SB", 32, send_mid, recv_mid)?;

        info!("Router ready, bridging SB and ISL")?;

        let mut bridge = Bridge::<_, _, MTU>::new(router, sb);
        bridge.run().await;

        #[allow(unreachable_code)]
        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
