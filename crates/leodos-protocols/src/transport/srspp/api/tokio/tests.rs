#![allow(unused)]

use crate::buffer_pool::HeapBufferPool;
use crate::network::{NetworkRead, NetworkWrite};
use crate::network::isl::address::Address;
use crate::network::isl::torus::Point;
use crate::network::spp::Apid;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::rto::FixedRto;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio::time::Instant;

use super::{SrsppReceiver, SrsppSender};

const SRSPP_WIN: usize = 8;
const SRSPP_BUF: usize = 4096;
const SRSPP_MTU: usize = 512;
const SRSPP_REASM: usize = 8192;
const TICKS_PER_SEC: u32 = 1000;
const RTO_MS: u32 = 1000;

const APID: u16 = 0x42;
const SAT_A: Address = Address::Satellite(Point { orb: 0, sat: 1 });
const SAT_B: Address = Address::Satellite(Point { orb: 0, sat: 2 });

fn pool() -> HeapBufferPool {
    HeapBufferPool::new(SRSPP_BUF + 2 * SRSPP_MTU + 1024)
}

fn sender_config(source: Address) -> SenderConfig {
    SenderConfig {
        source_address: source,
        apid: Apid::new(APID).unwrap(),
        function_code: 0,
        rto_ticks: RTO_MS,
        max_retransmits: 3,
        header_overhead: SrsppDataPacket::HEADER_SIZE,
    }
}

fn receiver_config(local: Address) -> ReceiverConfig {
    ReceiverConfig {
        local_address: local,
        apid: Apid::new(APID).unwrap(),
        function_code: 0,
        immediate_ack: true,
        ack_delay_ticks: 100,
        progress_timeout_ticks: None,
    }
}

struct MockLink {
    send_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
    recv_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
}

impl MockLink {
    fn pair() -> (MockLink, MockLink) {
        let a_to_b = Arc::new(Mutex::new(VecDeque::new()));
        let b_to_a = Arc::new(Mutex::new(VecDeque::new()));
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

impl NetworkWrite for MockLink {
    type Error = std::io::Error;
    async fn write(&mut self, packet: &[u8]) -> Result<(), Self::Error> {
        self.send_queue.lock().await.push_back(packet.to_vec());
        Ok(())
    }
}

impl NetworkRead for MockLink {
    type Error = std::io::Error;
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        loop {
            if let Some(packet) = self.recv_queue.lock().await.pop_front() {
                let len = packet.len().min(buffer.len());
                buffer[..len].copy_from_slice(&packet[..len]);
                return Ok(len);
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

#[tokio::test]
async fn srspp_one_way_delivery() {
    let (link_a, link_b) = MockLink::pair();
    let pool_a = pool();

    let mut sender: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_A),
        link_a,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_a,
        SRSPP_BUF,
    )
    .unwrap();
    let mut receiver: SrsppReceiver<_, ReceiverMachine<SRSPP_WIN, SRSPP_BUF, SRSPP_REASM>, SRSPP_MTU> =
        SrsppReceiver::new(receiver_config(SAT_B), SAT_A, link_b, TICKS_PER_SEC);

    let message = b"Hello, srspp!";

    let send = async {
        sender.send(SAT_B, message).await.unwrap();
        sender.flush().await.unwrap();
    };
    let recv = async {
        let mut buf = [0u8; 8192];
        let len = receiver
            .recv(&mut buf)
            .await
            .unwrap()
            .expect("unexpected EOS");
        buf[..len].to_vec()
    };

    let ((), received) = tokio::join!(send, recv);
    assert_eq!(received.as_slice(), message);
}

/// Two independent SRSPP streams (one per direction). A sends a request, B echoes,
/// A receives the reply. The roundtrip must complete well under SRSPP RTO so we'd
/// notice if the driver ever stalls between request handling and reply send.
#[tokio::test]
async fn srspp_request_reply_roundtrip() {
    let (req_a, req_b) = MockLink::pair();
    let (rep_b, rep_a) = MockLink::pair();
    let pool_a = pool();
    let pool_b = pool();

    let mut a_send: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_A),
        req_a,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_a,
        SRSPP_BUF,
    )
    .unwrap();
    let mut a_recv: SrsppReceiver<_, ReceiverMachine<SRSPP_WIN, SRSPP_BUF, SRSPP_REASM>, SRSPP_MTU> =
        SrsppReceiver::new(receiver_config(SAT_A), SAT_B, rep_a, TICKS_PER_SEC);

    let mut b_recv: SrsppReceiver<_, ReceiverMachine<SRSPP_WIN, SRSPP_BUF, SRSPP_REASM>, SRSPP_MTU> =
        SrsppReceiver::new(receiver_config(SAT_B), SAT_A, req_b, TICKS_PER_SEC);
    let mut b_send: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_B),
        rep_b,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_b,
        SRSPP_BUF,
    )
    .unwrap();

    let request = b"PING";
    let reply = b"PONG";

    let a_task = async {
        let started = Instant::now();
        a_send.send(SAT_B, request).await.unwrap();
        a_send.flush().await.unwrap();
        let mut buf = [0u8; 8192];
        let len = a_recv.recv(&mut buf).await.unwrap();
        (buf[..len].to_vec(), started.elapsed())
    };

    let b_task = async {
        let mut buf = [0u8; 8192];
        let len = b_recv.recv(&mut buf).await.unwrap();
        let got = buf[..len].to_vec();
        b_send.send(SAT_A, reply).await.unwrap();
        b_send.flush().await.unwrap();
        got
    };

    let ((received_reply, elapsed), received_request) = tokio::join!(a_task, b_task);

    assert_eq!(received_request.as_slice(), request);
    assert_eq!(received_reply.as_slice(), reply);
    assert!(
        elapsed < Duration::from_secs(2),
        "roundtrip took {elapsed:?}",
    );
}

