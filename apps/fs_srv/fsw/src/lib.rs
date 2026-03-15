#![no_std]

use leodos_libcfs::app::App;
use leodos_libcfs::cfe::sb::msg::MsgId;
use leodos_libcfs::cfe::sb::send_buf::SendBuffer;
use leodos_libcfs::err;
use leodos_libcfs::error::CfsError;
use leodos_libcfs::info;
use leodos_libcfs::os::fs::{self, AccessMode, Directory, File, SeekFrom};
use leodos_libcfs::runtime::join::join;
use leodos_libcfs::runtime::Runtime;

use leodos_protocols::datalink::link::cfs::sb::SbDatalink;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::ptp::PointToPoint;
use leodos_protocols::network::spp::Apid;
use leodos_protocols::transport::srspp::api::cfs::SrsppNode;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverConfig;
use leodos_protocols::transport::srspp::machine::sender::SenderConfig;
use leodos_protocols::transport::srspp::packet::SrsppDataPacket;
use leodos_protocols::transport::srspp::rto::FixedRto;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

// ── Constants ───────────────────────────────────────────────

const RTO_MS: u32 = 2000;
const ACK_DELAY_MS: u32 = 100;
const READ_CHUNK: usize = 400;
const MAX_BUF: usize = 4096;
const CFE_MISSION_ES_CMD_TOPICID: u16 = 6;
const CFE_ES_RELOAD_APP_CC: u16 = 7;
const CFE_MISSION_MAX_API_LEN: usize = 20;
const CFE_MISSION_MAX_PATH_LEN: usize = 64;

// ── Request/Response wire formats ───────────────────────────

const OP_LIST: u8 = 0;
const OP_PUT: u8 = 1;
const OP_GET: u8 = 2;
const OP_REMOVE: u8 = 3;
const OP_RENAME: u8 = 4;
const OP_INFO: u8 = 5;
const OP_RELOAD: u8 = 6;

const STATUS_OK: u8 = 0;
const STATUS_ERR: u8 = 1;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct FsRequest {
    op: u8,
    _pad: u8,
    path_len: u16,
    dest_len: u16,
    data_offset: u32,
    data_len: u32,
}

const REQ_HDR_SIZE: usize = core::mem::size_of::<FsRequest>();

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct FsResponse {
    op: u8,
    status: u8,
    _pad: [u8; 2],
    payload_len: u32,
}

const RESP_HDR_SIZE: usize = core::mem::size_of::<FsResponse>();

// ── Helpers ─────────────────────────────────────────────────

fn parse_str(data: &[u8], len: u16) -> Option<&str> {
    let l = len as usize;
    if l > data.len() {
        return None;
    }
    core::str::from_utf8(&data[..l]).ok()
}

fn write_response(
    buf: &mut [u8],
    op: u8,
    status: u8,
    payload: &[u8],
) -> usize {
    let resp = FsResponse {
        op,
        status,
        _pad: [0; 2],
        payload_len: payload.len() as u32,
    };
    let hdr_bytes = unsafe {
        core::slice::from_raw_parts(
            &resp as *const _ as *const u8,
            RESP_HDR_SIZE,
        )
    };
    buf[..RESP_HDR_SIZE].copy_from_slice(hdr_bytes);
    let end = RESP_HDR_SIZE + payload.len();
    if end <= buf.len() {
        buf[RESP_HDR_SIZE..end].copy_from_slice(payload);
    }
    end
}

fn write_error(buf: &mut [u8], op: u8) -> usize {
    write_response(buf, op, STATUS_ERR, &[])
}

// ── Operations ──────────────────────────────────────────────

fn handle_list(
    path: &str,
    resp_buf: &mut [u8],
) -> usize {
    let dir = match Directory::open(path) {
        Ok(d) => d,
        Err(_) => return write_error(resp_buf, OP_LIST),
    };

    let mut listing = [0u8; MAX_BUF - RESP_HDR_SIZE];
    let mut pos = 0;

    for entry in dir {
        if let Ok(name) = entry {
            let name_bytes = name.as_bytes();
            let needed = name_bytes.len() + 1;
            if pos + needed > listing.len() {
                break;
            }
            listing[pos..pos + name_bytes.len()]
                .copy_from_slice(name_bytes);
            listing[pos + name_bytes.len()] = b'\n';
            pos += needed;
        }
    }

    write_response(resp_buf, OP_LIST, STATUS_OK, &listing[..pos])
}

fn handle_put(
    path: &str,
    offset: u32,
    data: &[u8],
    resp_buf: &mut [u8],
) -> usize {
    if data.is_empty() {
        write_response(resp_buf, OP_PUT, STATUS_OK, &[])
    } else {
        let file = if offset == 0 {
            File::create(path)
        } else {
            File::open(path, AccessMode::ReadWrite)
        };
        match file {
            Ok(mut f) => {
                if offset > 0 {
                    if f.seek(SeekFrom::Start(offset)).is_err() {
                        return write_error(resp_buf, OP_PUT);
                    }
                }
                match f.sync_write(data) {
                    Ok(_) => write_response(
                        resp_buf,
                        OP_PUT,
                        STATUS_OK,
                        &[],
                    ),
                    Err(_) => write_error(resp_buf, OP_PUT),
                }
            }
            Err(_) => write_error(resp_buf, OP_PUT),
        }
    }
}

