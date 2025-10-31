//! Software Bus message transmission APIs.
use crate::cfe::sb::msg::MessageRef;
use crate::error::Result;
use crate::ffi;
use crate::status::check;

/// A handle to the cFE Software Bus for message transmission.
pub struct SoftwareBus;

impl SoftwareBus {
    /// Transmits a message by copying its contents into the Software Bus.
    pub fn transmit_msg(msg: MessageRef, is_origination: bool) -> Result<()> {
        check(unsafe {
            ffi::CFE_SB_TransmitMsg(msg.as_slice().as_ptr() as *const _, is_origination)
        })?;
        Ok(())
    }
}
