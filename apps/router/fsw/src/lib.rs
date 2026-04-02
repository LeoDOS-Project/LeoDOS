#![no_std]

use core::time::Duration;
use futures::FutureExt as _;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::log;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::Runtime;

use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::geo::LatLon;
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::algorithm::gateway::GatewayTable;
use leodos_protocols::network::isl::routing::packet::IslRoutingTelecommand;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::shell::Shell;
use leodos_protocols::network::isl::torus::Direction;
use leodos_protocols::network::isl::torus::Point;
use leodos_protocols::network::isl::torus::Torus;
use leodos_protocols::network::NetworkRead;
use leodos_protocols::network::NetworkWrite;
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
                topic: MsgId::local_tlm($topic as u16),
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

/// Unique port base for a satellite, accounting for both orbit and sat index.
fn sat_port_base(point: Point) -> u16 {
    PORT_BASE + (point.orb as u16 * NUM_SATS as u16 + point.sat as u16) * PORTS_PER_SAT
}

/// Returns (send_port, recv_port) for an ISL direction.
fn isl_ports(point: Point, dir: Direction) -> (u16, u16) {
    let base = sat_port_base(point) + isl_port_offset(dir);
    (base, base + 1)
}

const GROUND_OFFSET: u16 = 8;

/// Returns (send_port, recv_port) for the ground link.
fn ground_ports(point: Point) -> (u16, u16) {
    let base = sat_port_base(point) + GROUND_OFFSET;
    (base, base + 1)
}

fn udp_link(local_port: u16, remote_port: u16) -> Result<UdpDatalink, CfsError> {
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn isl_link(point: Point, dir: Direction) -> Result<UdpDatalink, CfsError> {
    let neighbor = TORUS.neighbor(point, dir);
    let (send, _) = isl_ports(point, dir);
    let (_, recv) = isl_ports(neighbor, dir.opposite());
    udp_link(send, recv)
}

fn ground_link(point: Point) -> Result<UdpDatalink, CfsError> {
    let (send, recv) = ground_ports(point);
    udp_link(send, recv)
}

#[no_mangle]
pub extern "C" fn ROUTER_AppMain() {
    system::wait_for_startup_sync(Duration::from_millis(10_000));
    Runtime::new().run(async {
        event::register(&[])?;
        log!("Router app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
        let Address::Satellite(point) = address else {
            log!("Invalid spacecraft ID")?;
            return Ok::<(), CfsError>(());
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
        log!("Loaded {} APID routes", route_count)?;

        let send_mid = MsgId::local_cmd(bindings::ROUTER_SEND_TOPICID as u16);

        let mut pipe = Pipe::new("ROUTER_SB", 32)?;
        pipe.subscribe(send_mid)?;

        log!("Router ready, bridging SB and ISL")?;

        let mut from_net = [0u8; MTU];
        let mut from_sb = [0u8; MTU + SB_HEADER_SIZE];

        /// Delivers a packet to the local SB based on its APID.
        fn deliver_local(routes: &[Route], data: &[u8]) {
            let Ok(packet) = IslRoutingTelecommand::parse(data) else {
                return;
            };
            let Some(mid) = lookup_topic(routes, packet.apid().value()) else {
                return;
            };
            let _ = publish_to_sb(mid, data);
        }

        enum Event {
            Net(usize),
            Sb(usize),
            Err,
        }

        loop {
            let event = {
                let net_read = router.read(&mut from_net).fuse();
                let sb_read = pipe.recv(&mut from_sb).fuse();
                pin_utils::pin_mut!(net_read, sb_read);

                futures::select_biased! {
                    r = net_read => r.map(Event::Net).unwrap_or(Event::Err),
                    r = sb_read => r.map(Event::Sb).unwrap_or(Event::Err),
                }
            };

            match event {
                Event::Net(len) => {
                    deliver_local(&routes, &from_net[..len]);
                }
                Event::Sb(len) => {
                    let payload = &from_sb[SB_HEADER_SIZE..len];
                    let Ok(packet) = IslRoutingTelecommand::parse(payload) else {
                        continue;
                    };
                    if packet.target() == address {
                        deliver_local(&routes, payload);
                    } else {
                        let _ = router.write(payload).await;
                    }
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
