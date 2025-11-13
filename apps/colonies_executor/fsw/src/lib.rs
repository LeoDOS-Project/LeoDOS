#![no_std]

mod functions;
mod transport;

use leodos_libcfs::log;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::mission::colonies::ColoniesExecutor;
use leodos_protocols::network::cfe::tc::Telecommand;
use leodos_protocols::network::spp::Apid;

use crate::functions::Handler;
use crate::transport::UdpLink;

const MY_APID: u16 = 0x100;

#[no_mangle]
pub extern "C" fn COLONIES_EXECUTOR_Main() {
    let mut buffer = [0u8; 4096];
    Runtime::new().run(async {
        log!("COLONIES_APP: Starting...")?;

        let socket = UdpSocket::bind(SocketAddr::new_ipv4("0.0.0.0", 5000)?)?;
        let target = SocketAddr::new_ipv4("127.0.0.1", 5001)?;
        let uplink = UdpLink::new(&socket, target);

        let handler = Handler::new();
        let apid = Apid::new(MY_APID).expect("Invalid APID");

        let mut executor = ColoniesExecutor::new(uplink, handler, apid);

        log!("COLONIES_APP: Listening for tasks...")?;

        loop {
            let Ok((len, _src)) = socket.recv(&mut buffer).await else {
                continue;
            };

            let (packet_bytes, buffer) = buffer.split_at_mut(len);

            let Ok(tc) = Telecommand::parse(packet_bytes) else {
                log!("COLONIES_APP: Invalid Telecommand received")?;
                continue;
            };

            if !tc.validate_cfe_checksum() {
                log!("COLONIES_APP: Checksum validation failed")?;
                continue;
            }

            if let Err(e) = executor.handle_packet(buffer, tc).await {
                log!("COLONIES_APP: Executor error {}", e)?;
            }
        }
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
