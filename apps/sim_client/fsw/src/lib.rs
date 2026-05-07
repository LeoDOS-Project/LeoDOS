//! cFS bridge between the leo-viz simulator and the local
//! software bus.
//!
//! At boot, opens a TCP connection to the address specified in
//! the `LEODOS_BRIDGE_ADDR` env var (host:port), writes one
//! [`Hello`] identifying this CPU's spacecraft id, and then loops
//! reading [`StateFrame`]s — one per simulator tick — and
//! republishes each as a [`BridgeStateTlm`] message on the cFE
//! software bus. Consumer apps subscribe to
//! [`SIM_CLIENT_BRIDGE_STATE_TOPICID`] for GPS-like position/
//! velocity and link visibility without coupling to the bridge's
//! wire format.
//!
//! Replaces the per-device hardware drivers (GPS, IMU, mag, …) on
//! simulator builds. On real hardware the per-device cFS apps are
//! loaded instead and this app stays unloaded.

#![no_std]
#![deny(unsafe_code)]

use core::time::Duration;
use leodos_libcfs::bridge::EventFrame;
use leodos_libcfs::bridge::Hello;
use leodos_libcfs::bridge::StateFrame;
use leodos_libcfs::cfe::es::app;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::evs::tlm::LongEventPayload;
use leodos_libcfs::cfe::evs::tlm::long_event_mid;
use leodos_libcfs::cfe::sb::msg::MessageRef;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::pipe::Timeout;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::log;
use leodos_libcfs::os::env;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::SocketDomain;
use leodos_libcfs::os::net::TcpStream;
use leodos_libcfs::os::task::delay;
use zerocopy::FromBytes;
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

const RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
const EVS_PIPE_DEPTH: u16 = 32;
/// Max EVS message bytes — must match `CFE_MISSION_EVS_MAX_MESSAGE_LENGTH`.
const MAX_EVS_RECV: usize = 256;

/// Per-tick telemetry produced from a decoded leo-viz [`StateFrame`].
/// Host endian; struct layout matches what consumer apps deserialize.
#[repr(C)]
#[derive(Clone, Copy, IntoBytes, Immutable, KnownLayout)]
pub struct BridgeStateTlm {
    /// Sequence number from the bridge server.
    pub seq: u32,
    /// Spacecraft id this entry corresponds to.
    pub scid: u32,
    /// Sim clock (ms since simulation epoch).
    pub sim_time_ms: u64,
    /// ECI position in meters.
    pub pos_eci_m: [f64; 3],
    /// ECI velocity in m/s.
    pub vel_eci_m_s: [f64; 3],
    /// Body→ECI nadir-pointing quaternion (w, x, y, z).
    pub nadir_quat: [f64; 4],
    /// Bitmask of torus neighbors currently in line of sight.
    pub los_neighbors: u8,
    /// Reserved.
    pub _pad: [u8; 1],
    /// Bitmask of ground stations currently in view.
    pub los_ground: u16,
    /// Trailing padding.
    pub _trail: [u8; 4],
}

fn convert(scid: u32, frame: &StateFrame) -> BridgeStateTlm {
    let mut pos = [0.0; 3];
    let mut vel = [0.0; 3];
    let mut quat = [0.0; 4];
    for i in 0..3 {
        pos[i] = frame.pos_eci_m[i].get();
        vel[i] = frame.vel_eci_m_s[i].get();
    }
    for i in 0..4 {
        quat[i] = frame.nadir_quat[i].get();
    }
    BridgeStateTlm {
        seq: frame.seq.get(),
        scid,
        sim_time_ms: frame.sim_time_ms.get(),
        pos_eci_m: pos,
        vel_eci_m_s: vel,
        nadir_quat: quat,
        los_neighbors: frame.los_neighbors,
        _pad: [0; 1],
        los_ground: frame.los_ground.get(),
        _trail: [0; 4],
    }
}

fn parse_addr(s: &str) -> Option<SocketAddr> {
    let (host, port_str) = s.rsplit_once(':')?;
    let port: u16 = port_str.parse().ok()?;
    SocketAddr::resolve_ipv4(host, port).ok()
}

fn read_exact(stream: &mut TcpStream, buf: &mut [u8]) -> bool {
    let mut filled = 0;
    while filled < buf.len() {
        match stream.read(&mut buf[filled..]) {
            Ok(0) => return false,
            Ok(n) => filled += n,
            Err(_) => return false,
        }
    }
    true
}

