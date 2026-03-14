//! CFDP Engine functions for initialization and control.

use crate::cf::transaction::Transaction;
use crate::cf::types::{CfdpClass, TxnState};
use crate::error::CfsError;
use crate::ffi;
use crate::status::{self, Status};
use core::ffi::CStr;

/// Returns the current engine sequence number.
pub fn engine_seq_num() -> u32 {
    unsafe { ffi::CF_AppData.engine.seq_num }
}

/// Finds a transaction by sequence number and source entity ID.
pub fn find_transaction_by_seq(seq_num: u32, src_eid: u32) -> Option<&'static Transaction> {
    let txns = &raw const ffi::CF_AppData;
    let txns = unsafe { &(*txns).engine.transactions };
    txns.iter()
        .find(|t| unsafe {
            !t.history.is_null()
                && (*t.history).seq_num == seq_num
                && (*t.history).src_eid == src_eid
        })
        .map(|t| unsafe { &*(t as *const ffi::CF_Transaction as *const Transaction) })
}

/// Finds a transaction mutably by sequence number and source entity ID.
pub fn find_transaction_by_seq_mut(seq_num: u32, src_eid: u32) -> Option<&'static mut Transaction> {
    let txns = &raw mut ffi::CF_AppData;
    let txns = unsafe { &mut (*txns).engine.transactions };
    txns.iter_mut()
        .find(|t| unsafe {
            !t.history.is_null()
                && (*t.history).seq_num == seq_num
                && (*t.history).src_eid == src_eid
        })
        .map(|t| unsafe { &mut *(t as *mut ffi::CF_Transaction as *mut Transaction) })
}

/// Returns the raw transaction state for a given sequence/entity.
pub fn transaction_state_raw(seq_num: u32, src_eid: u32) -> Option<TxnState> {
    find_transaction_by_seq(seq_num, src_eid).map(|t| t.state())
}

/// Initializes the CFDP engine.
pub fn init() -> Result<Status, CfsError> {
    status::check(unsafe { ffi::CF_CFDP_InitEngine() })
}

/// Cycles the CFDP engine (called once per wakeup).
pub fn cycle() {
    unsafe { ffi::CF_CFDP_CycleEngine() }
}

/// Disables the CFDP engine and resets all state.
pub fn disable() {
    unsafe { ffi::CF_CFDP_DisableEngine() }
}

/// Begins transmit of a file.
///
/// # Arguments
/// * `src_filename` - Local filename
/// * `dst_filename` - Remote filename
/// * `cfdp_class` - Class 1 or Class 2 transfer
/// * `keep` - Whether to keep the local file after completion
/// * `chan` - CF channel number
/// * `priority` - Priority level
/// * `dest_id` - Entity ID of remote receiver
pub fn tx_file(
    src_filename: &CStr,
    dst_filename: &CStr,
    cfdp_class: CfdpClass,
    keep: bool,
    chan: u8,
    priority: u8,
    dest_id: u32,
) -> Result<Status, CfsError> {
    status::check(unsafe {
        ffi::CF_CFDP_TxFile(
            src_filename.as_ptr(),
            dst_filename.as_ptr(),
            cfdp_class.into(),
            keep as u8,
            chan,
            priority,
            dest_id,
        )
    })
}

/// Begins transmit of a directory.
///
/// # Arguments
/// * `src_filename` - Local directory path
/// * `dst_filename` - Remote directory path
/// * `cfdp_class` - Class 1 or Class 2 transfer
/// * `keep` - Whether to keep local files after completion
/// * `chan` - CF channel number
/// * `priority` - Priority level
/// * `dest_id` - Entity ID of remote receiver
pub fn playback_dir(
    src_filename: &CStr,
    dst_filename: &CStr,
    cfdp_class: CfdpClass,
    keep: bool,
    chan: u8,
    priority: u8,
    dest_id: u16,
) -> Result<Status, CfsError> {
    status::check(unsafe {
        ffi::CF_CFDP_PlaybackDir(
            src_filename.as_ptr(),
            dst_filename.as_ptr(),
            cfdp_class.into(),
            keep as u8,
            chan,
            priority,
            dest_id,
        )
    })
}

/// Starts a new RX transaction on the specified channel.
pub fn start_rx_transaction(chan_num: u8) -> Option<&'static mut Transaction> {
    let ptr = unsafe { ffi::CF_CFDP_StartRxTransaction(chan_num) };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *(ptr as *mut Transaction) })
    }
}

/// Initializes the CF application.
pub fn app_init() -> Result<Status, CfsError> {
    status::check(unsafe { ffi::CF_AppInit() })
}

/// CF application main entry point.
pub fn app_main() {
    unsafe { ffi::CF_AppMain() }
}

/// Initializes the CF tables.
pub fn table_init() -> Result<Status, CfsError> {
    status::check(unsafe { ffi::CF_TableInit() })
}

