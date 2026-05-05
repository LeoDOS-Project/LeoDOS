//! cFS bridge between the walker-delta simulator and the local
//! software bus.
//!
//! Listens for walker-delta state packets on
//! [`leodos_libcfs::bridge::TOPOLOGY_PORT`], filters out the entry
//! matching this CPU's spacecraft id, and republishes it as a
//! [`BridgeStateTlm`] message on the cFE software bus. Consumer
//! apps subscribe to [`SIM_CLIENT_BRIDGE_STATE_TOPICID`] to obtain
//! GPS-like position/velocity and link visibility without coupling
//! to the bridge's wire format.
//!
//! Replaces the per-device hardware drivers (GPS, IMU, mag, …) on
//! simulator builds. On real hardware the per-device cFS apps are
//! loaded instead and this app stays unloaded.

#![no_std]
#![deny(unsafe_code)]

use core::time::Duration;
use leodos_libcfs::bridge::DecodeError;
use leodos_libcfs::bridge::SatState;
use leodos_libcfs::bridge::TOPOLOGY_PORT;
use leodos_libcfs::bridge::decode_state;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::log;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;
use leodos_libcfs::runtime::Runtime;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const RECV_BUF_BYTES: usize = 65_536;

/// Per-tick telemetry produced from a decoded walker-delta packet.
/// Host endian; struct layout matches what consumer apps deserialize.
#[repr(C)]
#[derive(Clone, Copy, IntoBytes, Immutable, KnownLayout)]
pub struct BridgeStateTlm {
    /// Sequence number from the bridge publisher.
    pub seq: u32,
    /// Spacecraft id this entry corresponds to.
    pub scid: u32,
    /// Sim clock (ms since simulation epoch) when the snapshot was taken.
    pub sim_time_ms: u64,
    /// ECI position in meters.
    pub pos_eci_m: [f64; 3],
    /// ECI velocity in m/s.
    pub vel_eci_m_s: [f64; 3],
    /// Body→ECI nadir-pointing quaternion (w, x, y, z).
    pub nadir_quat: [f64; 4],
    /// Bitmask of torus neighbors currently in line of sight.
    pub los_neighbors: u8,
    /// Reserved for alignment.
    pub _pad: [u8; 1],
    /// Bitmask of ground stations currently in view.
    pub los_ground: u16,
    /// Trailing padding to a multiple of 8 bytes (struct alignment).
    pub _trail: [u8; 4],
}

fn convert(scid: u32, seq: u32, sim_time_ms: u64, sat: &SatState) -> BridgeStateTlm {
    let mut pos = [0.0; 3];
    let mut vel = [0.0; 3];
    let mut quat = [0.0; 4];
    for i in 0..3 {
        pos[i] = sat.pos_eci_m[i].get();
        vel[i] = sat.vel_eci_m_s[i].get();
    }
    for i in 0..4 {
        quat[i] = sat.nadir_quat[i].get();
    }
    BridgeStateTlm {
        seq,
        scid,
        sim_time_ms,
        pos_eci_m: pos,
        vel_eci_m_s: vel,
        nadir_quat: quat,
        los_neighbors: sat.los_neighbors,
        _pad: [0; 1],
        los_ground: sat.los_ground.get(),
        _trail: [0; 4],
    }
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn SIM_CLIENT_AppMain() {
    system::wait_for_startup_sync(Duration::from_millis(10_000));
    Runtime::new().run(async {
        event::register(&[])?;

        let scid = system::get_spacecraft_id();
        let topic = bindings::SIM_CLIENT_BRIDGE_STATE_TOPICID as u16;
        let mid = MsgId::local_tlm(topic);
        log!(
            "SIM_CLIENT: scid={} listening on UDP :{}",
            scid,
            TOPOLOGY_PORT,
        )?;

        let bind_addr = SocketAddr::new_ipv4("0.0.0.0", TOPOLOGY_PORT)?;
        let sock = UdpSocket::bind(bind_addr)?;
        let mut buf = [0u8; RECV_BUF_BYTES];

        loop {
            let (len, _src) = sock.recv(&mut buf).await?;
            let (header, sats) = match decode_state(&buf[..len]) {
                Ok(v) => v,
                Err(DecodeError::BadMagic) => continue,
                Err(e) => {
                    log!("SIM_CLIENT: decode error {:?}", e)?;
                    continue;
                }
            };

            let Some(sat) = sats.iter().find(|s| s.scid.get() == scid) else {
                continue;
            };

            let tlm = convert(scid, header.seq.get(), header.sim_time_ms.get(), sat);
            SendBuffer::publish_typed(mid, &tlm)?;
        }

        #[allow(unreachable_code)]
        Ok::<(), leodos_libcfs::error::CfsError>(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