fn write_all(stream: &mut TcpStream, buf: &[u8]) -> bool {
    let mut sent = 0;
    while sent < buf.len() {
        match stream.write(&buf[sent..]) {
            Ok(0) => return false,
            Ok(n) => sent += n,
            Err(_) => return false,
        }
    }
    true
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn SIM_CLIENT_AppMain() {
    system::wait_for_startup_sync(Duration::from_millis(10_000));
    if event::register(&[]).is_err() {
        return;
    }

    let scid = system::get_spacecraft_id();
    let topic = bindings::SIM_CLIENT_BRIDGE_STATE_TOPICID as u16;
    let mid = MsgId::local_tlm(topic);

    let addr_str: heapless::String<128> = match env::var("LEODOS_BRIDGE_ADDR") {
        Some(v) => v,
        None => {
            log!("SIM_CLIENT: LEODOS_BRIDGE_ADDR not set, exiting").ok();
            return;
        }
    };
    let Some(addr) = parse_addr(&addr_str) else {
        log!("SIM_CLIENT: cannot resolve LEODOS_BRIDGE_ADDR").ok();
        return;
    };

    log!("SIM_CLIENT: scid={} dialing bridge", scid).ok();

    let mut evs_pipe = match Pipe::new("SIM_CLIENT_EVS", EVS_PIPE_DEPTH) {
        Ok(p) => match p.subscribe(long_event_mid()) {
            Ok(()) => Some(p),
            Err(_) => {
                log!("SIM_CLIENT: EVS subscribe failed").ok();
                None
            }
        },
        Err(_) => {
            log!("SIM_CLIENT: EVS pipe create failed").ok();
            None
        }
    };

    while app::run_loop().is_ok() {
        match TcpStream::connect(addr.clone(), SocketDomain::IPv4) {
            Ok(mut stream) => {
                let hello = Hello::new(scid);
                if !write_all(&mut stream, hello.as_bytes()) {
                    log!("SIM_CLIENT: hello write failed").ok();
                } else {
                    log!("SIM_CLIENT: scid={} bridge connected", scid).ok();
                    pump(&mut stream, scid, mid, evs_pipe.as_mut());
                }
            }
            Err(_) => {
                log!("SIM_CLIENT: connect failed, retrying").ok();
            }
        }
        let _ = delay(RECONNECT_BACKOFF);
    }
}

fn pump(stream: &mut TcpStream, scid: u32, mid: MsgId, mut evs_pipe: Option<&mut Pipe>) {
    let mut buf = [0u8; core::mem::size_of::<StateFrame>()];
    let mut event_seq: u32 = 0;
    while app::run_loop().is_ok() {
        if !read_exact(stream, &mut buf) {
            log!("SIM_CLIENT: bridge disconnected").ok();
            return;
        }
        let Ok(frame) = StateFrame::read_from_bytes(&buf) else {
            log!("SIM_CLIENT: frame decode failed").ok();
            return;
        };
        if frame.validate().is_err() {
            log!("SIM_CLIENT: frame validation failed").ok();
            return;
        }
        let tlm = convert(scid, &frame);
        let _ = SendBuffer::publish_typed(mid, &tlm);

        if let Some(pipe) = evs_pipe.as_deref_mut() {
            if !drain_events(stream, pipe, scid, frame.sim_time_ms.get(), &mut event_seq) {
                log!("SIM_CLIENT: bridge write failed during event drain").ok();
                return;
            }
        }
    }
}

/// Pull every queued EVS event out of `pipe` (non-blocking) and ship
/// each as an [`EventFrame`] over the bridge stream. Returns `false`
/// on bridge write failure so the caller can reconnect.
fn drain_events(
    stream: &mut TcpStream,
    pipe: &mut Pipe,
    scid: u32,
    sim_time_ms: u64,
    seq: &mut u32,
) -> bool {
    let mut recv_buf = [0u8; MAX_EVS_RECV];
    loop {
        let len = match pipe.timed_recv(&mut recv_buf, Timeout::Poll) {
            Ok(n) => n,
            Err(_) => return true,
        };
        if len == 0 {
            return true;
        }
        let msg = MessageRef::new(&recv_buf[..len]);
        let Ok(payload) = msg.payload::<LongEventPayload>() else {
            continue;
        };
        let frame = EventFrame::new(
            *seq,
            sim_time_ms,
            scid,
            payload.packet_id.event_id.get(),
            payload.packet_id.event_type.get() as u8,
            payload.app_name_str().as_bytes(),
            payload.message_str().as_bytes(),
        );
        *seq = seq.wrapping_add(1);
        if !write_all(stream, frame.as_bytes()) {
            return false;
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