/// Checks and updates CF tables.
pub fn check_tables() {
    unsafe { ffi::CF_CheckTables() }
}

/// Constructs a PDU header for a transaction.
///
/// Returns a pointer to the logical PDU buffer, or null on failure.
pub fn construct_pdu_header(
    txn: &Transaction,
    directive_code: crate::cf::FileDirective,
    src_eid: u32,
    dst_eid: u32,
    towards_sender: bool,
    txn_seq: u32,
    silent: bool,
) -> Option<&'static mut crate::cf::pdu::LogicalPduBuffer> {
    let ptr = unsafe {
        ffi::CF_CFDP_ConstructPduHeader(
            &txn.0,
            directive_code as ffi::CF_CFDP_FileDirective_t,
            src_eid,
            dst_eid,
            towards_sender,
            txn_seq,
            silent,
        )
    };
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { &mut *(ptr as *mut crate::cf::pdu::LogicalPduBuffer) })
    }
}

/// Appends a TLV entry to a TLV list.
pub fn append_tlv(tlv_list: &mut crate::cf::pdu::LogicalTlvList, tlv_type: crate::cf::TlvType) {
    unsafe {
        ffi::CF_CFDP_AppendTlv(&mut tlv_list.0, tlv_type as ffi::CF_CFDP_TlvType_t);
    }
}

/// Receives and processes a PDU header from a channel.
pub fn recv_ph(
    chan_num: u8,
    ph: &mut crate::cf::pdu::LogicalPduBuffer,
) -> Result<Status, CfsError> {
    status::check(unsafe { ffi::CF_CFDP_RecvPh(chan_num, &mut ph.0) })
}

/// Copies a string from an LV structure.
///
/// # Safety
/// The destination buffer must be large enough to hold the LV data.
pub unsafe fn copy_string_from_lv(dst: &mut [u8], src_lv: &crate::cf::pdu::LogicalLv) -> i32 {
    ffi::CF_CFDP_CopyStringFromLV(dst.as_mut_ptr() as *mut i8, dst.len(), &src_lv.0)
}

/// Arms the ACK timer for a transaction.
pub fn arm_ack_timer(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_ArmAckTimer(&mut txn.0) }
}

/// Arms the inactivity timer for a transaction.
pub fn arm_inact_timer(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_ArmInactTimer(&mut txn.0) }
}

/// Handles receiving a PDU on a dropped transaction.
pub fn recv_drop(txn: &mut Transaction, ph: &mut crate::cf::pdu::LogicalPduBuffer) {
    unsafe { ffi::CF_CFDP_RecvDrop(&mut txn.0, &mut ph.0) }
}

/// Handles receiving a PDU on a held transaction.
pub fn recv_hold(txn: &mut Transaction, ph: &mut crate::cf::pdu::LogicalPduBuffer) {
    unsafe { ffi::CF_CFDP_RecvHold(&mut txn.0, &mut ph.0) }
}

/// Initial receive handler for new transactions.
pub fn recv_init(txn: &mut Transaction, ph: &mut crate::cf::pdu::LogicalPduBuffer) {
    unsafe { ffi::CF_CFDP_RecvInit(&mut txn.0, &mut ph.0) }
}

/// Dispatches a received PDU to the appropriate handler.
pub fn dispatch_recv(txn: &mut Transaction, ph: &mut crate::cf::pdu::LogicalPduBuffer) {
    unsafe { ffi::CF_CFDP_DispatchRecv(&mut txn.0, &mut ph.0) }
}

/// Receives a PDU on a channel.
pub fn receive_pdu(chan: &mut super::Channel, ph: &mut crate::cf::pdu::LogicalPduBuffer) {
    unsafe { ffi::CF_CFDP_ReceivePdu(&mut chan.0, &mut ph.0) }
}

/// Sets up a receive transaction.
pub fn setup_rx_transaction(txn: &mut Transaction, ph: &mut crate::cf::pdu::LogicalPduBuffer) {
    unsafe { ffi::CF_CFDP_SetupRxTransaction(&mut txn.0, &mut ph.0) }
}

/// Sets up a transmit transaction.
pub fn setup_tx_transaction(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_SetupTxTransaction(&mut txn.0) }
}

/// Starts the first pending transaction on a channel.
pub fn start_first_pending(chan: &mut super::Channel) -> bool {
    unsafe { ffi::CF_CFDP_StartFirstPending(&mut chan.0) }
}

/// Allocates a chunk list for a transaction.
pub fn alloc_chunk_list(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_AllocChunkList(&mut txn.0) }
}

/// Ticks transactions on a channel.
pub fn tick_transactions(chan: &mut super::Channel) {
    unsafe { ffi::CF_CFDP_TickTransactions(&mut chan.0) }
}

