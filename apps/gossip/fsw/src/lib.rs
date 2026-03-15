#![no_std]

use futures::FutureExt as _;
use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::es::system;
use zerocopy::FromBytes as _;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::Runtime;
use leodos_libcfs::runtime::time::sleep;
use leodos_libcfs::{err, info};
use zerocopy::IntoBytes;

use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::datalink::{DatalinkRead, DatalinkWrite};
use leodos_protocols::network::isl::address::{Address, SpacecraftId};
use leodos_protocols::network::isl::gossip::packet::IslGossipTelecommand;
use leodos_protocols::network::isl::gossip::{GossipChannel, GossipConfig};
use leodos_protocols::network::isl::torus::{Direction, Point, Torus};
use leodos_protocols::network::spp::Apid;

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
const PORTS_PER_SAT: u16 =
    bindings::GOSSIP_PORTS_PER_SAT as u16;
const INTERVAL_SECS: u32 =
    bindings::GOSSIP_INTERVAL_SECS as u32;

const MTU: usize = 512;
const SB_HEADER_SIZE: usize = 8;

const GOSSIP_APID: u16 = bindings::GOSSIP_APID as u16;
const GOSSIP_FC: u8 =
    bindings::GOSSIP_FUNCTION_CODE as u8;
const SW_VERSION: u8 = bindings::GOSSIP_SW_VERSION as u8;

#[repr(C, packed)]
#[derive(
    Clone,
    Copy,
    zerocopy::FromBytes,
    zerocopy::IntoBytes,
    zerocopy::Immutable,
    zerocopy::KnownLayout,
)]
struct HealthState {
    orb: u8,
    sat: u8,
    alive: u8,
    sw_version: u8,
    pass_count: u16,
    error_count: u16,
}

fn isl_port_offset(dir: Direction) -> u16 {
    match dir {
        Direction::North => 0,
        Direction::South => 2,
        Direction::East => 4,
        Direction::West => 6,
    }
}

fn isl_ports(
    point: Point,
    dir: Direction,
) -> (u16, u16) {
    let base = PORT_BASE
        + point.sat as u16 * PORTS_PER_SAT
        + isl_port_offset(dir);
    (base, base + 1)
}

fn orb_ip(
    orb: u8,
    out: &mut [u8; 16],
) -> Result<&str, CfsError> {
    leodos_protocols::fmt!(out, "172.20.{orb}.10")
        .ok()
        .and_then(|len| {
            core::str::from_utf8(&out[..len]).ok()
        })
        .ok_or(CfsError::ValidationFailure)
}

