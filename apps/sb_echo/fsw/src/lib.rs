#![no_std]
#![deny(unsafe_code)]

use core::time::Duration;
use futures::FutureExt as _;
use leodos_libcfs::cfe::es::system;
use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::log;
use leodos_libcfs::os::net::SocketAddr;
use leodos_libcfs::os::net::UdpSocket;
use leodos_libcfs::runtime::Runtime;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn SB_ECHO_AppMain() {
    system::wait_for_startup_sync(Duration::from_millis(10_000));
    Runtime::new().run(async {
        event::register(&[])?;
        log!("SB_ECHO: starting")?;

        let topic = bindings::ROUTER_SEND_TOPICID as u16;
        let mid = MsgId::local_cmd(topic);
        log!("SB_ECHO: subscribing to topic 0x{:02X}", topic)?;

        let mut pipe = Pipe::new("SB_ECHO", 16)?;
        pipe.subscribe(mid)?;

        // Second async source — a dummy UDP socket (never receives)
        let sock = UdpSocket::bind(SocketAddr::new_ipv4("127.0.0.1", 9876)?)?;
        let mut udp_buf = [0u8; 64];

        let mut sb_buf = [0u8; 512];

        // Simulate the router's nested select pattern:
        // inner_read() has its own select_biased! that always returns Pending
        // outer select_biased! should still poll pipe.recv()
        async fn inner_read(sock: &UdpSocket, buf: &mut [u8]) -> usize {
            // Mimics Router::read() — a loop with select_biased! inside
            loop {
                let read1 = sock.recv(buf).fuse();
                let read2 = futures::future::pending::<()>().fuse();
                pin_utils::pin_mut!(read1, read2);
                futures::select_biased! {
                    r = read1 => match r {
                        Ok(len) => return len.0,
                        Err(_) => continue,
                    },
                    _ = read2 => {}
                }
            }
        }

        loop {
            let net_read = inner_read(&sock, &mut udp_buf).fuse();
            let sb_read = pipe.recv(&mut sb_buf).fuse();
            pin_utils::pin_mut!(net_read, sb_read);

            futures::select_biased! {
                _len = net_read => {
                    log!("SB_ECHO: udp recv (unexpected)")?;
                }
                r = sb_read => {
                    let Ok(len) = r else { continue };
                    log!("SB_ECHO: select recv {} bytes", len)?;
                }
            }
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
