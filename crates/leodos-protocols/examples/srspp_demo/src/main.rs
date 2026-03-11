use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{Duration, sleep};

use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::network::{NetworkWriter, NetworkReader};
use leodos_protocols::transport::srspp::api::tokio::{SrsppReceiver, SrsppSender};
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::rto::FixedRto;

const MTU: usize = 512;

struct SimulatedLink {
    tx: mpsc::Sender<Vec<u8>>,
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
}

impl SimulatedLink {
    fn new_pair() -> (Self, Self) {
        let (tx_a, rx_a) = mpsc::channel(64);
        let (tx_b, rx_b) = mpsc::channel(64);
        
        let link_a = SimulatedLink {
            tx: tx_b,
            rx: Arc::new(Mutex::new(rx_a)),
        };
        let link_b = SimulatedLink {
            tx: tx_a,
            rx: Arc::new(Mutex::new(rx_b)),
        };
        
        (link_a, link_b)
    }
}

impl NetworkWriter for SimulatedLink {
    type Error = std::io::Error;

    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.tx.send(data.to_vec()).await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed"))
    }
}

impl NetworkReader for SimulatedLink {
    type Error = std::io::Error;

    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
        let mut rx = self.rx.lock().await;
        if let Some(data) = rx.recv().await {
            let len = data.len().min(buffer.len());
            buffer[..len].copy_from_slice(&data[..len]);
            Ok(len)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "channel closed"))
        }
    }
}

fn sender_config(addr: Address) -> SenderConfig {
    SenderConfig {
        source_address: addr,
        apid: Apid::new(0x50).unwrap(),
        function_code: 0,
        message_id: 0,
        action_code: 0,
        rto_ticks: 1000,
        max_retransmits: 3,
        header_overhead: leodos_protocols::transport::srspp::packet::SrsppDataPacket::HEADER_SIZE,
    }
}

fn receiver_config(addr: Address) -> ReceiverConfig {
    ReceiverConfig {
        local_address: addr,
        apid: Apid::new(0x50).unwrap(),
        function_code: 0,
        message_id: 0,
        action_code: 0,
        immediate_ack: true,
        ack_delay_ticks: 100,
        progress_timeout_ticks: None,
    }
}

#[tokio::main]
async fn main() {
    println!("=== SRSPP Demo ===\n");

    let sender_addr = Address::satellite(0, 1);
    let receiver_addr = Address::satellite(2, 3);

    println!("Sender:   {:?}", sender_addr);
    println!("Receiver: {:?}", receiver_addr);
    println!();

    let (sender_link, receiver_link) = SimulatedLink::new_pair();

    let mut sender: SrsppSender<_, _, 8, 4096, MTU> =
        SrsppSender::new(sender_config(sender_addr), sender_link, FixedRto::new(1000), 1000);

    let mut receiver: SrsppReceiver<_, 8, 4096, MTU, 8192> =
        SrsppReceiver::new(receiver_config(receiver_addr), sender_addr, receiver_link, 1000);

    let send_task = tokio::spawn(async move {
        for i in 0..5 {
            let msg = format!("Hello #{} from {:?}", i, sender_addr);
            println!("[Sender] Sending: {}", msg);
            if let Err(e) = sender.send(receiver_addr, msg.as_bytes()).await {
                eprintln!("[Sender] Error: {:?}", e);
                break;
            }
            sleep(Duration::from_millis(200)).await;
        }
        println!("[Sender] Flushing...");
        let _ = sender.flush().await;
        println!("[Sender] Done");
    });

    let recv_task = tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        for _ in 0..5 {
            match receiver.recv(&mut buf).await {
                Ok(len) => {
                    let msg = String::from_utf8_lossy(&buf[..len]);
                    println!("[Receiver] Got: {}", msg);
                }
                Err(e) => {
                    eprintln!("[Receiver] Error: {:?}", e);
                    break;
                }
            }
        }
        println!("[Receiver] Done");
    });

    let _ = tokio::join!(send_task, recv_task);

    println!("\n=== Demo Complete ===");
}
