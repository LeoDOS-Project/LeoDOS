//! UDP subscriber for the walker-delta ↔ LeoDOS bridge.
//!
//! Listens on [`crate::bridge::TOPOLOGY_PORT`] (or a caller-
//! supplied bind address), decodes incoming state packets, and
//! publishes the latest [`Snapshot`] for the rest of the process
//! to read. Backends (hwlib, PSP) consume the snapshot to provide
//! GPS, link visibility, etc. without modifying cFS app code.
//!
//! Single instance per process — typically owned by the cFS app
//! main and queried by hwlib backends through a `OnceLock`.

use crate::bridge::DecodeError;
use crate::bridge::SatState;
use crate::bridge::decode_state;
use std::io;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

const RECV_BUF_BYTES: usize = 65_536;
const RECV_TIMEOUT: Duration = Duration::from_millis(500);

/// Latest decoded state from walker-delta.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Sequence number from the most recent packet.
    pub seq: u32,
    /// Sim clock (ms since simulation epoch).
    pub sim_time_ms: u64,
    /// Wall clock when the publisher emitted the packet (ms since UNIX epoch).
    pub real_time_ms: u64,
    /// Per-satellite state, ordered as received.
    pub sats: Vec<SatState>,
}

impl Snapshot {
    /// Find the entry for `scid`, if present.
    pub fn sat(&self, scid: u32) -> Option<&SatState> {
        self.sats.iter().find(|s| s.scid.get() == scid)
    }
}

/// Background UDP subscriber. Owns a thread that drains packets
/// into an `Arc<Mutex<Snapshot>>`. Stops when dropped.
pub struct TopologyClient {
    state: Arc<Mutex<Snapshot>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    local_addr: SocketAddr,
}

impl TopologyClient {
    /// Bind to `addr` and start the receive thread.
    pub fn start(addr: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_read_timeout(Some(RECV_TIMEOUT))?;
        let local_addr = socket.local_addr()?;
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let stop = Arc::new(AtomicBool::new(false));

        let thread = {
            let state = Arc::clone(&state);
            let stop = Arc::clone(&stop);
            thread::Builder::new()
                .name("leodos-topology-rx".into())
                .spawn(move || run(socket, state, stop))?
        };

        Ok(Self {
            state,
            stop,
            thread: Some(thread),
            local_addr,
        })
    }

    /// Returns a clone of the current snapshot.
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().expect("topology mutex poisoned").clone()
    }

    /// Returns the address the receive socket is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl Drop for TopologyClient {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

fn run(socket: UdpSocket, state: Arc<Mutex<Snapshot>>, stop: Arc<AtomicBool>) {
    let mut buf = vec![0u8; RECV_BUF_BYTES];
    while !stop.load(Ordering::Relaxed) {
        let n = match socket.recv_from(&mut buf) {
            Ok((n, _)) => n,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) if e.kind() == io::ErrorKind::TimedOut => continue,
            Err(_) => continue,
        };
        match decode_state(&buf[..n]) {
            Ok((header, sats)) => {
                let snapshot = Snapshot {
                    seq: header.seq.get(),
                    sim_time_ms: header.sim_time_ms.get(),
                    real_time_ms: header.real_time_ms.get(),
                    sats: sats.to_vec(),
                };
                if let Ok(mut guard) = state.lock() {
                    *guard = snapshot;
                }
            }
            Err(DecodeError::BadMagic) => continue,
            Err(_) => continue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::SatState;
    use crate::bridge::StateHeader;
    use crate::bridge::encode_state;

    fn send_packet(target: SocketAddr, seq: u32, sats: &[SatState]) {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let header = StateHeader::new(seq, 1_000, 2_000, sats.len() as u16);
        let mut buf = vec![0u8; 32 + sats.len() * 96];
        encode_state(&mut buf, &header, sats);
        socket.send_to(&buf, target).unwrap();
    }

    fn wait_for_seq(client: &TopologyClient, seq: u32) -> Snapshot {
        for _ in 0..100 {
            let snap = client.snapshot();
            if snap.seq == seq && !snap.sats.is_empty() {
                return snap;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("did not receive seq={} within timeout", seq);
    }

    #[test]
    fn decodes_received_packet() {
        let client = TopologyClient::start("127.0.0.1:0".parse().unwrap()).unwrap();
        let target = client.local_addr();

        let sats = [SatState::new(
            42,
            [7_000_000.0, 0.0, 0.0],
            [0.0, 7_500.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
            0b0011,
            0,
        )];
        send_packet(target, 7, &sats);
        let snap = wait_for_seq(&client, 7);

        assert_eq!(snap.seq, 7);
        assert_eq!(snap.sim_time_ms, 1_000);
        assert_eq!(snap.sats.len(), 1);
        assert_eq!(snap.sats[0].scid.get(), 42);
        assert_eq!(snap.sat(42).unwrap().pos_eci_m[0].get(), 7_000_000.0);
        assert!(snap.sat(99).is_none());
    }

    #[test]
    fn ignores_garbage_packets() {
        let client = TopologyClient::start("127.0.0.1:0".parse().unwrap()).unwrap();
        let target = client.local_addr();

        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.send_to(b"not a state packet", target).unwrap();
        thread::sleep(Duration::from_millis(50));
        assert_eq!(client.snapshot().seq, 0);
        assert!(client.snapshot().sats.is_empty());

        let sats = [SatState::new(1, [0.0; 3], [0.0; 3], [1.0, 0.0, 0.0, 0.0], 0, 0)];
        send_packet(target, 5, &sats);
        let snap = wait_for_seq(&client, 5);
        assert_eq!(snap.sats.len(), 1);
    }
}
