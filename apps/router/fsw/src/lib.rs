#![no_std]
#![deny(unsafe_code)]

use core::fmt::Write as _;
use core::time::Duration;
use futures::FutureExt as _;
use leodos_libcfs::cfe::es::pool::MemPool;
use leodos_libcfs::cfe::es::pool::MemPoolStorage;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::cfe::tbl::Table;
use leodos_libcfs::cfe::tbl::TableOptions;
use leodos_libcfs::cfe::tbl::Validate;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::log;
use leodos_libcfs::os::fs::AccessMode;
use leodos_libcfs::os::fs::File;
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

/// Compile-time fallback used when the runtime config table is
/// missing the field (legacy `router_ground.bin` written by an
/// older leo-viz). Lives only in the build's C header.
const DEFAULT_NUM_ORBS: u8 = bindings::ROUTER_NUM_ORBS as u8;
const DEFAULT_NUM_SATS: u8 = bindings::ROUTER_NUM_SATS as u8;
const DEFAULT_ALTITUDE_M: f32 = 550_000.0;
const DEFAULT_INCLINATION_DEG: f32 = 87.0;

const LOCALHOST: &str = "127.0.0.1";
const PORT_BASE: u16 = 6000;

/// EVS event id fired on every forwarded ISL packet. Message format
/// is `fwd src=<endpoint> dst=<endpoint>` where each endpoint is
/// `sat:<scid>` or `gnd:<station>`. leo-viz parses this to draw a
/// transient line between the two endpoints.
const ROUTER_FORWARD_EID: u16 = 100;

const ROUTER_GROUND_MAX_STATIONS: usize = bindings::ROUTER_MAX_GROUND_STATIONS as usize;

/// Mirrors `RouterGroundEntry_t` in
/// `apps/router/fsw/tables/router_ground.h`.
#[repr(C)]
#[derive(Clone, Copy, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::KnownLayout, zerocopy::Immutable)]
struct RouterGroundEntry {
    station_id: u8,
    _pad: [u8; 3],
    lat_deg: f32,
    lon_deg: f32,
}

/// Mirrors `RouterGroundTable_t` in the same header. Loaded from
/// the leo-viz-written file at [`ROUTER_GROUND_RUNTIME_PATH`] if
/// present, otherwise from cFE Table Services'
/// `/cf/router_ground.tbl` (the compiled-in default). Valid
/// entries are `entries[0..count]`.
#[repr(C)]
#[derive(Clone, Copy, zerocopy::FromBytes, zerocopy::IntoBytes, zerocopy::KnownLayout, zerocopy::Immutable)]
struct RouterGroundTable {
    num_orbs: u8,
    num_sats: u8,
    _pad0: [u8; 2],
    altitude_m: f32,
    inclination_deg: f32,
    count: u8,
    _pad1: [u8; 3],
    entries: [RouterGroundEntry; ROUTER_GROUND_MAX_STATIONS],
}

/// Path to the runtime-overridden ground-station file written by
/// leo-viz into the bind-mounted log dir before launching the
/// container. Plain `RouterGroundTable` bytes (host endian, x86_64
/// little-endian on both sides). When present, supersedes the
/// compiled-in default at `/cf/router_ground.tbl`.
const ROUTER_GROUND_RUNTIME_PATH: &str = "/tmp/leodos/router_ground.bin";

impl Default for RouterGroundTable {
    fn default() -> Self {
        Self {
            num_orbs: DEFAULT_NUM_ORBS,
            num_sats: DEFAULT_NUM_SATS,
            _pad0: [0; 2],
            altitude_m: DEFAULT_ALTITUDE_M,
            inclination_deg: DEFAULT_INCLINATION_DEG,
            count: 0,
            _pad1: [0; 3],
            entries: [RouterGroundEntry {
                station_id: 0,
                _pad: [0; 3],
                lat_deg: 0.0,
                lon_deg: 0.0,
            }; ROUTER_GROUND_MAX_STATIONS],
        }
    }
}

impl Validate for RouterGroundTable {}

/// Best-effort read of [`ROUTER_GROUND_RUNTIME_PATH`] into a
/// `RouterGroundTable`. Returns `None` if the file is missing,
/// truncated, or fails any sanity check.
fn read_runtime_ground_table() -> Option<RouterGroundTable> {
    use zerocopy::FromBytes;
    let mut file = File::open(ROUTER_GROUND_RUNTIME_PATH, AccessMode::ReadOnly).ok()?;
    let mut buf = [0u8; core::mem::size_of::<RouterGroundTable>()];
    let n = file.sync_read(&mut buf).ok()?;
    if n != buf.len() {
        return None;
    }
    let table = RouterGroundTable::read_from_bytes(&buf).ok()?;
    if (table.count as usize) > ROUTER_GROUND_MAX_STATIONS {
        return None;
    }
    Some(table)
}