fn handle_remove(
    path: &str,
    resp_buf: &mut [u8],
) -> usize {
    match fs::remove(path) {
        Ok(()) => write_response(resp_buf, OP_REMOVE, STATUS_OK, &[]),
        Err(_) => write_error(resp_buf, OP_REMOVE),
    }
}

fn handle_rename(
    old: &str,
    new: &str,
    resp_buf: &mut [u8],
) -> usize {
    match fs::rename(old, new) {
        Ok(()) => write_response(resp_buf, OP_RENAME, STATUS_OK, &[]),
        Err(_) => write_error(resp_buf, OP_RENAME),
    }
}

fn handle_info(
    path: &str,
    resp_buf: &mut [u8],
) -> usize {
    match fs::stat(path) {
        Ok(st) => {
            let mut info = [0u8; 64];
            let is_dir = if st.is_dir() { b'd' } else { b'-' };
            let size = st.size();

            let mut pos = 0;
            info[pos] = is_dir;
            pos += 1;
            info[pos] = b' ';
            pos += 1;

            let mut num_buf = [0u8; 20];
            let mut n = size;
            let mut num_len = 0;
            if n == 0 {
                num_buf[0] = b'0';
                num_len = 1;
            } else {
                while n > 0 {
                    num_buf[num_len] = b'0' + (n % 10) as u8;
                    n /= 10;
                    num_len += 1;
                }
                num_buf[..num_len].reverse();
            }
            if pos + num_len <= info.len() {
                info[pos..pos + num_len]
                    .copy_from_slice(&num_buf[..num_len]);
                pos += num_len;
            }

            write_response(
                resp_buf,
                OP_INFO,
                STATUS_OK,
                &info[..pos],
            )
        }
        Err(_) => write_error(resp_buf, OP_INFO),
    }
}

fn handle_reload(
    app_name: &str,
    file_path: &str,
    resp_buf: &mut [u8],
) -> usize {
    let result = (|| -> Result<(), CfsError> {
        let cmd_hdr_size = core::mem::size_of::<
            leodos_libcfs::cfe::sb::msg::CmdHeader,
        >();
        let payload_size =
            CFE_MISSION_MAX_API_LEN + CFE_MISSION_MAX_PATH_LEN;
        let total = cmd_hdr_size + payload_size;

        let mut send_buf = SendBuffer::new(total)?;
        {
            let mut msg = send_buf.view();
            let es_cmd_mid =
                MsgId::from_local_cmd(CFE_MISSION_ES_CMD_TOPICID);
            msg.init(es_cmd_mid, total)?;
            msg.set_fcn_code(CFE_ES_RELOAD_APP_CC)?;

            let data = unsafe { msg.user_data() } as *mut u8;
            let payload = unsafe {
                core::slice::from_raw_parts_mut(data, payload_size)
            };
            payload.fill(0);

            let name_bytes = app_name.as_bytes();
            let name_len =
                name_bytes.len().min(CFE_MISSION_MAX_API_LEN - 1);
            payload[..name_len]
                .copy_from_slice(&name_bytes[..name_len]);

            let file_bytes = file_path.as_bytes();
            let file_len = file_bytes
                .len()
                .min(CFE_MISSION_MAX_PATH_LEN - 1);
            payload[CFE_MISSION_MAX_API_LEN
                ..CFE_MISSION_MAX_API_LEN + file_len]
                .copy_from_slice(&file_bytes[..file_len]);

            msg.generate_checksum()?;
        }
        send_buf.send(true)
    })();

    match result {
        Ok(()) => write_response(
            resp_buf,
            OP_RELOAD,
            STATUS_OK,
            &[],
        ),
        Err(_) => write_error(resp_buf, OP_RELOAD),
    }
}

// ── App entry point ─────────────────────────────────────────