// ── DTN store-and-forward tests ─────────────────────────────

use super::SrsppDtnSender;
use crate::transport::srspp::dtn::Reachable;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

/// Toggleable reachability oracle so tests can simulate going in
/// and out of line-of-sight.
struct ToggleReachable(Arc<AtomicBool>);

impl Reachable for ToggleReachable {
    fn is_reachable(&self, _origin: Address, _target: Address) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// When the target is reachable, DTN-send goes straight to the wire.
#[tokio::test]
async fn dtn_reachable_target_sends_immediately() {
    let (link_a, link_b) = MockLink::pair();
    let pool_a = pool();

    let inner: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_A),
        link_a,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_a,
        SRSPP_BUF,
    )
    .unwrap();
    let toggle = Arc::new(AtomicBool::new(true));
    let mut sender =
        SrsppDtnSender::new(inner, ToggleReachable(toggle.clone()), SAT_A, 16);
    let mut receiver: SrsppReceiver<
        _,
        ReceiverMachine<SRSPP_WIN, SRSPP_BUF, SRSPP_REASM>,
        SRSPP_MTU,
    > = SrsppReceiver::new(receiver_config(SAT_B), SAT_A, link_b, TICKS_PER_SEC);

    let send = async {
        sender.send(SAT_B, &b"hi"[..]).await.unwrap();
        assert_eq!(sender.pending(SAT_B), 0, "nothing should be buffered");
        sender.flush().await.unwrap();
    };
    let recv = async {
        let mut buf = [0u8; 64];
        let len = receiver.recv(&mut buf).await.unwrap();
        buf[..len].to_vec()
    };
    let ((), got) = tokio::join!(send, recv);
    assert_eq!(got.as_slice(), b"hi");
}

/// Sends while unreachable accumulate in the per-target store and
/// nothing reaches the wire.
#[tokio::test]
async fn dtn_unreachable_target_buffers() {
    let (link_a, _link_b) = MockLink::pair();
    let pool_a = pool();

    let inner: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_A),
        link_a,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_a,
        SRSPP_BUF,
    )
    .unwrap();
    let toggle = Arc::new(AtomicBool::new(false));
    let mut sender =
        SrsppDtnSender::new(inner, ToggleReachable(toggle.clone()), SAT_A, 16);

    sender.send(SAT_B, &b"one"[..]).await.unwrap();
    sender.send(SAT_B, &b"two"[..]).await.unwrap();
    sender.send(SAT_B, &b"three"[..]).await.unwrap();

    assert_eq!(sender.pending(SAT_B), 3);
    assert_eq!(sender.pending_total(), 3);
}

/// When the target transitions from unreachable to reachable, the
/// next reachable-side send drains the store first.
#[tokio::test]
async fn dtn_drains_on_reachability_change() {
    let (link_a, link_b) = MockLink::pair();
    let pool_a = pool();

    let inner: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_A),
        link_a,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_a,
        SRSPP_BUF,
    )
    .unwrap();
    let toggle = Arc::new(AtomicBool::new(false));
    let mut sender =
        SrsppDtnSender::new(inner, ToggleReachable(toggle.clone()), SAT_A, 16);
    let mut receiver: SrsppReceiver<
        _,
        ReceiverMachine<SRSPP_WIN, SRSPP_BUF, SRSPP_REASM>,
        SRSPP_MTU,
    > = SrsppReceiver::new(receiver_config(SAT_B), SAT_A, link_b, TICKS_PER_SEC);

    sender.send(SAT_B, &b"queued-1"[..]).await.unwrap();
    sender.send(SAT_B, &b"queued-2"[..]).await.unwrap();
    sender.send(SAT_B, &b"queued-3"[..]).await.unwrap();
    assert_eq!(sender.pending(SAT_B), 3);

    toggle.store(true, Ordering::SeqCst);

    let send = async {
        sender.send(SAT_B, &b"final"[..]).await.unwrap();
        sender.flush().await.unwrap();
        assert_eq!(sender.pending(SAT_B), 0, "store should be drained");
    };
    let recv = async {
        let mut buf = [0u8; 64];
        let mut out = Vec::new();
        for _ in 0..4 {
            let len = receiver.recv(&mut buf).await.unwrap();
            out.push(buf[..len].to_vec());
        }
        out
    };
    let ((), got) = tokio::join!(send, recv);
    assert_eq!(got[0], b"queued-1");
    assert_eq!(got[1], b"queued-2");
    assert_eq!(got[2], b"queued-3");
    assert_eq!(got[3], b"final");
}

/// Per-target capacity caps queue length — oldest entries get
/// evicted when the cap is exceeded.
#[tokio::test]
async fn dtn_per_target_capacity_caps_queue() {
    let (link_a, _link_b) = MockLink::pair();
    let pool_a = pool();

    let inner: SrsppSender<_, _, _, SRSPP_WIN, SRSPP_MTU> = SrsppSender::new(
        sender_config(SAT_A),
        link_a,
        FixedRto::new(RTO_MS),
        TICKS_PER_SEC,
        &pool_a,
        SRSPP_BUF,
    )
    .unwrap();
    let toggle = Arc::new(AtomicBool::new(false));
    let mut sender =
        SrsppDtnSender::new(inner, ToggleReachable(toggle.clone()), SAT_A, 2);

    sender.send(SAT_B, &b"a"[..]).await.unwrap();
    sender.send(SAT_B, &b"b"[..]).await.unwrap();
    sender.send(SAT_B, &b"c"[..]).await.unwrap();
    assert_eq!(sender.pending(SAT_B), 2, "capacity should cap at 2");
}
