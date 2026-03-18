#![no_std]

use futures::FutureExt as _;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::Runtime;
use leodos_libcfs::{err, info};

use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::algorithm::gateway::GatewayTable;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::{Direction, Point, Torus};
use leodos_protocols::network::{NetworkRead, NetworkWrite};
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

const SB_HEADER_SIZE: usize = 8;

const MAX_ROUTES: usize = bindings::ROUTER_MAX_ROUTES as usize;

struct Route {
    apid: u16,
    topic: MsgId,
}

fn build_routing_table() -> (heapless::Vec<Route, MAX_ROUTES>, usize) {
    let mut table = heapless::Vec::new();
    macro_rules! add_route {
        ($apid:expr, $topic:expr) => {
            let _ = table.push(Route {
                apid: $apid as u16,
                topic: MsgId::from_local_tlm($topic as u16),
            });
        };
    }
    add_route!(
        bindings::ROUTER_ROUTE_0_APID,
        bindings::ROUTER_ROUTE_0_TOPIC
    );
    add_route!(
        bindings::ROUTER_ROUTE_1_APID,
        bindings::ROUTER_ROUTE_1_TOPIC
    );
    add_route!(
        bindings::ROUTER_ROUTE_2_APID,
        bindings::ROUTER_ROUTE_2_TOPIC
    );
    let len = table.len();
    (table, len)
}

fn lookup_topic(table: &[Route], apid: u16) -> Option<MsgId> {
    table.iter().find(|r| r.apid == apid).map(|r| r.topic)
}

fn publish_to_sb(mid: MsgId, data: &[u8]) -> Result<(), CfsError> {
    let total_size = SB_HEADER_SIZE + data.len();
    let mut buf = SendBuffer::new(total_size)?;
    {
        let mut msg = buf.view();
        msg.init(mid, total_size)?;
        let slice = buf.as_mut_slice();
        slice[SB_HEADER_SIZE..].copy_from_slice(data);
    }
    buf.send(true)?;
    Ok(())
}

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
        .ok_or_else(|| CfsError::ValidationFailure)
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

        let mut router: Router<_, _, _, _, MTU, 2048> = Router::builder()
            .north(isl_link(point, Direction::North)?)
            .south(isl_link(point, Direction::South)?)
            .east(isl_link(point, Direction::East)?)
            .west(isl_link(point, Direction::West)?)
            .ground(ground_link(point)?)
            .address(address)
            .algorithm(DistanceMinimizing::new(SHELL, gateway_table))
            .clock(MetClock::new())
            .build();

        let (routes, route_count) = build_routing_table();
        info!("Loaded {route_count} APID routes")?;

        let send_mid = MsgId::from_local_cmd(bindings::ROUTER_SEND_TOPICID as u16);

        let mut pipe = Pipe::new("ROUTER_SB", 32)?;
        pipe.subscribe(send_mid)?;

        info!("Router ready, bridging SB and ISL")?;

        let mut from_net = [0u8; MTU];
        let mut from_sb = [0u8; MTU + SB_HEADER_SIZE];

        enum Event {
            FromNetwork(usize),
            FromSb(usize),
            Err,
        }

        loop {
            let event = {
                let net_read = router.read(&mut from_net).fuse();
                let sb_read = pipe.recv(&mut from_sb).fuse();
                pin_utils::pin_mut!(net_read, sb_read);

                futures::select_biased! {
                    r = net_read => match r {
                        Ok(len) => Event::FromNetwork(len),
                        Err(_) => Event::Err,
                    },
                    r = sb_read => match r {
                        Ok(len) => Event::FromSb(len),
                        Err(_) => Event::Err,
                    },
                }
            };

            match event {
                Event::FromNetwork(len) => {
                    if len < 2 {
                        continue;
                    }
                    let data = &from_net[..len];
                    let apid = u16::from_be_bytes([data[0], data[1]]) & 0x07FF;
                    if let Some(mid) = lookup_topic(&routes, apid) {
                        let _ = publish_to_sb(mid, data);
                    }
                }
                Event::FromSb(len) => {
                    if len <= SB_HEADER_SIZE {
                        continue;
                    }
                    let payload = &from_sb[SB_HEADER_SIZE..len];
                    let _ = router.write(payload).await;
                }
                Event::Err => {}
            }
        }

        #[allow(unreachable_code)]
        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
