#![no_std]

use heapless::Vec as HVec;
use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::error::Error;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::time::sleep;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppSender;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::rto::FixedRto;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const LOCAL_IP: &str = "127.0.0.1";
const LOCAL_PORT: u16 = 5001;
const REMOTE_IP: &str = "127.0.0.1";
const REMOTE_PORT: u16 = 5002;

fn format_u32(mut n: u32, buf: &mut [u8; 10]) -> &[u8] {
    if n == 0 {
        buf[0] = b'0';
        return &buf[0..1];
    }
    let mut i = 10;
    while n > 0 && i > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    &buf[i..]
}

#[no_mangle]
pub extern "C" fn SRSPP_SENDER_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        event::info(0, "SRSPP Sender starting")?;

        let local_addr = SocketAddr::new_ipv4(LOCAL_IP, LOCAL_PORT)?;
        let remote_addr = SocketAddr::new_ipv4(REMOTE_IP, REMOTE_PORT)?;
        let datalink = UdpDatalink::bind(local_addr, remote_addr)?;
        let mut network = PointToPoint::new(datalink);

        let target = Address::satellite(0, 2);
        let origin = Address::satellite(0, 1);
        let config = SenderConfig {
            source_address: origin,
            apid: Apid::new(0x50).unwrap(),
            function_code: 0,
            rto_ticks: 1000,
            max_retransmits: 3,
            header_overhead: leodos_protocols::transport::srspp::packet::SrsppDataPacket::HEADER_SIZE,
        };

        let sender = SrsppSender::new(config, origin, NoStore, AlwaysReachable);
        let (mut handle, mut driver) = sender.split(FixedRto::new(1000));

        let send_task = async {
            let mut counter: u32 = 0;
            loop {
                let mut msg: HVec<u8, 64> = HVec::new();
                let _ = msg.extend_from_slice(b"Hello from sender #");

                let mut num_buf = [0u8; 10];
                let num_str = format_u32(counter, &mut num_buf);
                let _ = msg.extend_from_slice(num_str);

                if handle.send(target, &msg[..]).await.is_err() {
                    break;
                }
                event::info(1, "Sent message").ok();

                counter = counter.wrapping_add(1);
                sleep(Duration::from_secs(2)).await;
            }
        };

        let _ = join(send_task, driver.run(&mut network)).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
