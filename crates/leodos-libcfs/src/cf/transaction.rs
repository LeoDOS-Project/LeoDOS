//! CFDP Transaction types and functions.

use crate::ffi;
use crate::cf::crc::Crc;
use crate::cf::pdu::LogicalPduBuffer;
use crate::cf::timer::Timer;
use crate::cf::types::{CfdpClass, TxnState, TxnStatus};
use crate::error::Error;
use crate::status::{self, Status};

/// CFDP Transaction state object.
#[repr(transparent)]
pub struct Transaction(pub(crate) ffi::CF_Transaction_t);

impl Transaction {
    /// Returns the current transaction state.
    pub fn state(&self) -> TxnState {
        TxnState::try_from(self.0.state).unwrap_or(TxnState::Undef)
    }

    /// Returns true if the transaction is using reliable (class 2) mode.
    pub fn is_reliable_mode(&self) -> bool {
        self.0.reliable_mode
    }

    /// Returns the channel number for this transaction.
    pub fn chan_num(&self) -> u8 {
        self.0.chan_num
    }

    /// Returns the priority of this transaction.
    pub fn priority(&self) -> u8 {
        self.0.priority
    }

    /// Returns the file size.
    pub fn file_size(&self) -> u32 {
        self.0.fsize
    }

    /// Returns the current file offset.
    pub fn file_offset(&self) -> u32 {
        self.0.foffs
    }

    /// Returns a reference to the CRC state.
    pub fn crc(&self) -> &Crc {
        unsafe { &*(&self.0.crc as *const ffi::CF_Crc_t as *const Crc) }
    }

    /// Returns a mutable reference to the CRC state.
    pub fn crc_mut(&mut self) -> &mut Crc {
        unsafe { &mut *(&mut self.0.crc as *mut ffi::CF_Crc_t as *mut Crc) }
    }

    /// Returns a reference to the inactivity timer.
    pub fn inactivity_timer(&self) -> &Timer {
        unsafe { &*(&self.0.inactivity_timer as *const ffi::CF_Timer_t as *const Timer) }
    }

    /// Returns a mutable reference to the inactivity timer.
    pub fn inactivity_timer_mut(&mut self) -> &mut Timer {
        unsafe { &mut *(&mut self.0.inactivity_timer as *mut ffi::CF_Timer_t as *mut Timer) }
    }

    /// Returns a reference to the ACK timer.
    pub fn ack_timer(&self) -> &Timer {
        unsafe { &*(&self.0.ack_timer as *const ffi::CF_Timer_t as *const Timer) }
    }

    /// Returns a mutable reference to the ACK timer.
    pub fn ack_timer_mut(&mut self) -> &mut Timer {
        unsafe { &mut *(&mut self.0.ack_timer as *mut ffi::CF_Timer_t as *mut Timer) }
    }

    /// Gets the transaction status code.
    pub fn get_status(&self) -> TxnStatus {
        let status = unsafe { ffi::CF_CFDP_GetTxnStatus(&self.0) };
        TxnStatus::try_from(status).unwrap_or(TxnStatus::Undefined)
    }

    /// Sets the transaction status code.
    pub fn set_status(&mut self, status: TxnStatus) {
        unsafe { ffi::CF_CFDP_SetTxnStatus(&mut self.0, status as ffi::CF_TxnStatus_t) }
    }

    /// Returns true if the transaction is in a good state (no errors).
    pub fn is_ok(&self) -> bool {
        let status = unsafe { ffi::CF_CFDP_TxnIsOK(&self.0) };
        status == ffi::CF_TxnStatus_t_CF_TxnStatus_NO_ERROR
    }

    /// Finishes the transaction and puts it into holdover state.
    pub fn finish(&mut self, keep_history: bool) {
        unsafe { ffi::CF_CFDP_FinishTransaction(&mut self.0, keep_history) }
    }

    /// Recycles the transaction, returning resources to the free list.
    pub fn recycle(&mut self) {
        unsafe { ffi::CF_CFDP_RecycleTransaction(&mut self.0) }
    }

    /// Cancels the transaction.
    pub fn cancel(&mut self) {
        unsafe { ffi::CF_CFDP_CancelTransaction(&mut self.0) }
    }

    /// Arms the ACK timer.
    pub fn arm_ack_timer(&mut self) {
        unsafe { ffi::CF_CFDP_ArmAckTimer(&mut self.0) }
    }

    /// Arms the inactivity timer.
    pub fn arm_inact_timer(&mut self) {
        unsafe { ffi::CF_CFDP_ArmInactTimer(&mut self.0) }
    }

