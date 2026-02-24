#![no_std]

use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::routing::algorithm::distance_minimizing::DistanceMinimizing;
use leodos_protocols::network::isl::routing::local::LocalChannel;
use leodos_protocols::network::isl::routing::Router;
use leodos_protocols::network::isl::torus::Point;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::NetworkLayer;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

pub mod data;
mod handler;
mod isl;
mod net;
mod roles;

pub const NUM_ORBITS: u8 = bindings::SPACECOMP_NUM_ORBITS as u8;
pub const NUM_SATS: u8 = bindings::SPACECOMP_NUM_SATS as u8;
pub const INCLINATION_RAD: f32 = 87.0 * (core::f32::consts::PI / 180.0);

const APID: u16 = bindings::SPACECOMP_APID as u16;

#[no_mangle]
pub extern "C" fn SPACECOMP_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        event::info(0, "SpaceCoMP app starting")?;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
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
        let links = net::bind_isl_links(orbit, sat)?;

        let local_channel: LocalChannel<8, 1024> = LocalChannel::new();
        let (mut app_link, router_link) = local_channel.split();

        let algorithm = DistanceMinimizing::new(INCLINATION_RAD);

        let mut router = Router::new(
            links.north,
            links.south,
            links.east,
            links.west,
            links.ground,
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
