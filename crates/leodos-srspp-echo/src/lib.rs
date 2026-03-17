#![no_std]

use leodos_libcfs::cfe::evs::event;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::pipe::Pipe;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::error::Error;
use leodos_libcfs::error::Result;
use leodos_libcfs::runtime::Runtime;
use leodos_libcfs::runtime::join::join;
use leodos_protocols::network::NetworkLayer;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppReceiver;
use leodos_protocols::transport::srspp::api::cfs::SrsppRxHandle;
use leodos_protocols::transport::srspp::api::cfs::SrsppSender;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
use leodos_protocols::transport::srspp::api::cfs::TransportError;
use leodos_protocols::transport::srspp::dtn::AlwaysReachable;
use leodos_protocols::transport::srspp::dtn::NoStore;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverMachine;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::rto::FixedRto;

type SrsppError = TransportError<Error>;
type RxHandle<'a> = SrsppRxHandle<'a, Error, ReceiverMachine<WIN, BUF, REASM>, MAX_STREAMS>;
type TxHandle<'a> = SrsppTxHandle<'a, Error, NoStore, AlwaysReachable, WIN, BUF, MTU>;

const RX_APID: u16 = 0x42;
const TX_APID: u16 = 0x43;
const WIN: usize = 8;
const BUF: usize = 4096;
const MTU: usize = 512;
const REASM: usize = 8192;
const MAX_STREAMS: usize = 4;
const RTO_MS: u32 = 1000;
const MAX_RETRANSMITS: u8 = 3;
const ACK_DELAY_MS: u32 = 100;

const RX_TOPIC_ID: u16 = 0x100;
const TX_TOPIC_ID: u16 = 0x101;

struct SbRxLink {
    pipe: Pipe,
}

impl SbRxLink {
    fn new(topic_id: u16) -> Result<Self> {
        let pipe = Pipe::new("SRSPP_ECHO_RX", 16)?;
        let msg_id = MsgId::from_local_tlm(topic_id);
        pipe.subscribe(msg_id)?;
        Ok(Self { pipe })
    }
}

impl NetworkLayer for SbRxLink {
    type Error = Error;

    async fn send(&mut self, _data: &[u8]) -> core::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn recv(&mut self, buffer: &mut [u8]) -> core::result::Result<usize, Self::Error> {
        self.pipe.recv(buffer).await
    }
}

struct SbTxLink {
    msg_id: MsgId,
}

impl SbTxLink {
    fn new(topic_id: u16) -> Self {
        Self {
            msg_id: MsgId::from_local_tlm(topic_id),
        }
    }
}

impl NetworkLayer for SbTxLink {
    type Error = Error;

    async fn send(&mut self, data: &[u8]) -> core::result::Result<(), Self::Error> {
        let mut buf = SendBuffer::new(data.len())?;
        buf.as_mut_slice().copy_from_slice(data);
        buf.view().init(self.msg_id, data.len())?;
        buf.send(true)
    }

    async fn recv(&mut self, _buffer: &mut [u8]) -> core::result::Result<usize, Self::Error> {
        core::future::pending().await
    }
}

fn rx_config() -> ReceiverConfig {
    ReceiverConfig {
        local_address: Address::ground(1),
        apid: Apid::new(RX_APID).unwrap(),
        function_code: 0,
        message_id: 0,
        action_code: 0,
        immediate_ack: false,
        ack_delay_ticks: ACK_DELAY_MS,
        progress_timeout_ticks: None,
    }
}

fn tx_config() -> SenderConfig {
    SenderConfig {
        source_address: Address::ground(1),
        apid: Apid::new(TX_APID).unwrap(),
        function_code: 0,
        message_id: 0,
        action_code: 0,
        rto_ticks: RTO_MS,
        max_retransmits: MAX_RETRANSMITS,
        header_overhead: leodos_protocols::transport::srspp::packet::SrsppDataPacket::HEADER_SIZE,
    }
}

async fn echo_loop<'a>(
    rx_handle: &mut RxHandle<'a>,
    tx_handle: &mut TxHandle<'a>,
) -> core::result::Result<(), SrsppError> {
    let mut recv_buf = [0u8; REASM];
    loop {
        let (source, len) = rx_handle.recv(&mut recv_buf).await?;
        event::info(1, "Echo: received message").ok();
        tx_handle.send(source, &recv_buf[..len]).await?;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn CFE_ES_Main() {
    Runtime::new().run(async {
        event::register(&[])?;
        event::info(0, "SRSPP Echo starting")?;

        let mut rx_link = SbRxLink::new(RX_TOPIC_ID)?;
        let mut tx_link = SbTxLink::new(TX_TOPIC_ID);

        let receiver: SrsppReceiver<Error, ReceiverMachine<WIN, BUF, REASM>, MAX_STREAMS> =
            SrsppReceiver::new(rx_config());
        let sender = SrsppSender::new(tx_config(), Address::ground(1), NoStore, AlwaysReachable);

        let (mut rx_handle, mut rx_driver) = receiver.split();
        let (mut tx_handle, mut tx_driver) = sender.split(FixedRto::new(RTO_MS));

        let echo_task = echo_loop(&mut rx_handle, &mut tx_handle);
        let rx_task = rx_driver.run::<MTU>(&mut rx_link);
        let tx_task = tx_driver.run(&mut tx_link);

        let _ = join(echo_task, join(rx_task, tx_task)).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