fn local_link(
    local_port: u16,
    remote_port: u16,
) -> Result<UdpDatalink, CfsError> {
    let local =
        SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote =
        SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn remote_link(
    local_port: u16,
    remote_orb: u8,
    remote_port: u16,
) -> Result<UdpDatalink, CfsError> {
    let mut buf = [0u8; 16];
    let ip = orb_ip(remote_orb, &mut buf)?;
    let local =
        SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote =
        SocketAddr::new_ipv4(ip, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn isl_link(
    point: Point,
    dir: Direction,
) -> Result<UdpDatalink, CfsError> {
    let neighbor = TORUS.neighbor(point, dir);
    let (send, _) = isl_ports(point, dir);
    let (_, recv) =
        isl_ports(neighbor, dir.opposite());
    if point.orb == neighbor.orb {
        local_link(send, recv)
    } else {
        remote_link(send, neighbor.orb, recv)
    }
}

fn publish_to_sb(
    mid: MsgId,
    data: &[u8],
) -> Result<(), CfsError> {
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

async fn try_read_one(
    link: &mut UdpDatalink,
    buf: &mut [u8],
) -> Option<usize> {
    let read = link.read(buf).fuse();
    let timeout = sleep(Duration::from_millis(10)).fuse();
    pin_utils::pin_mut!(read, timeout);

    futures::select_biased! {
        r = read => r.ok(),
        _ = timeout => None,
    }
}

#[no_mangle]
pub extern "C" fn GOSSIP_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        info!("Gossip app starting")?;

        let scid =
            SpacecraftId::new(system::get_spacecraft_id());
        let address = scid.to_address(NUM_SATS);
        let Address::Satellite(point) = address else {
            err!("Invalid spacecraft ID for gossip")?;
            return Ok(());
        };

        let mut links = [
            isl_link(point, Direction::North)?,
            isl_link(point, Direction::South)?,
            isl_link(point, Direction::East)?,
            isl_link(point, Direction::West)?,
        ];

        let apid = Apid::new(GOSSIP_APID)
            .map_err(|_| CfsError::ValidationFailure)?;

        let channel = GossipChannel::<8, 256>::new(
            GossipConfig {
                torus: TORUS,
                my_address: address,
                apid,
                function_code: GOSSIP_FC,
            },
        );

        let (mut sender, mut receiver, driver) =
            channel.split();

        let hk_mid = MsgId::from_local_tlm(
            bindings::GOSSIP_HK_TLM_TOPICID as u16,
        );

        info!(
            "Gossip ready at ({}, {})",
            point.orb, point.sat
        )?;

        let mut pass_count: u16 = 0;
        let mut error_count: u16 = 0;
        let mut pkt_buf = [0u8; MTU];
        let mut recv_buf = [0u8; MTU];
        let mut fwd_buf = [0u8; MTU];

        loop {
            sleep(Duration::from_secs(INTERVAL_SECS)).await;
            pass_count = pass_count.wrapping_add(1);

            let state = HealthState {
                orb: point.orb,
                sat: point.sat,
                alive: 1,
                sw_version: SW_VERSION,
                pass_count,
                error_count,
            };

            if sender
                .send(0, u8::MAX, state.as_bytes())
                .await
                .is_err()
            {
                error_count =
                    error_count.wrapping_add(1);
            }

            while let Some((len, directions)) =
                driver.poll_outgoing(&mut pkt_buf)
            {
                for dir in directions.iter() {
                    let idx = dir_index(*dir);
                    if links[idx]
                        .write(&pkt_buf[..len])
                        .await
                        .is_err()
                    {
                        error_count =
                            error_count.wrapping_add(1);
                    }
                }
            }

            for dir_idx in 0..4 {
                loop {
                    let len = match try_read_one(
                        &mut links[dir_idx],
                        &mut recv_buf,
                    )
                    .await
                    {
                        Some(n) if n > 0 => n,
                        _ => break,
                    };

                    fwd_buf[..len]
                        .copy_from_slice(&recv_buf[..len]);

                    let pkt = match
                        IslGossipTelecommand::ref_from_bytes(
                            &fwd_buf[..len],
                        )
                    {
                        Ok(p) => p,
                        Err(_) => continue,
                    };

                    let forwards =
                        driver.process_incoming(pkt);

                    let mut fwd_dirs: heapless::Vec<
                        Direction,
                        4,
                    > = heapless::Vec::new();
                    for (d, _) in forwards.iter() {
                        let _ = fwd_dirs.push(*d);
                    }

                    for d in fwd_dirs.iter() {
                        let idx = dir_index(*d);
                        let _ = links[idx]
                            .write(&fwd_buf[..len])
                            .await;
                    }
                }
            }

            let mut payload_buf = [0u8; 256];
            while let Some(msg) =
                receiver.try_recv(&mut payload_buf)
            {
                let _ = publish_to_sb(
                    hk_mid,
                    &payload_buf[..msg.len],
                );
            }
        }

        #[allow(unreachable_code)]
        Ok(())
    });
}

fn dir_index(dir: Direction) -> usize {
    match dir {
        Direction::North => 0,
        Direction::South => 1,
        Direction::East => 2,
        Direction::West => 3,
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(
        info,
    )
}
