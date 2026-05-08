#![no_std]
#![deny(unsafe_code)]

use core::time::Duration;

use leodos_libcfs::cfe::es::pool::MemPool;
use leodos_libcfs::cfe::es::pool::MemPoolStorage;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::log;
use leodos_libcfs::runtime::Runtime;

use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::SpacecraftId;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppListener;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

use zerocopy::network_endian::U32;
use zerocopy::network_endian::U64;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const MTU: usize = 512;
const SRSPP_BUF_SIZE: usize = 4096;

/// Backing memory for the ping app's SRSPP buffer pool. Sized for
/// the sender's send buffer + driver tx + driver recv buffers + cFE
/// pool overhead.
const POOL_BYTES: usize = SRSPP_BUF_SIZE + 2 * MTU + 1024;
static POOL_STORAGE: MemPoolStorage<POOL_BYTES> = MemPoolStorage::new();

/// Ping request: identifies the message and records the ground send time.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone)]
pub struct PingPayload {
    pub seq: U32,
    pub sent_ms: U64,
}

/// Pong reply: echoes the ping seq and reports the responding satellite.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone)]
pub struct PongPayload {
    pub seq: U32,
    pub scid: U32,
    pub orb: u8,
    pub sat: u8,
    pub _pad: [u8; 2],
    pub met_seconds: U32,
    pub met_subseconds: U32,
    pub sent_ms: U64,
}

#[no_mangle]
pub extern "C" fn PING_AppMain() {
    system::wait_for_startup_sync(Duration::from_millis(10_000));
    Runtime::new().run(async {
        if let Err(e) = run().await {
            let _ = log!("Ping app exited: {}", e);
        }
        Ok::<(), CfsError>(())
    });
}

async fn run() -> Result<(), CfsError> {
    event::register(&[])?;
    log!("Ping app starting")?;

    let scid = system::get_spacecraft_id();
    let num_sats = bindings::PING_NUM_SATS as u8;
    let num_orbits = bindings::PING_NUM_ORBITS as u8;
    let Some(address) = SpacecraftId::new(scid).to_address(num_orbits, num_sats) else {
        log!("Ping: scid {} is not a configured satellite", scid)?;
        return Ok(());
    };
    let Address::Satellite(point) = address else { unreachable!() };

    let recv_mid = MsgId::local_cmd(bindings::PING_ROUTER_RECV_TOPICID as u16);
    let send_mid = MsgId::local_cmd(bindings::ROUTER_SEND_TOPICID as u16);
    let sb_link = SbDatalink::new("PING_ISL", 32, recv_mid, send_mid)?;
    let network = PointToPoint::new(sb_link);

    let apid = Apid::new(bindings::PING_APID as u16).expect("valid APID");
    let sender_config = SenderConfig::builder()
        .source_address(address)
        .apid(apid)
        .function_code(0)
        .rto_ticks(1000)
        .max_retransmits(3)
        .header_overhead(SrsppDataPacket::HEADER_SIZE)
        .build();

    let receiver_config = ReceiverConfig::builder()
        .local_address(address)
        .apid(apid)
        .function_code(0)
        .immediate_ack(true)
        .ack_delay_ticks(100)
        .build();

    let pool = MemPool::new(POOL_STORAGE.take()?, false)?;

    let listener: SrsppListener<
        '_,
        CfsError,
        _,
        NoStore,
        AlwaysReachable,
        ReceiverMachine<8, 4096, 8192>,
        8,
        512,
        4,
    > = SrsppListener::new(
        &pool,
        SRSPP_BUF_SIZE,
        sender_config,
        receiver_config,
        NoStore,
        AlwaysReachable,
    )?;

    log!("Ping ready on sat({}, {})", point.orb, point.sat)?;

    listener
        .listen(network, FixedRto::new(1000), |mut stream, mut tx, _| async move {
            let mut recv_buf = [0u8; 128];
            let len = stream.recv(&mut recv_buf).await?;
            let source = stream.source();
            let Ok(ping) = PingPayload::ref_from_bytes(&recv_buf[..len]) else {
                let _ = log!("Ping: bad payload ({} bytes)", len);
                return Ok(());
            };
            let seq = ping.seq.get();
            let sent_ms = ping.sent_ms.get();
            let _ = log!("Ping: seq={} from {:?}", seq, source);
            let met = SysTime::now_met();
            let pong = PongPayload {
                seq: U32::new(seq),
                scid: U32::new(scid),
                orb: point.orb,
                sat: point.sat,
                _pad: [0; 2],
                met_seconds: U32::new(met.seconds()),
                met_subseconds: U32::new(met.subseconds()),
                sent_ms: U64::new(sent_ms),
            };
            tx.send(&pong).await?;
            Ok(())
        })
        .await
        .map_err(|e| {
            let _ = log!("Ping: listener exited: {}", e);
            CfsError::Osal(leodos_libcfs::error::OsalError::Error)
        })?;
    Ok(())
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
