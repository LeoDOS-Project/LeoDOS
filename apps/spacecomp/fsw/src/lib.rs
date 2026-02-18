#![no_std]

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::datalink::link::cfs::UdpDataLink;
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::local::LocalChannel;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::torus::{Point, Torus};
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::NetworkLayer;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

mod handler;
mod isl;
mod roles;

const IP: &str = "127.0.0.1";
const PORT_BASE: u16 = 6000;
const PORTS_PER_SAT: u16 = 10;

pub const NUM_ORBITS: u8 = 20;
pub const NUM_SATS: u8 = 72;
pub const INCLINATION_RAD: f32 = 87.0 * (core::f32::consts::PI / 180.0);

const APID: u16 = 0x60;

const PORT_NORTH: u16 = 0;
const PORT_SOUTH: u16 = 2;
const PORT_EAST: u16 = 4;
const PORT_WEST: u16 = 6;
const PORT_GROUND: u16 = 8;

fn sat_base_port(orbit: u8, sat: u8) -> u16 {
    PORT_BASE + (orbit as u16 * NUM_SATS as u16 + sat as u16) * PORTS_PER_SAT
}

fn udp_link(local_port: u16, remote_port: u16) -> Result<UdpDataLink, leodos_libcfs::error::Error> {
    let local = SocketAddr::new_ipv4(IP, local_port)?;
    let remote = SocketAddr::new_ipv4(IP, remote_port)?;
    UdpDataLink::bind(local, remote)
}

#[no_mangle]
pub extern "C" fn SPACECOMP_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        event::info(0, "SpaceCoMP app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id() as u16);
        let address: Address = scid.into();
        let (orbit, sat) = match address {
            Address::Satellite {
                orbit_id,
                satellite_id,
            } => (orbit_id, satellite_id),
            _ => {
                event::info(0, "Invalid spacecraft ID")?;
                return Ok(());
            }
        };

        let local_node = Point::new(orbit, sat);

        let my_base = sat_base_port(orbit, sat);

        let north_neighbor = (orbit, Torus::next(sat, NUM_SATS));
        let south_neighbor = (orbit, Torus::prev(sat, NUM_SATS));
        let east_neighbor = (Torus::next(orbit, NUM_ORBITS), sat);
        let west_neighbor = (Torus::prev(orbit, NUM_ORBITS), sat);

        let north = udp_link(
            my_base + PORT_NORTH,
            sat_base_port(north_neighbor.0, north_neighbor.1) + PORT_SOUTH + 1,
        )?;
        let south = udp_link(
            my_base + PORT_SOUTH,
            sat_base_port(south_neighbor.0, south_neighbor.1) + PORT_NORTH + 1,
        )?;
        let east = udp_link(
            my_base + PORT_EAST,
            sat_base_port(east_neighbor.0, east_neighbor.1) + PORT_WEST + 1,
        )?;
        let west = udp_link(
            my_base + PORT_WEST,
            sat_base_port(west_neighbor.0, west_neighbor.1) + PORT_EAST + 1,
        )?;
        let ground = udp_link(my_base + PORT_GROUND, my_base + PORT_GROUND + 1)?;

        let local_channel: LocalChannel<8, 1024> = LocalChannel::new();
        let (mut app_link, router_link) = local_channel.split();

        let algorithm = DistanceMinimizing::new(INCLINATION_RAD);

        let mut router = Router::new(
            north,
            south,
            east,
            west,
            ground,
            router_link,
            address,
            NUM_ORBITS,
            NUM_SATS,
            algorithm,
        );

        let ctx = isl::Context {
            local_address: address,
            apid: Apid::new(APID).unwrap(),
        };

        let app_task = async {
            let mut state = handler::State::new();
            let mut buf = [0u8; 1024];
            let mut payload_buf = [0u8; 512];

            loop {
                let len = match app_link.recv(&mut buf).await {
                    Ok(len) => len,
                    Err(_) => break,
                };

                let cmd = isl::parse_and_copy(&buf[..len], &mut payload_buf);
                let cmd = match cmd {
                    Some(c) => c,
                    None => continue,
                };

                state
                    .handle(
                        &mut app_link,
                        &ctx,
                        local_node,
                        cmd.op_code,
                        cmd.job_id,
                        &payload_buf[..cmd.payload_len],
                    )
                    .await;
            }
        };

        let _ = join(router.run(), app_task).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