/// Append a `sat:<scid>` or `gnd:<station>` token for `addr` to `out`.
fn write_endpoint<const N: usize>(out: &mut heapless::String<N>, addr: Address, num_sats: u8) {
    match addr {
        Address::Satellite(p) => {
            let scid = p.orb as u16 * num_sats as u16 + p.sat as u16;
            let _ = write!(out, "sat:{}", scid);
        }
        Address::Ground { station } => {
            let _ = write!(out, "gnd:{}", station);
        }
    }
}

/// Fire an EVS event describing one forwarded ISL packet. The
/// emitting spacecraft is the source (it's the one forwarding); the
/// EVS event header carries the emitting scid.
fn emit_forward_event(dst: Address, num_sats: u8) {
    let mut msg: heapless::String<96> = heapless::String::new();
    let _ = msg.push_str("fwd dst=");
    write_endpoint(&mut msg, dst, num_sats);
    let _ = event::info(ROUTER_FORWARD_EID, &msg);
}
const PORTS_PER_SAT: u16 = 5;
const MTU: usize = 1024;

const SB_HEADER_SIZE: usize = 8;

const MAX_ROUTES: usize = bindings::ROUTER_MAX_ROUTES as usize;

/// Backing memory for the router's buffer pool. Size: 5 ports × 2
/// MTU buffers + cFE pool overhead, rounded up.
const POOL_BYTES: usize = 5 * 2 * MTU + 1024;
static POOL_STORAGE: MemPoolStorage<POOL_BYTES> = MemPoolStorage::new();

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
                topic: MsgId::local_cmd($topic as u16),
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
    add_route!(
        bindings::ROUTER_ROUTE_3_APID,
        bindings::ROUTER_ROUTE_3_TOPIC
    );
    let len = table.len();
    (table, len)
}

fn lookup_topic(table: &[Route], apid: u16) -> Option<MsgId> {
    table.iter().find(|r| r.apid == apid).map(|r| r.topic)
}

fn isl_port_offset(dir: Direction) -> u16 {
    match dir {
        Direction::North => 0,
        Direction::South => 1,
        Direction::East => 2,
        Direction::West => 3,
    }
}

const GROUND_OFFSET: u16 = 4;

/// Unique port base for a satellite, accounting for both orbit and sat index.
fn sat_port_base(point: Point, num_sats: u8) -> u16 {
    PORT_BASE + (point.orb as u16 * num_sats as u16 + point.sat as u16) * PORTS_PER_SAT
}

/// Returns the bidirectional UDP port for an ISL direction.
fn isl_port(point: Point, dir: Direction, num_sats: u8) -> u16 {
    sat_port_base(point, num_sats) + isl_port_offset(dir)
}

/// Returns the bidirectional UDP port for the ground link.
fn ground_port(point: Point, num_sats: u8) -> u16 {
    sat_port_base(point, num_sats) + GROUND_OFFSET
}

fn udp_link(local_port: u16, remote_port: u16) -> Result<UdpDatalink, CfsError> {
    let local = SocketAddr::new_ipv4(LOCALHOST, local_port)?;
    let remote = SocketAddr::new_ipv4(LOCALHOST, remote_port)?;
    UdpDatalink::bind(local, remote)
}

fn isl_link(point: Point, dir: Direction, torus: Torus, num_sats: u8) -> Result<UdpDatalink, CfsError> {
    let neighbor = torus.neighbor(point, dir);
    let local = isl_port(point, dir, num_sats);
    let remote = isl_port(neighbor, dir.opposite(), num_sats);
    udp_link(local, remote)
}

/// Port the single ground station process binds.
/// Shared destination for all satellites' ground-bound traffic.
const GROUND_STATION_PORT: u16 = 9000;

