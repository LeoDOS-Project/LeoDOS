use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{RwLock, mpsc, oneshot};

use crate::isl_router::SatAddr;
use crate::messages::*;

pub struct GroundStation {
    gateway: SatAddr,
    tx: mpsc::Sender<Packet>,
    req_id: AtomicU64,
    pending: Arc<RwLock<HashMap<u64, oneshot::Sender<String>>>>,
}

impl GroundStation {
    pub fn new(gateway: SatAddr, tx: mpsc::Sender<Packet>) -> Self {
        Self {
            gateway,
            tx,
            req_id: AtomicU64::new(0),
            pending: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn handle_downlink(&self, mut rx: mpsc::Receiver<Packet>) {
        while let Some(pkt) = rx.recv().await {
            if let Packet::Forward {
                msg: Msg::Downlink { req_id, data },
                ..
            } = pkt
            {
                println!("[Ground] Received downlink for req {}", req_id);
                if let Some(tx) = self.pending.write().await.remove(&req_id) {
                    let _ = tx.send(data);
                }
            }
        }
    }

    pub async fn request(&self, bbox: BoundingBox) -> Option<String> {
        let req_id = self.req_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending.write().await.insert(req_id, tx);

        println!("[Ground] Uplink to {} for req {}", self.gateway, req_id);

        self.tx
            .send(Packet::Forward {
                dest: self.gateway,
                msg: Msg::Uplink { req_id, bbox },
                hops: 0,
            })
            .await
            .ok()?;

        rx.await.ok()
    }
}