#[no_mangle]
pub extern "C" fn FS_SRV_AppMain() {
    Runtime::new().run(async {
        let _app = App::builder()
            .name("FS_SRV")
            .cmd_topic(bindings::FS_SRV_CMD_TOPICID as u16)
            .send_hk_topic(bindings::FS_SRV_SEND_HK_TOPICID as u16)
            .hk_tlm_topic(bindings::FS_SRV_HK_TLM_TOPICID as u16)
            .version("0.1.0")
            .build()?;

        let router_send =
            MsgId::from_local_cmd(bindings::FS_SRV_CMD_TOPICID as u16);
        let router_recv = MsgId::from_local_tlm(
            bindings::FS_SRV_HK_TLM_TOPICID as u16,
        );
        let sb = SbDatalink::new("FS_SB", 8, router_recv, router_send)?;
        let network = PointToPoint::new(sb);

        let apid =
            Apid::new(bindings::FS_SRV_APID as u16).unwrap();
        let local_addr = Address::satellite(0, 1);

        let sender_config = SenderConfig::builder()
            .source_address(local_addr)
            .apid(apid)
            .function_code(0)
            .rto_ticks(RTO_MS)
            .max_retransmits(5)
            .header_overhead(SrsppDataPacket::HEADER_SIZE)
            .build();

        let receiver_config = ReceiverConfig::builder()
            .local_address(local_addr)
            .apid(apid)
            .function_code(0)
            .immediate_ack(false)
            .ack_delay_ticks(ACK_DELAY_MS)
            .progress_timeout_ticks(RTO_MS * 3)
            .build();

        let node: SrsppNode<CfsError> =
            SrsppNode::new(sender_config, receiver_config);
        let (mut rx, mut tx, mut driver) =
            node.split(network, FixedRto::new(RTO_MS));

        let workflow = async {
            let mut recv_buf = [0u8; MAX_BUF];
            let mut resp_buf = [0u8; MAX_BUF];

            info!("FS_SRV ready").ok();

            loop {
                let (source, len) = match rx.recv(&mut recv_buf).await
                {
                    Ok(v) => v,
                    Err(e) => {
                        err!("recv: {}", e).ok();
                        continue;
                    }
                };

                if len < REQ_HDR_SIZE {
                    err!("short request: {} bytes", len).ok();
                    continue;
                }

                let req: FsRequest = unsafe {
                    core::ptr::read_unaligned(
                        recv_buf.as_ptr() as *const FsRequest,
                    )
                };
                let body = &recv_buf[REQ_HDR_SIZE..len];

                let path_len = req.path_len as usize;
                let dest_len = req.dest_len as usize;

                let path = match parse_str(body, req.path_len) {
                    Some(p) => p,
                    None => {
                        err!("bad path").ok();
                        continue;
                    }
                };

                match req.op {
                    OP_LIST => {
                        let n = handle_list(path, &mut resp_buf);
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                    OP_PUT => {
                        let data_start = path_len + dest_len;
                        let data = if data_start < body.len() {
                            &body[data_start..]
                        } else {
                            &[]
                        };
                        let n = handle_put(
                            path,
                            req.data_offset,
                            data,
                            &mut resp_buf,
                        );
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                    OP_GET => {
                        let mut file = match File::open(
                            path,
                            AccessMode::ReadOnly,
                        ) {
                            Ok(f) => f,
                            Err(_) => {
                                let n = write_error(
                                    &mut resp_buf,
                                    OP_GET,
                                );
                                tx.send(source, &resp_buf[..n])
                                    .await
                                    .ok();
                                continue;
                            }
                        };
                        let size = match fs::stat(path) {
                            Ok(s) => s.size(),
                            Err(_) => {
                                let n = write_error(
                                    &mut resp_buf,
                                    OP_GET,
                                );
                                tx.send(source, &resp_buf[..n])
                                    .await
                                    .ok();
                                continue;
                            }
                        };
                        let mut data = [0u8; MAX_BUF];
                        let mut total_read = 0;
                        while total_read < size {
                            let to_read = READ_CHUNK
                                .min(size - total_read)
                                .min(data.len() - RESP_HDR_SIZE);
                            match file.sync_read(
                                &mut data[RESP_HDR_SIZE
                                    ..RESP_HDR_SIZE + to_read],
                            ) {
                                Ok(n) if n > 0 => {
                                    total_read += n;
                                    let resp = FsResponse {
                                        op: OP_GET,
                                        status: STATUS_OK,
                                        _pad: [0; 2],
                                        payload_len: n as u32,
                                    };
                                    let hdr = unsafe {
                                        core::slice::from_raw_parts(
                                            &resp as *const _
                                                as *const u8,
                                            RESP_HDR_SIZE,
                                        )
                                    };
                                    data[..RESP_HDR_SIZE]
                                        .copy_from_slice(hdr);
                                    tx.send(
                                        source,
                                        &data
                                            [..RESP_HDR_SIZE + n],
                                    )
                                    .await
                                    .ok();
                                }
                                _ => break,
                            }
                        }
                    }
                    OP_REMOVE => {
                        let n =
                            handle_remove(path, &mut resp_buf);
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                    OP_RENAME => {
                        let dest = match parse_str(
                            &body[path_len..],
                            req.dest_len,
                        ) {
                            Some(d) => d,
                            None => {
                                err!("bad dest path").ok();
                                continue;
                            }
                        };
                        let n = handle_rename(
                            path,
                            dest,
                            &mut resp_buf,
                        );
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                    OP_INFO => {
                        let n = handle_info(path, &mut resp_buf);
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                    OP_RELOAD => {
                        let dest = match parse_str(
                            &body[path_len..],
                            req.dest_len,
                        ) {
                            Some(d) => d,
                            None => {
                                err!("bad reload path").ok();
                                continue;
                            }
                        };
                        let n = handle_reload(
                            path,
                            dest,
                            &mut resp_buf,
                        );
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                    _ => {
                        err!("unknown op {}", req.op).ok();
                        let n = write_error(&mut resp_buf, req.op);
                        tx.send(source, &resp_buf[..n]).await.ok();
                    }
                }
            }
        };

        let _ = join(workflow, driver.run()).await;

        Ok(())
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
