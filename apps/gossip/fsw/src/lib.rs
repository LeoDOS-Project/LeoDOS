#![no_std]

use futures::FutureExt as _;
use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::{Pipe, Timeout};
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::time::sleep;
use leodos_libcfs::runtime::Runtime;
use leodos_libcfs::{err, info};

use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::gossip::Gossip;
use leodos_protocols::network::isl::torus::{Direction, Point, Torus};
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::{NetworkRead, NetworkWrite};

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const NUM_ORBS: u8 = bindings::GOSSIP_NUM_ORBS as u8;
const NUM_SATS: u8 = bindings::GOSSIP_NUM_SATS as u8;
const TORUS: Torus = Torus::new(NUM_ORBS, NUM_SATS);

const LOCALHOST: &str = "127.0.0.1";
const PORT_BASE: u16 = bindings::GOSSIP_PORT_BASE as u16;
const PORTS_PER_SAT: u16 = bindings::GOSSIP_PORTS_PER_SAT as u16;
const INTERVAL_SECS: u32 = bindings::GOSSIP_INTERVAL_SECS as u32;
const MAX_ROUTES: usize = bindings::GOSSIP_MAX_ROUTES as usize;

const MTU: usize = 512;
const SB_HEADER_SIZE: usize = 8;

const GOSSIP_APID: u16 = bindings::GOSSIP_APID as u16;
const GOSSIP_FC: u8 = bindings::GOSSIP_FUNCTION_CODE as u8;

struct Route {
    apid: u16,
    topic: MsgId,
}

fn build_routing_table() -> heapless::Vec<Route, MAX_ROUTES> {
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
        bindings::GOSSIP_ROUTE_0_APID,
        bindings::GOSSIP_ROUTE_0_TOPIC
    );
    add_route!(
        bindings::GOSSIP_ROUTE_1_APID,
        bindings::GOSSIP_ROUTE_1_TOPIC
    );
    add_route!(
        bindings::GOSSIP_ROUTE_2_APID,
        bindings::GOSSIP_ROUTE_2_TOPIC
    );
    table
}

fn lookup_topic(table: &[Route], apid: u16) -> Option<MsgId> {
    table.iter().find(|r| r.apid == apid).map(|r| r.topic)
}

fn isl_port_offset(dir: Direction) -> u16 {
    match dir {
        Direction::North => 0,
        Direction::South => 2,
        Direction::East => 4,
        Direction::West => 6,
    }
}

fn isl_ports(point: Point, dir: Direction) -> (u16, u16) {
    let base = PORT_BASE
        + point.sat as u16 * PORTS_PER_SAT
        + isl_port_offset(dir);
    (base, base + 1)
}

fn udp_link(
    local_port: u16,
    remote_port: u16,
) -> Result<UdpDatalink, CfsError> {
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn isl_link(
    point: Point,
    dir: Direction,
) -> Result<UdpDatalink, CfsError> {
    let neighbor = TORUS.neighbor(point, dir);
    let (send, _) = isl_ports(point, dir);
    let (_, recv) = isl_ports(neighbor, dir.opposite());
    udp_link(send, recv)
}

fn publish_to_sb(
    mid: MsgId,
    data: &[u8],
) -> Result<(), CfsError> {
    let total = SB_HEADER_SIZE + data.len();
    let mut buf = SendBuffer::new(total)?;
    {
        let mut msg = buf.view();
        msg.init(mid, total)?;
        let slice = buf.as_mut_slice();
        slice[SB_HEADER_SIZE..].copy_from_slice(data);
    }
    buf.send(true)?;
    Ok(())
}

#[no_mangle]
pub extern "C" fn GOSSIP_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        info!("Gossip app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
        let Address::Satellite(point) = address else {
            err!("Invalid spacecraft ID")?;
            return Ok(());
        };

        let apid = Apid::new(GOSSIP_APID)
            .map_err(|_| CfsError::ValidationFailure)?;

        let mut gossip: Gossip<UdpDatalink> = Gossip::builder()
            .north(isl_link(point, Direction::North)?)
            .south(isl_link(point, Direction::South)?)
            .east(isl_link(point, Direction::East)?)
            .west(isl_link(point, Direction::West)?)
            .address(address)
            .torus(TORUS)
            .apid(apid)
            .function_code(GOSSIP_FC)
            .build();

        let routes = build_routing_table();
        info!("Loaded {} APID routes", routes.len())?;

        let send_mid = MsgId::from_local_cmd(
            bindings::GOSSIP_SEND_TOPICID as u16,
        );
        let mut pipe = Pipe::new("GOSSIP_SB", 16)?;
        pipe.subscribe(send_mid)?;

        info!(
            "Gossip ready at ({}, {})",
            point.orb, point.sat
        )?;

        let mut recv_buf = [0u8; MTU];
        let mut sb_buf = [0u8; MTU + SB_HEADER_SIZE];

        enum Event {
            Received(usize),
            Timeout,
            Err,
        }

        loop {
            // Drain SB pipe for outbound gossip from local apps
            loop {
                match pipe.timed_recv(
                    &mut sb_buf,
                    Timeout::Poll,
                ) {
                    Ok(len) if len > SB_HEADER_SIZE => {
                        let payload =
                            &sb_buf[SB_HEADER_SIZE..len];
                        let _ = gossip.write(payload).await;
                    }
                    _ => break,
                }
            }

            // Read from ISL with a timeout so we can
            // check the SB pipe again
            let event = {
                let read =
                    gossip.read(&mut recv_buf).fuse();
                let timeout = sleep(
                    Duration::from_secs(INTERVAL_SECS),
                )
                .fuse();
                pin_utils::pin_mut!(read, timeout);

                futures::select_biased! {
                    r = read => match r {
                        Ok(len) => Event::Received(len),
                        Err(_) => Event::Err,
                    },
                    _ = timeout => Event::Timeout,
                }
            };

            if let Event::Received(len) = event {
                let data = &recv_buf[..len];
                if data.len() >= 2 {
                    let apid = u16::from_be_bytes(
                        [data[0], data[1]],
                    ) & 0x07FF;
                    if let Some(mid) =
                        lookup_topic(&routes, apid)
                    {
                        let _ = publish_to_sb(mid, data);
                    }
                }
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