/// Processes a playback directory.
pub fn process_playback_directory(chan: &mut super::Channel, pb: &mut super::Playback) {
    unsafe { ffi::CF_CFDP_ProcessPlaybackDirectory(&mut chan.0, &mut pb.0) }
}

/// Processes polling directories on a channel.
pub fn process_polling_directories(chan: &mut super::Channel) {
    unsafe { ffi::CF_CFDP_ProcessPollingDirectories(&mut chan.0) }
}

/// Sends an EOT (End of Transaction) packet.
pub fn send_eot_pkt(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_SendEotPkt(&mut txn.0) }
}

/// Checks the ACK/NAK count and increments the counter.
pub fn check_ack_nak_count(txn: &mut Transaction, counter: &mut u8) -> bool {
    unsafe { ffi::CF_CFDP_CheckAckNakCount(&mut txn.0, counter) }
}

/// Completes a tick cycle for a transaction.
pub fn complete_tick(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_CompleteTick(&mut txn.0) }
}

/// Sends new data for a sender-side tick.
pub fn s_tick_new_data(txn: &mut Transaction) {
    unsafe { ffi::CF_CFDP_S_Tick_NewData(&mut txn.0) }
}

/// Gets the move target path for a file.
///
/// # Safety
/// The dest_dir and subject_file must be valid C strings.
pub unsafe fn get_move_target<'a>(
    dest_dir: &core::ffi::CStr,
    subject_file: &core::ffi::CStr,
    dest_buf: &'a mut [u8],
) -> Option<&'a core::ffi::CStr> {
    let ptr = ffi::CF_CFDP_GetMoveTarget(
        dest_dir.as_ptr(),
        subject_file.as_ptr(),
        dest_buf.as_mut_ptr() as *mut i8,
        dest_buf.len(),
    );
    if ptr.is_null() {
        None
    } else {
        Some(core::ffi::CStr::from_ptr(ptr))
    }
}

/// Gets the temporary file name for a transaction.
///
/// # Safety
/// The history must be valid.
pub unsafe fn get_temp_name(hist: &super::History, dest_buf: &mut [u8]) {
    ffi::CF_CFDP_GetTempName(&hist.0, dest_buf.as_mut_ptr() as *mut i8, dest_buf.len());
}

/// Initializes the encoder and prepares it to receive a PDU.
///
/// # Safety
/// The msgbuf must point to a valid message buffer.
pub unsafe fn encode_start(
    enc: &mut crate::cf::pdu::EncoderState<'_>,
    msgbuf: *mut core::ffi::c_void,
    ph: &mut crate::cf::pdu::LogicalPduBuffer,
    encap_hdr_size: usize,
    total_size: usize,
) {
    ffi::CF_CFDP_EncodeStart(
        enc.as_raw_mut(),
        msgbuf,
        &mut ph.0,
        encap_hdr_size,
        total_size,
    );
}

/// Initializes the decoder and prepares it to decode a PDU.
///
/// # Safety
/// The msgbuf must point to a valid message buffer.
pub unsafe fn decode_start(
    dec: &mut crate::cf::pdu::DecoderState<'_>,
    msgbuf: *const core::ffi::c_void,
    ph: &mut crate::cf::pdu::LogicalPduBuffer,
    encap_hdr_size: usize,
    total_size: usize,
) {
    ffi::CF_CFDP_DecodeStart(
        dec.as_raw_mut(),
        msgbuf,
        &mut ph.0,
        encap_hdr_size,
        total_size,
    );
}

/// Closes files callback function (for CList traversal).
///
/// This is typically used internally when cleaning up transactions.
#[allow(dead_code)]
pub(crate) fn close_files(
    node: *mut crate::cf::CListNode,
    context: *mut core::ffi::c_void,
) -> crate::cf::CListTraverseStatus {
    let result = unsafe { ffi::CF_CFDP_CloseFiles(node as *mut ffi::CF_CListNode_t, context) };
    if result == ffi::CF_CListTraverse_Status_t_CF_CListTraverse_Status_CONTINUE {
        crate::cf::CListTraverseStatus::Continue
    } else {
        crate::cf::CListTraverseStatus::Exit
    }
}

/// Tick callback function (for CList traversal).
///
/// This is typically used internally during engine tick processing.
#[allow(dead_code)]
pub(crate) fn do_tick(
    node: *mut crate::cf::CListNode,
    context: *mut core::ffi::c_void,
) -> crate::cf::CListTraverseStatus {
    let result = unsafe { ffi::CF_CFDP_DoTick(node as *mut ffi::CF_CListNode_t, context) };
    if result == ffi::CF_CListTraverse_Status_t_CF_CListTraverse_Status_CONTINUE {
        crate::cf::CListTraverseStatus::Continue
    } else {
        crate::cf::CListTraverseStatus::Exit
    }
}
