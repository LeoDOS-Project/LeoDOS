#![no_std]

use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::error::Error;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::datalink::link::cfs::udp::UdpDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppReceiver;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

const LOCAL_IP: &str = "127.0.0.1";
const LOCAL_PORT: u16 = 5002;
const REMOTE_IP: &str = "127.0.0.1";
const REMOTE_PORT: u16 = 5001;

#[no_mangle]
pub extern "C" fn SRSPP_RECEIVER_AppMain() {
    Runtime::new().run(async {
        event::register(&[])?;
        event::info(0, "SRSPP Receiver starting")?;

        let local_addr = SocketAddr::new_ipv4(LOCAL_IP, LOCAL_PORT)?;
        let remote_addr = SocketAddr::new_ipv4(REMOTE_IP, REMOTE_PORT)?;
        let datalink = UdpDatalink::bind(local_addr, remote_addr)?;
        let network = PointToPoint::new(datalink);

        let config = ReceiverConfig {
            local_address: Address::satellite(0, 2),
            apid: Apid::new(0x50).unwrap(),
            function_code: 0,
            immediate_ack: false,
            ack_delay_ticks: 100,
            progress_timeout_ticks: None,
        };

        let receiver: SrsppReceiver<Error> = SrsppReceiver::new(config);
        let (mut handle, mut driver) = receiver.split::<_, 512>(network);

        let recv_task = async {
            let mut buf = [0u8; 8192];
            loop {
                let (_, len) = match handle.recv(&mut buf).await {
                    Ok(r) => r,
                    Err(_) => break,
                };
                if let Ok(text) = core::str::from_utf8(&buf[..len]) {
                    event::info(1, text).ok();
                } else {
                    event::info(1, "Received binary message").ok();
                }
            }
        };

        let _ = join(recv_task, driver.run()).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