    /// Allocates a chunk list for the transaction.
    pub fn alloc_chunk_list(&mut self) {
        unsafe { ffi::CF_CFDP_AllocChunkList(&mut self.0) }
    }

    /// Increments ack/nak counter and checks limit.
    pub fn check_ack_nak_count(&mut self, counter: &mut u8) -> bool {
        unsafe { ffi::CF_CFDP_CheckAckNakCount(&mut self.0, counter) }
    }

    /// Completes tick processing on the transaction.
    pub fn complete_tick(&mut self) {
        unsafe { ffi::CF_CFDP_CompleteTick(&mut self.0) }
    }

    /// Sends an end-of-transaction packet.
    pub fn send_eot_pkt(&mut self) {
        unsafe { ffi::CF_CFDP_SendEotPkt(&mut self.0) }
    }

    /// Sends a metadata PDU.
    pub fn send_md(&mut self) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_SendMd(&mut self.0) })
    }

    /// Sends an EOF PDU.
    pub fn send_eof(&mut self) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_SendEof(&mut self.0) })
    }

    /// Sends a FIN PDU.
    pub fn send_fin(&mut self) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_SendFin(&mut self.0) })
    }

    /// Sends an ACK PDU for the specified directive code.
    pub fn send_ack(&mut self, dir_code: u8) -> Result<Status, Error> {
        status::check(unsafe {
            ffi::CF_CFDP_SendAck(&mut self.0, dir_code as ffi::CF_CFDP_FileDirective_t)
        })
    }

    /// Sends file data PDU.
    pub fn send_fd(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_SendFd(&mut self.0, &mut ph.0) })
    }

    /// Sends a NAK PDU.
    pub fn send_nak(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_SendNak(&mut self.0, &mut ph.0) }
    }

    /// Receives and processes a metadata PDU.
    pub fn recv_md(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_RecvMd(&mut self.0, &mut ph.0) })
    }

    /// Receives and processes a file data PDU.
    pub fn recv_fd(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_RecvFd(&mut self.0, &mut ph.0) })
    }

    /// Receives and processes an EOF PDU.
    pub fn recv_eof(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_RecvEof(&mut self.0, &mut ph.0) })
    }

    /// Receives and processes an ACK PDU.
    pub fn recv_ack(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_RecvAck(&mut self.0, &mut ph.0) })
    }

    /// Receives and processes a FIN PDU.
    pub fn recv_fin(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_RecvFin(&mut self.0, &mut ph.0) })
    }

    /// Receives and processes a NAK PDU.
    pub fn recv_nak(&mut self, ph: &mut LogicalPduBuffer) -> Result<Status, Error> {
        status::check(unsafe { ffi::CF_CFDP_RecvNak(&mut self.0, &mut ph.0) })
    }

    /// Dispatches a received PDU to its handler.
    pub fn dispatch_recv(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_DispatchRecv(&mut self.0, &mut ph.0) }
    }

    /// Receive state function to ignore a packet.
    pub fn recv_drop(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_RecvDrop(&mut self.0, &mut ph.0) }
    }

    /// Receive state function during holdover period.
    pub fn recv_hold(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_RecvHold(&mut self.0, &mut ph.0) }
    }

    /// Receive state function to process new rx transaction.
    pub fn recv_init(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_RecvInit(&mut self.0, &mut ph.0) }
    }

    /// Sets up an RX transaction based on received PDU.
    pub fn setup_rx(&mut self, ph: &mut LogicalPduBuffer) {
        unsafe { ffi::CF_CFDP_SetupRxTransaction(&mut self.0, &mut ph.0) }
    }

    /// Sets up a TX transaction.
    pub fn setup_tx(&mut self) {
        unsafe { ffi::CF_CFDP_SetupTxTransaction(&mut self.0) }
    }

    /// Initializes the transaction for transmitting a file.
    pub fn init_tx_file(&mut self, cfdp_class: CfdpClass, keep: u8, chan: u8, priority: u8) {
        unsafe {
            ffi::CF_CFDP_InitTxnTxFile(&mut self.0, cfdp_class.into(), keep, chan, priority)
        }
    }

    /// Tick processor to send new file data.
    pub fn tick_new_data(&mut self) {
        unsafe { ffi::CF_CFDP_S_Tick_NewData(&mut self.0) }
    }

    #[allow(dead_code)]
    pub(crate) fn as_raw_ptr(&self) -> *const ffi::CF_Transaction_t {
        &self.0
    }

    #[allow(dead_code)]
    pub(crate) fn as_raw_mut_ptr(&mut self) -> *mut ffi::CF_Transaction_t {
        &mut self.0
    }
}
