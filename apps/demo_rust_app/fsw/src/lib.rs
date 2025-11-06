#![no_std]

use leodos_ccsds::cfe::tc::Telecommand;
use leodos_ccsds::spp::SpacePacket;
use leodos_libcfs::log;
use leodos_libcfs::log::syslog;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;
use leodos_libcfs::runtime::scope::Scope;
use leodos_libcfs::runtime::sync::spsc;
use leodos_libcfs::runtime::Runtime;
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

#[repr(C)]
#[derive(Debug, Clone, Copy, IntoBytes, FromBytes, KnownLayout, Unaligned, Immutable)]
struct NoArgsCmd {}

#[no_mangle]
pub extern "C" fn RUST_Main() {
    Runtime::new().run(async {
        let mut channel = spsc::channel::<u8, 1024>();
        let mut scope = Scope::<10, 8>::new();
        let (mut tx, mut rx) = channel.split();
        scope.spawn(async move {
            loop {
                let value = rx.recv().await;
                log!("RUST_APP: Received value: {}", value).ok();
            }
        })?;
        scope.spawn(async move {
            let counter: u8 = 0;
            loop {
                tx.send(counter).await;
            }
        })?;
        scope.spawn(async move {
            let socket_addr = SocketAddr::new_ipv4("0.0.0.0", 1236)?;
            let mut socket = UdpSocket::bind(socket_addr)?;

            let mut udp_buffer = [0u8; 256];

            loop {
                let Ok((bytes_received, _addr)) = socket.recv(&mut udp_buffer).await else {
                    continue;
                };

                let Ok(sp) = SpacePacket::parse(&udp_buffer[..bytes_received]) else {
                    syslog("RUST_APP: Failed to parse space packet").ok();
                    continue;
                };

                let Ok(cmd) = <&Telecommand<NoArgsCmd>>::try_from(sp) else {
                    syslog("RUST_APP: Failed to convert to Telecommand").ok();
                    continue;
                };

                if !cmd.validate_cfe_checksum() {
                    syslog("RUST_APP: Checksum validation failed").ok();
                    continue;
                }

                syslog("RUST_APP: init() called, socket bound").ok();
            }
        })?;
        scope.await
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
