//! End-to-end tests for the cFS-side `SrsppEndpoint` using a mock
//! clock so the run loop never touches cFE FFI for time.
//!
//! Two paired endpoints share an in-memory link; assertions ride on
//! top of `endpoint.sender(...)` / `endpoint.listener()` views.

#![cfg(feature = "cfs-stubs")]

mod shims;

use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;
use std::collections::VecDeque;
use std::rc::Rc;

use futures::executor::block_on;
use futures::pin_mut;
use futures::select_biased;
use futures::FutureExt;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;

use leodos_protocols::buffer_pool::HeapBufferPool;
use leodos_protocols::network::NetworkRead;
use leodos_protocols::network::NetworkWrite;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::Clock;
use leodos_protocols::transport::srspp::api::cfs::RecvKind;
use leodos_protocols::transport::srspp::api::cfs::SrsppEndpoint;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

// ── Mock clock ───────────────────────────────────────────────

/// Test clock that never blocks on real time. `now()` returns a
/// fixed value; `sleep()` yields to the executor *once* and then
/// resolves, so the endpoint's run loop returns `Pending` between
/// iterations and concurrently-joined run loops actually get polled.
#[derive(Copy, Clone)]
struct MockClock;

impl Clock for MockClock {
    fn now(&self) -> SysTime {
        SysTime::from(Duration::from_millis(0))
    }
    fn sleep(&self, _duration: Duration) -> impl core::future::Future<Output = ()> {
        let mut yielded = false;
        poll_fn(move |cx| {
            if yielded {
                Poll::Ready(())
            } else {
                yielded = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
    }
}

// ── Mock link ────────────────────────────────────────────────

/// In-memory paired byte mirror. `pair()` returns two endpoints whose
/// writes flow into each other's reads. Single-threaded only.
struct MockLink {
    send_queue: Rc<RefCell<VecDeque<Vec<u8>>>>,
    recv_queue: Rc<RefCell<VecDeque<Vec<u8>>>>,
}

impl MockLink {
    fn pair() -> (Self, Self) {
        let a_to_b: Rc<RefCell<VecDeque<Vec<u8>>>> = Rc::new(RefCell::new(VecDeque::new()));
        let b_to_a: Rc<RefCell<VecDeque<Vec<u8>>>> = Rc::new(RefCell::new(VecDeque::new()));
        let a = MockLink {
            send_queue: a_to_b.clone(),
            recv_queue: b_to_a.clone(),
        };
        let b = MockLink {
            send_queue: b_to_a,
            recv_queue: a_to_b,
        };
        (a, b)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("mock link error")]
struct MockLinkError;

impl NetworkWrite for MockLink {
    type Error = MockLinkError;
    async fn write(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
        self.send_queue.borrow_mut().push_back(packet.to_vec());
        Ok(())
    }
}

impl NetworkRead for MockLink {
    type Error = MockLinkError;
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        poll_fn(|cx| {
            let mut q = self.recv_queue.borrow_mut();
            if let Some(packet) = q.pop_front() {
                let len = packet.len().min(buffer.len());
                buffer[..len].copy_from_slice(&packet[..len]);
                Poll::Ready(Ok(len))
            } else {
                // No data; ask the executor to re-poll us so we make
                // progress alongside the other join'd futures.
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        })
        .await
    }
}

// ── Test config ──────────────────────────────────────────────

const WIN: usize = 8;
const MTU: usize = 512;
const MAX_TX: usize = 4;
const MAX_STREAMS: usize = 4;
const SRSPP_BUF: usize = 4096;

fn addr_a() -> Address {
    Address::satellite(0, 1)
}
fn addr_b() -> Address {
    Address::satellite(0, 2)
}

fn sender_config(source: Address) -> SenderConfig {
    SenderConfig::builder()
        .source_address(source)
        .apid(Apid::new(0x42).unwrap())
        .function_code(0)
        .rto_ticks(1000)
        .max_retransmits(3)
        .header_overhead(SrsppDataPacket::HEADER_SIZE)
        .build()
}

fn receiver_config(local: Address) -> ReceiverConfig {
    ReceiverConfig::builder()
        .local_address(local)
        .apid(Apid::new(0x42).unwrap())
        .function_code(0)
        .immediate_ack(true)
        .ack_delay_ticks(100)
        .build()
}

// Tests run on cargo's default test-thread stack (~2 MB on Linux). The
// ReceiverMachine carries its reassembly + message buffers inline as
// `[u8; N]`, so generous defaults like 4 KB / 8 KB × MAX_STREAMS would
// overflow the stack once two endpoints sit there. Tiny buffers are
// fine for these tests — payloads are dozens of bytes at most.
const TEST_REASM: usize = 256;
const TEST_MSG_BUF: usize = 256;

type TestEndpoint<'pool> = SrsppEndpoint<
    'pool,
    MockLinkError,
    HeapBufferPool,
    NoStore,
    AlwaysReachable,
    ReceiverMachine<WIN, TEST_REASM, TEST_MSG_BUF>,
    WIN,
    MTU,
    MAX_TX,
    MAX_STREAMS,
>;

// ── Tests ────────────────────────────────────────────────────

/// Sender-side `send()` lands as a `RecvKind::Data` event on the
/// listener side, with the right source address and payload bytes.
#[test]
fn endpoint_data_roundtrip() {
    let pool_a = HeapBufferPool::new(64 * 1024);
    let pool_b = HeapBufferPool::new(64 * 1024);
    let endpoint_a: TestEndpoint = SrsppEndpoint::new(
        &pool_a,
        SRSPP_BUF,
        sender_config(addr_a()),
        receiver_config(addr_a()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();
    let endpoint_b: TestEndpoint = SrsppEndpoint::new(
        &pool_b,
        SRSPP_BUF,
        sender_config(addr_b()),
        receiver_config(addr_b()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();

    let (link_a, link_b) = MockLink::pair();
    let mut tx = endpoint_a.sender(addr_b()).unwrap();
    let mut listener = endpoint_b.listener().unwrap();

    block_on(async {
        let test = async {
            tx.send(b"hello").await.unwrap();
            tx.flush().await.unwrap();
            let mut buf = [0u8; 64];
            let (source, kind) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(source, addr_a());
            assert_eq!(kind, RecvKind::Data(5));
            assert_eq!(&buf[..5], b"hello");
        };
        let run_a = endpoint_a.run(link_a, FixedRto::new(1000), MockClock).fuse();
        let run_b = endpoint_b.run(link_b, FixedRto::new(1000), MockClock).fuse();
        pin_mut!(test, run_a, run_b);
        select_biased! {
            () = test.fuse() => {},
            r = run_a => panic!("endpoint_a.run exited: {:?}", r),
            r = run_b => panic!("endpoint_b.run exited: {:?}", r),
        }
    });
}

/// Empty payload (`send(&[])`) is a legitimate `RecvKind::Data(0)` —
/// not an EOS event. Guards against the bug we just fixed where EOS
/// detection was based on payload length.
#[test]
fn endpoint_empty_data_is_data_zero_not_eos() {
    let pool_a = HeapBufferPool::new(64 * 1024);
    let pool_b = HeapBufferPool::new(64 * 1024);
    let endpoint_a: TestEndpoint = SrsppEndpoint::new(
        &pool_a,
        SRSPP_BUF,
        sender_config(addr_a()),
        receiver_config(addr_a()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();
    let endpoint_b: TestEndpoint = SrsppEndpoint::new(
        &pool_b,
        SRSPP_BUF,
        sender_config(addr_b()),
        receiver_config(addr_b()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();

    let (link_a, link_b) = MockLink::pair();
    let mut tx = endpoint_a.sender(addr_b()).unwrap();
    let mut listener = endpoint_b.listener().unwrap();

    block_on(async {
        let test = async {
            tx.send(b"").await.unwrap();
            tx.flush().await.unwrap();
            let mut buf = [0u8; 64];
            let (source, kind) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(source, addr_a());
            assert_eq!(kind, RecvKind::Data(0), "empty DATA must not surface as Eos");
        };
        let run_a = endpoint_a.run(link_a, FixedRto::new(1000), MockClock).fuse();
        let run_b = endpoint_b.run(link_b, FixedRto::new(1000), MockClock).fuse();
        pin_mut!(test, run_a, run_b);
        select_biased! {
            () = test.fuse() => {},
            r = run_a => panic!("endpoint_a.run exited: {:?}", r),
            r = run_b => panic!("endpoint_b.run exited: {:?}", r),
        }
    });
}

/// `send_eos()` produces a `RecvKind::Eos` event on the listener side,
/// distinct from any data event.
#[test]
fn endpoint_eos_roundtrip() {
    let pool_a = HeapBufferPool::new(64 * 1024);
    let pool_b = HeapBufferPool::new(64 * 1024);
    let endpoint_a: TestEndpoint = SrsppEndpoint::new(
        &pool_a,
        SRSPP_BUF,
        sender_config(addr_a()),
        receiver_config(addr_a()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();
    let endpoint_b: TestEndpoint = SrsppEndpoint::new(
        &pool_b,
        SRSPP_BUF,
        sender_config(addr_b()),
        receiver_config(addr_b()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();

    let (link_a, link_b) = MockLink::pair();
    let mut tx = endpoint_a.sender(addr_b()).unwrap();
    let mut listener = endpoint_b.listener().unwrap();

    block_on(async {
        let test = async {
            // Sender FIFO (bug 1) is fixed, so send + send_eos can be
            // back-to-back without the first being clobbered. The
            // receiver's `complete_message_len` is still a single
            // slot (bug 2 in CLAUDE.md), so the consumer drains the
            // data event before letting EOS arrive — recv.await yields
            // to the run loops which deliver the data, then we recv
            // before queueing more.
            tx.send(b"data").await.unwrap();
            let mut buf = [0u8; 64];
            let (_src, k1) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(k1, RecvKind::Data(4));
            tx.send_eos().await.unwrap();
            tx.flush().await.unwrap();
            let (_src, k2) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(k2, RecvKind::Eos);
        };
        let run_a = endpoint_a.run(link_a, FixedRto::new(1000), MockClock).fuse();
        let run_b = endpoint_b.run(link_b, FixedRto::new(1000), MockClock).fuse();
        pin_mut!(test, run_a, run_b);
        select_biased! {
            () = test.fuse() => {},
            r = run_a => panic!("endpoint_a.run exited: {:?}", r),
            r = run_b => panic!("endpoint_b.run exited: {:?}", r),
        }
    });
}

/// Emulates the ping/pong demo: ground (A) opens a fresh per-ping
/// connection (new sender slot, seq starts at 0 again) and tears it
/// down with EOS; sat (B) replies with pong + EOS. Validates that the
/// receiver correctly reaps closed streams on DATA-after-EOS so the
/// second and subsequent cycles are not silently dropped as
/// duplicates.
#[tokio::test(flavor = "current_thread")]
async fn endpoint_multi_cycle_ping_pong() {
    let pool_ground = HeapBufferPool::new(64 * 1024);
    let pool_sat = HeapBufferPool::new(64 * 1024);
    let endpoint_ground: TestEndpoint = SrsppEndpoint::new(
        &pool_ground,
        SRSPP_BUF,
        sender_config(addr_a()),
        receiver_config(addr_a()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();
    let endpoint_sat: TestEndpoint = SrsppEndpoint::new(
        &pool_sat,
        SRSPP_BUF,
        sender_config(addr_b()),
        receiver_config(addr_b()),
        NoStore,
        AlwaysReachable,
    )
    .unwrap();

    let (link_ground, link_sat) = MockLink::pair();

    const CYCLES: usize = 3;

    // Each side fires DATA + EOS back-to-back, matching the real
    // ground tool / ping app flow. Bug 2 used to clobber DATA when
    // EOS landed at the receiver before the listener drained.
    let sat_serve = async {
        let mut buf = [0u8; 64];
        for cycle in 0..CYCLES {
            let mut listener = endpoint_sat.listener().unwrap();
            let (source, k1) = listener.recv(&mut buf).await.unwrap();
            assert!(matches!(k1, RecvKind::Data(_)), "cycle {} ping data", cycle);
            assert_eq!(&buf[..4], b"ping", "cycle {} ping payload", cycle);
            let (_, k2) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(k2, RecvKind::Eos, "cycle {} ping eos", cycle);
            drop(listener);
            let mut tx = endpoint_sat.sender(source).unwrap();
            tx.send(b"pong").await.unwrap();
            tx.send_eos().await.unwrap();
            tx.flush().await.unwrap();
            drop(tx);
        }
    };

    let ground_loop = async {
        let mut buf = [0u8; 64];
        for cycle in 0..CYCLES {
            let mut tx = endpoint_ground.sender(addr_b()).unwrap();
            tx.send(b"ping").await.unwrap();
            tx.send_eos().await.unwrap();
            tx.flush().await.unwrap();
            drop(tx);
            let mut listener = endpoint_ground.listener().unwrap();
            let (src, k1) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(src, addr_b(), "cycle {} pong source", cycle);
            assert!(matches!(k1, RecvKind::Data(_)), "cycle {} pong data", cycle);
            assert_eq!(&buf[..4], b"pong", "cycle {} pong payload", cycle);
            let (_, k2) = listener.recv(&mut buf).await.unwrap();
            assert_eq!(k2, RecvKind::Eos, "cycle {} pong eos", cycle);
            drop(listener);
        }
    };

    let test = async {
        futures::future::join(ground_loop, sat_serve).await;
    };
    let run_g = endpoint_ground.run(link_ground, FixedRto::new(1000), MockClock).fuse();
    let run_s = endpoint_sat.run(link_sat, FixedRto::new(1000), MockClock).fuse();
    pin_mut!(test, run_g, run_s);
    select_biased! {
        () = test.fuse() => {},
        r = run_g => panic!("endpoint_ground.run exited: {:?}", r),
        r = run_s => panic!("endpoint_sat.run exited: {:?}", r),
    }
}
