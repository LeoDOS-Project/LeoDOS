#![no_std]

use leodos_libcfs::log;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;
use leodos_libcfs::runtime::Runtime;
use leodos_protocols::network::cfe::tc::Telecommand;
use leodos_protocols::network::cfe::tm::Telemetry;
use zerocopy::IntoBytes;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

#[no_mangle]
pub extern "C" fn SAMPLE_RUST_AppMain() {
    Runtime::new().run(async {
        let socket = UdpSocket::bind(SocketAddr::new_ipv4("0.0.0.0", 1236)?)?;

        let mut input_buffer = [0u8; 256];
        let mut output_buffer = [0u8; 256];

        loop {
            let Ok((bytes_received, addr)) = socket.recv(&mut input_buffer).await else {
                continue;
            };

            let Ok(tc) = Telecommand::parse(&input_buffer[..bytes_received]) else {
                continue;
            };

            if !tc.validate_cfe_checksum() {
                log!("SAMPLE_RUST_APP: Checksum validation failed").ok();
                continue;
            }
            let message = core::str::from_utf8(tc.payload()).unwrap_or("<invalid utf8>");

            log!("SAMPLE_RUST_APP: Got message {}", message).ok();
            let sequence_count = tc.sequence_count();

            let response = "Hello from SAMPLE_RUST_APP! ";

            let tm = Telemetry::builder()
                .buffer(&mut output_buffer)
                .apid(tc.apid())
                .sequence_count(sequence_count)
                .payload_len(response.len())
                .time(0)
                .build()
                .expect("Should build Telemetry packet");

            tm.payload_mut().copy_from_slice(response.as_bytes());

            socket.send(tm.as_bytes(), &addr).await?;
        }
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