fn ground_link(point: Point, num_sats: u8) -> Result<UdpDatalink, CfsError> {
    let local = ground_port(point, num_sats);
    udp_link(local, GROUND_STATION_PORT)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn ROUTER_AppMain() {
    system::wait_for_startup_sync(Duration::from_millis(10_000));
    Runtime::new().run(async {
        event::register(&[])?;
        log!("Router app starting")?;

        // Map the leo-viz-written runtime config directory into OSAL
        // so `File::open` can reach it. PSP only maps `/cf` by default.
        // Idempotent across multiple apps; ignore "already exists".
        let _ = leodos_libcfs::os::fs::add_fixed_map(
            "/tmp/leodos",
            "/tmp/leodos",
        );

        // Load constellation + ground-station config. Prefer the
        // leo-viz-written runtime file (set per-spawn so the cFS side
        // matches the UI); fall back to the compiled-in .tbl with
        // default 3x3 / 550km / 87° values.
        let ground_table =
            Table::<RouterGroundTable>::new("GroundTable", TableOptions::DEFAULT)?;
        let mut loaded_from_runtime = false;
        if let Some(parsed) = read_runtime_ground_table() {
            if ground_table.load_from_slice(&[parsed]).is_ok() {
                log!(
                    "Router: config loaded from runtime ({}x{}, alt={}m, incl={}°, {} stations)",
                    parsed.num_orbs,
                    parsed.num_sats,
                    parsed.altitude_m as u32,
                    parsed.inclination_deg as u32,
                    parsed.count
                )?;
                loaded_from_runtime = true;
            } else {
                log!("Router: runtime ground table load_from_slice failed")?;
            }
        }
        if !loaded_from_runtime {
            if let Err(e) = ground_table.load_from_file("/cf/router_ground.tbl") {
                log!("Router: ground table load failed: {:?}", e)?;
            }
        }
        let config = ground_table.get_or_default();
        let torus = Torus::new(config.num_orbs, config.num_sats);
        let shell = Shell::new(torus, config.altitude_m, config.inclination_deg);
        let num_sats = config.num_sats;

        let scid = SpacecraftId::new(system::get_spacecraft_id());
        let Some(address) = scid.to_address(config.num_orbs, config.num_sats) else {
            log!("Invalid spacecraft ID")?;
            return Ok::<(), CfsError>(());
        };
        let Address::Satellite(point) = address else { unreachable!() };

        let mut gateway_table = GatewayTable::<4>::new(5.0);
        let n = (config.count as usize).min(ROUTER_GROUND_MAX_STATIONS);
        for entry in config.entries[..n].iter() {
            gateway_table.add_station(
                entry.station_id,
                LatLon::new(entry.lat_deg, entry.lon_deg),
            );
        }

        let pool = MemPool::new(POOL_STORAGE.take()?, false)?;

        let mut router: Router<'_, _, _, _, _, _, 2048> = Router::builder()
            .pool(&pool)
            .mtu(MTU)
            .north(isl_link(point, Direction::North, torus, num_sats)?)
            .south(isl_link(point, Direction::South, torus, num_sats)?)
            .east(isl_link(point, Direction::East, torus, num_sats)?)
            .west(isl_link(point, Direction::West, torus, num_sats)?)
            .ground(ground_link(point, num_sats)?)
            .address(address)
            .algorithm(DistanceMinimizing::new(shell, gateway_table))
            .clock(MetClock::new())
            .build()?;

        let (routes, route_count) = build_routing_table();
        log!("Loaded {} APID routes", route_count)?;

        let send_mid = MsgId::local_cmd(bindings::ROUTER_SEND_TOPICID as u16);

        let mut pipe = Pipe::new("ROUTER_SB", 32)?;
        pipe.subscribe(send_mid)?;

        log!("Router ready, bridging SB and ISL")?;

        let mut from_net = [0u8; MTU];
        let mut from_sb = [0u8; MTU + SB_HEADER_SIZE];

        fn deliver_local(routes: &[Route], data: &[u8]) {
            let Ok(packet) = IslRoutingTelecommand::parse(data) else {
                return;
            };
            let Some(mid) = lookup_topic(routes, packet.apid().value()) else {
                return;
            };
            let _ = SendBuffer::publish(mid, data);
        }

        enum Event {
            Net(usize),
            Sb(usize),
            Diag,
            Err,
        }

        loop {
            let event = {
                let net_read = router.read(&mut from_net).fuse();
                let sb_read = pipe.recv(&mut from_sb).fuse();
                let diag_tick = leodos_libcfs::runtime::time::sleep(
                    leodos_libcfs::cfe::duration::Duration::from_millis(1000),
                )
                .fuse();
                pin_utils::pin_mut!(net_read, sb_read, diag_tick);

                futures::select_biased! {
                    r = net_read => r.map(Event::Net).unwrap_or(Event::Err),
                    r = sb_read => r.map(Event::Sb).unwrap_or(Event::Err),
                    () = diag_tick => Event::Diag,
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
                        emit_forward_event(packet.target(), num_sats);
                        let _ = router.write(payload).await;
                    }
                }
                Event::Diag => {
                    // Surface only spin-loop-level activity (>10k
                    // router-loop iterations per second). Normal
                    // forwarding traffic is <<1k/s.
                    let d = router.take_diag();
                    if d.iterations > 10_000 {
                        let (from_port, target_raw, hop) = d.last_route;
                        let _ = log!(
                            "Router: spin? iter={} e_r={} w_w={} last from={} target=0x{:04x} hop={}",
                            d.iterations,
                            d.isl_reads[2],
                            d.isl_writes[3],
                            from_port,
                            target_raw,
                            hop,
                        );
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
