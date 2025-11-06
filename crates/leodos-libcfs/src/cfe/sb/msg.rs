//! Safe, idiomatic wrappers for the CFE Message Services (CFE_MSG) API.
//!
//! This module provides `MessageRef` and `MessageMut` structs to safely create,
//! access, and modify the headers of CFE Software Bus messages. It operates on
//! Rust byte slices (`&[u8]` and `&mut [u8]`) to prevent common errors
//! associated with raw pointer manipulation.

use crate::cfe::time::SysTime;
use crate::error::{Error, Result};
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;

/// A Command Header.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CmdHeader(pub(crate) ffi::CFE_MSG_CommandHeader_t);

/// A Telemetry Header.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default)]
pub struct TlmHeader(pub(crate) ffi::CFE_MSG_TelemetryHeader_t);

/// A type-safe, zero-cost wrapper for a cFE Software Bus Message ID.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct MsgId(pub(crate) ffi::CFE_SB_MsgId_t);

impl MsgId {
    /// Returns the raw underlying `CFE_SB_MsgId_t`.
    pub fn value(&self) -> u32 {
        self.0.Value
    }
}

impl PartialEq for MsgId {
    fn eq(&self, other: &Self) -> bool {
        self.0.Value == other.0.Value
    }
}
impl Eq for MsgId {}

impl MsgId {
    /// Checks if the message ID is numerically within the valid mission-defined range.
    ///
    /// # C-API Mapping
    /// This is a safe Rust implementation of the C function `CFE_SB_IsValidMsgId`.
    pub fn is_valid(&self) -> bool {
        // Per the CFE_SB_IsValidMsgId logic, a valid ID is non-zero and within the platform-defined range.
        self.0.Value != 0 && self.0.Value <= ffi::CFE_PLATFORM_SB_HIGHEST_VALID_MSGID
    }

    /// Creates a command `MsgId` from a mission-defined topic ID and a CPU instance number.
    pub fn from_cmd(topic_id: u16, instance_num: u16) -> Self {
        Self(ffi::CFE_SB_MsgId_t {
            Value: unsafe { ffi::CFE_SB_CmdTopicIdToMsgId(topic_id, instance_num) },
        })
    }

    /// Creates a telemetry `MsgId` from a mission-defined topic ID and a CPU instance number.
    pub fn from_tlm(topic_id: u16, instance_num: u16) -> Self {
        Self(ffi::CFE_SB_MsgId_t {
            Value: unsafe { ffi::CFE_SB_TlmTopicIdToMsgId(topic_id, instance_num) },
        })
    }

    /// Creates a global command `MsgId` from a mission-defined topic ID.
    pub fn from_global_cmd(topic_id: u16) -> Self {
        Self(ffi::CFE_SB_MsgId_t {
            Value: unsafe { ffi::CFE_SB_GlobalCmdTopicIdToMsgId(topic_id) },
        })
    }

    /// Creates a global telemetry `MsgId` from a mission-defined topic ID.
    pub fn from_global_tlm(topic_id: u16) -> Self {
        Self(ffi::CFE_SB_MsgId_t {
            Value: unsafe { ffi::CFE_SB_GlobalTlmTopicIdToMsgId(topic_id) },
        })
    }

    /// Creates a local command `MsgId` from a mission-defined topic ID for the current CPU.
    pub fn from_local_cmd(topic_id: u16) -> Self {
        Self(ffi::CFE_SB_MsgId_t {
            Value: unsafe { ffi::CFE_SB_LocalCmdTopicIdToMsgId(topic_id) },
        })
    }

    /// Creates a local telemetry `MsgId` from a mission-defined topic ID for the current CPU.
    pub fn from_local_tlm(topic_id: u16) -> Self {
        Self(ffi::CFE_SB_MsgId_t {
            Value: unsafe { ffi::CFE_SB_LocalTlmTopicIdToMsgId(topic_id) },
        })
    }

    /// Gets the message type (Command or Telemetry) from this message ID.
    pub fn get_type(&self) -> Result<MsgType> {
        let mut msg_type = MaybeUninit::uninit();
        check(unsafe { ffi::CFE_MSG_GetTypeFromMsgId(self.0, msg_type.as_mut_ptr()) })?;
        Ok(unsafe { msg_type.assume_init() }.into())
    }
}

/// The type of a cFE message (Command or Telemetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MsgType {
    /// An invalid or unknown message type.
    Invalid = ffi::CFE_MSG_Type_CFE_MSG_Type_Invalid,
    /// A command message.
    Cmd = ffi::CFE_MSG_Type_CFE_MSG_Type_Cmd,
    /// A telemetry message.
    Tlm = ffi::CFE_MSG_Type_CFE_MSG_Type_Tlm,
}

impl From<ffi::CFE_MSG_Type_t> for MsgType {
    fn from(val: ffi::CFE_MSG_Type_t) -> Self {
        match val {
            ffi::CFE_MSG_Type_CFE_MSG_Type_Cmd => MsgType::Cmd,
            ffi::CFE_MSG_Type_CFE_MSG_Type_Tlm => MsgType::Tlm,
            _ => MsgType::Invalid,
        }
    }
}

/// A safe, read-only wrapper around a CFE message byte slice.
///
/// This provides methods to access header fields without needing `unsafe` code
/// or raw pointers in your application.
#[derive(Debug, Copy, Clone)]
pub struct MessageRef<'a> {
    slice: &'a [u8],
}

impl<'a> MessageRef<'a> {
    /// Creates a new `MessageRef` from a byte slice.
    pub fn new(slice: &'a [u8]) -> Self {
        Self { slice }
    }

    /// Returns the underlying byte slice of the entire message.
    pub fn as_slice(&self) -> &'a [u8] {
        self.slice
    }

    /// Gets the header version from the message header.
    pub fn header_version(&self) -> Result<u16> {
        let mut version = 0;
        check(unsafe {
            ffi::CFE_MSG_GetHeaderVersion(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut version,
            )
        })?;
        Ok(version)
    }

    /// Gets the APID from the message header.
    pub fn apid(&self) -> Result<u16> {
        let mut apid = 0;
        check(unsafe {
            ffi::CFE_MSG_GetApId(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut apid,
            )
        })?;
        Ok(apid)
    }

    /// Gets the total size of the message from its header.
    pub fn size(&self) -> Result<usize> {
        let mut size = 0;
        let status = unsafe {
            ffi::CFE_MSG_GetSize(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut size,
            )
        };
        check(status)?;
        Ok(size)
    }

    /// Gets the message ID from the message header.
    pub fn msg_id(&self) -> Result<MsgId> {
        let mut msg_id = MaybeUninit::uninit();
        let status = unsafe {
            ffi::CFE_MSG_GetMsgId(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                msg_id.as_mut_ptr(),
            )
        };
        check(status)?;
        Ok(MsgId(unsafe { msg_id.assume_init() }))
    }

    /// Gets the function code from a command message header.
    ///
    /// Returns an error if the message does not have a secondary command header.
    pub fn fcn_code(&self) -> Result<u16> {
        let mut fcn_code = 0;
        let status = unsafe {
            ffi::CFE_MSG_GetFcnCode(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut fcn_code,
            )
        };
        check(status)?;
        Ok(fcn_code)
    }

    /// Gets the timestamp from a telemetry message header.
    ///
    /// Returns an error if the message does not have a secondary telemetry header.
    pub fn time(&self) -> Result<SysTime> {
        let mut time = MaybeUninit::uninit();
        let status = unsafe {
            ffi::CFE_MSG_GetMsgTime(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                time.as_mut_ptr(),
            )
        };
        check(status)?;
        Ok(SysTime(unsafe { time.assume_init() }))
    }

    /// Gets the sequence count from the message header.
    pub fn sequence_count(&self) -> Result<u16> {
        let mut count = 0;
        let status = unsafe {
            ffi::CFE_MSG_GetSequenceCount(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut count,
            )
        };
        check(status)?;
        Ok(count)
    }

    /// Validates the checksum of a command message.
    ///
    /// Returns `Ok(true)` if the checksum is valid, `Ok(false)` if it is not.
    /// Returns an error if the message does not have a command secondary header.
    pub fn validate_checksum(&self) -> Result<bool> {
        let mut is_valid = false;
        let status = unsafe {
            ffi::CFE_MSG_ValidateChecksum(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut is_valid,
            )
        };
        check(status)?;
        Ok(is_valid)
    }

    /// Gets the message type (Command or Telemetry) from the header.
    pub fn get_type(&self) -> Result<MsgType> {
        let mut msg_type = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetType(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                msg_type.as_mut_ptr(),
            )
        })?;
        Ok(unsafe { msg_type.assume_init() }.into())
    }

    /// Checks if the message header indicates the presence of a secondary header.
    pub fn has_secondary_header(&self) -> Result<bool> {
        let mut has_secondary = false;
        check(unsafe {
            ffi::CFE_MSG_GetHasSecondaryHeader(
                self.slice.as_ptr() as *const ffi::CFE_MSG_Message_t,
                &mut has_secondary,
            )
        })?;
        Ok(has_secondary)
    }

    /// Gets a pointer to the user data portion of the message.
    ///
    /// # C-API Mapping
    /// This is a wrapper for `CFE_SB_GetUserData`.
    ///
    /// # Safety
    /// The caller must ensure the returned pointer is cast to the correct payload struct type.
    pub unsafe fn user_data(&self) -> *mut libc::c_void {
        ffi::CFE_SB_GetUserData(self.slice.as_ptr() as *mut ffi::CFE_MSG_Message_t)
    }

    /// Gets the length of the user data portion of the message.
    pub fn user_data_length(&self) -> usize {
        unsafe { ffi::CFE_SB_GetUserDataLength(self.slice.as_ptr() as *const _) }
    }

    /// Gets the segmentation flag from the message header.
    pub fn segmentation_flag(&self) -> Result<ffi::CFE_MSG_SegmentationFlag_t> {
        let mut flag = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetSegmentationFlag(
                self.as_slice().as_ptr() as *const _,
                flag.as_mut_ptr(),
            )
        })?;
        Ok(unsafe { flag.assume_init() })
    }

    /// Gets the EDS version from the message header.
    pub fn eds_version(&self) -> Result<u16> {
        let mut version = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetEDSVersion(self.as_slice().as_ptr() as *const _, version.as_mut_ptr())
        })?;
        Ok(unsafe { version.assume_init() })
    }

    /// Gets the endianness indicator from the message header.
    pub fn endian(&self) -> Result<ffi::CFE_MSG_Endian_t> {
        let mut endian = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetEndian(self.as_slice().as_ptr() as *const _, endian.as_mut_ptr())
        })?;
        Ok(unsafe { endian.assume_init() })
    }

    /// Gets the playback flag from the message header.
    pub fn playback_flag(&self) -> Result<ffi::CFE_MSG_PlaybackFlag_t> {
        let mut flag = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetPlaybackFlag(self.as_slice().as_ptr() as *const _, flag.as_mut_ptr())
        })?;
        Ok(unsafe { flag.assume_init() })
    }

    /// Gets the subsystem ID from the message header.
    pub fn subsystem(&self) -> Result<u16> {
        let mut subsystem = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetSubsystem(self.as_slice().as_ptr() as *const _, subsystem.as_mut_ptr())
        })?;
        Ok(unsafe { subsystem.assume_init() })
    }

    /// Gets the system ID from the message header.
    pub fn system(&self) -> Result<u16> {
        let mut system = MaybeUninit::uninit();
        check(unsafe {
            ffi::CFE_MSG_GetSystem(self.as_slice().as_ptr() as *const _, system.as_mut_ptr())
        })?;
        Ok(unsafe { system.assume_init() })
    }
}

/// A safe, writeable wrapper around a CFE message byte slice.
///
/// This provides methods to initialize and modify header fields. It is typically
/// used with a `SendBuffer` from the `sb` module.
#[derive(Debug)]
pub struct MessageMut<'a> {
    pub(crate) slice: &'a mut [u8],
}

impl<'a> MessageMut<'a> {
    /// Returns the underlying byte slice of the entire message.
    pub fn as_slice(&self) -> &[u8] {
        self.slice
    }

    /// Returns the underlying mutable byte slice of the entire message.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.slice
    }

    /// Transmits the message in this buffer.
    pub fn send(self, is_origination: bool) -> Result<()> {
        let status = unsafe {
            ffi::CFE_SB_TransmitBuffer(self.slice.as_mut_ptr() as *mut _, is_origination)
        };

        if status == ffi::CFE_SUCCESS {
            Ok(())
        } else {
            Err(Error::from(status))
        }
    }

    /// Initializes a message header within the buffer.
    ///
    /// This routine zeroes the buffer up to `size`, sets default header fields,
    /// and then populates the message ID and length fields.
    pub fn init(&mut self, msg_id: MsgId, size: usize) -> Result<()> {
        if size > self.slice.len() {
            return Err(Error::OsErrInvalidSize);
        }
        let status = unsafe {
            ffi::CFE_MSG_Init(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                msg_id.0,
                size,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Sets the total size of the message in its header.
    pub fn set_size(&mut self, size: usize) -> Result<()> {
        if size > self.slice.len() {
            return Err(Error::OsErrInvalidSize);
        }
        let status = unsafe {
            ffi::CFE_MSG_SetSize(self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t, size)
        };
        check(status)?;
        Ok(())
    }

    /// Sets the message ID in the message header.
    pub fn set_msg_id(&mut self, msg_id: MsgId) -> Result<()> {
        let status = unsafe {
            ffi::CFE_MSG_SetMsgId(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                msg_id.0,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Sets the header version in the message header.
    pub fn set_header_version(&mut self, version: u16) -> Result<()> {
        check(unsafe {
            ffi::CFE_MSG_SetHeaderVersion(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                version,
            )
        })?;
        Ok(())
    }

    /// Sets the APID in the message header.
    pub fn set_apid(&mut self, apid: u16) -> Result<()> {
        check(unsafe {
            ffi::CFE_MSG_SetApId(self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t, apid)
        })?;
        Ok(())
    }

    /// Sets the function code in a command message header.
    ///
    /// Returns an error if the message does not have a secondary command header.
    pub fn set_fcn_code(&mut self, fcn_code: u16) -> Result<()> {
        let status = unsafe {
            ffi::CFE_MSG_SetFcnCode(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                fcn_code,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Sets the timestamp in a telemetry message header.
    ///
    /// Returns an error if the message does not have a secondary telemetry header.
    pub fn set_time(&mut self, new_time: SysTime) -> Result<()> {
        let status = unsafe {
            ffi::CFE_MSG_SetMsgTime(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                new_time.0,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Sets the sequence count in the message header.
    pub fn set_sequence_count(&mut self, count: u16) -> Result<()> {
        let status = unsafe {
            ffi::CFE_MSG_SetSequenceCount(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                count,
            )
        };
        check(status)?;
        Ok(())
    }

    /// Calculates and sets the checksum field in a command message header.
    ///
    /// Returns an error if the message does not have a command secondary header.
    pub fn generate_checksum(&mut self) -> Result<()> {
        let status = unsafe {
            ffi::CFE_MSG_GenerateChecksum(self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t)
        };
        check(status)?;
        Ok(())
    }

    /// Sets the message type (Command or Telemetry) in the header.
    pub fn set_type(&mut self, msg_type: MsgType) -> Result<()> {
        check(unsafe {
            ffi::CFE_MSG_SetType(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                msg_type as ffi::CFE_MSG_Type_t,
            )
        })?;
        Ok(())
    }

    /// Sets the flag indicating whether a secondary header is present.
    pub fn set_has_secondary_header(&mut self, has_secondary: bool) -> Result<()> {
        check(unsafe {
            ffi::CFE_MSG_SetHasSecondaryHeader(
                self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t,
                has_secondary,
            )
        })?;
        Ok(())
    }

    /// Gets a raw pointer to the user data portion of the message.
    /// # C-API Mapping
    /// This is a wrapper for `CFE_SB_GetUserData`.
    ///
    /// # Safety
    /// The caller must ensure the returned pointer is cast to the correct payload struct type
    /// and that the size of the payload struct does not exceed the user data length.
    pub unsafe fn user_data(&mut self) -> *mut libc::c_void {
        ffi::CFE_SB_GetUserData(self.slice.as_mut_ptr() as *mut ffi::CFE_MSG_Message_t)
    }

    /// Returns a mutable reference to the message payload, interpreted as type `P`.
    ///
    /// This is the primary safe method for accessing the payload of a message.
    /// It performs a size check to ensure the payload type `P` fits within the
    /// available user data area of the message buffer, preventing buffer overruns.
    ///
    /// # Errors
    ///
    /// Returns `Error::StatusWrongMsgLength` if `size_of::<P>()` is larger
    /// than the available user data length in the message buffer.
    pub fn payload<P: Sized>(&mut self) -> Result<&mut P> {
        if core::mem::size_of::<P>() > self.user_data_length() {
            return Err(Error::CfeStatusWrongMsgLength);
        }
        // This is safe because:
        // 1. We have a mutable reference to the slice, ensuring exclusive access.
        // 2. We have checked that the size of P fits within the user data area.
        // 3. user_data() provides a correctly aligned pointer to the payload.
        unsafe {
            let payload_ptr = self.user_data() as *mut P;
            Ok(&mut *payload_ptr)
        }
    }

    /// Gets the length of the user data portion of the message.
    pub fn user_data_length(&self) -> usize {
        unsafe { ffi::CFE_SB_GetUserDataLength(self.slice.as_ptr() as *const _) }
    }

    /// Sets the length of the user data portion of the message.
    pub fn set_user_data_length(&mut self, length: usize) {
        unsafe { ffi::CFE_SB_SetUserDataLength(self.slice.as_mut_ptr() as *mut _, length) }
    }

    /// Sets the time field in the message header with the current spacecraft time.
    pub fn timestamp(&mut self) {
        unsafe { ffi::CFE_SB_TimeStampMsg(self.slice.as_mut_ptr() as *mut _) }
    }

    /// Sets the segmentation flag in the message header.
    pub fn set_segmentation_flag(&mut self, flag: ffi::CFE_MSG_SegmentationFlag_t) -> Result<()> {
        check(unsafe {
            ffi::CFE_MSG_SetSegmentationFlag(self.slice.as_mut_ptr() as *mut _, flag)
        })?;
        Ok(())
    }

    /// Sets the EDS version in the message header.
    pub fn set_eds_version(&mut self, version: u16) -> Result<()> {
        check(unsafe { ffi::CFE_MSG_SetEDSVersion(self.slice.as_mut_ptr() as *mut _, version) })?;
        Ok(())
    }

    /// Sets the endianness indicator in the message header.
    pub fn set_endian(&mut self, endian: ffi::CFE_MSG_Endian_t) -> Result<()> {
        check(unsafe { ffi::CFE_MSG_SetEndian(self.slice.as_mut_ptr() as *mut _, endian) })?;
        Ok(())
    }

    /// Sets the playback flag in the message header.
    pub fn set_playback_flag(&mut self, flag: ffi::CFE_MSG_PlaybackFlag_t) -> Result<()> {
        check(unsafe { ffi::CFE_MSG_SetPlaybackFlag(self.slice.as_mut_ptr() as *mut _, flag) })?;
        Ok(())
    }

    /// Sets the subsystem ID in the message header.
    pub fn set_subsystem(&mut self, subsystem: u16) -> Result<()> {
        check(unsafe { ffi::CFE_MSG_SetSubsystem(self.slice.as_mut_ptr() as *mut _, subsystem) })?;
        Ok(())
    }

    /// Sets the system ID in the message header.
    pub fn set_system(&mut self, system: u16) -> Result<()> {
        check(unsafe { ffi::CFE_MSG_SetSystem(self.slice.as_mut_ptr() as *mut _, system) })?;
        Ok(())
    }
}

/// Copies a Rust string slice into a fixed-size C-style char array within a message.
///
/// This is a safe wrapper around `CFE_SB_MessageStringSet`. It handles truncation
/// and null-padding correctly.
///
/// # Arguments
/// * `dest`: A mutable slice representing the fixed-size char array in the message.
/// * `src`: The Rust string slice to copy from.
pub fn message_string_set(dest: &mut [i8], src: &str) -> Result<usize> {
    let bytes_copied = unsafe {
        ffi::CFE_SB_MessageStringSet(
            dest.as_mut_ptr() as *mut libc::c_char,
            src.as_ptr() as *const libc::c_char,
            dest.len(),
            src.len(),
        )
    };
    if bytes_copied < 0 {
        Err(Error::from(bytes_copied))
    } else {
        Ok(bytes_copied as usize)
    }
}

/// Copies a string from a fixed-size C-style char array within a message to a Rust buffer.
///
/// This is a safe wrapper around `CFE_SB_MessageStringGet`. It correctly handles
/// unterminated strings from the source buffer and ensures the destination is null-terminated.
///
/// # Arguments
/// * `dest`: The mutable Rust byte buffer to copy the string into.
/// * `src`: The fixed-size C-style `i8` array from the message.
/// * `default_src`: An optional default string to use if the source string is empty.
pub fn message_string_get<'a>(
    dest: &'a mut [u8],
    src: &[i8],
    default_src: Option<&str>,
) -> Result<&'a str> {
    let default_ptr = default_src.map_or(core::ptr::null(), |s| s.as_ptr() as *const libc::c_char);
    let bytes_copied = unsafe {
        ffi::CFE_SB_MessageStringGet(
            dest.as_mut_ptr() as *mut libc::c_char,
            src.as_ptr() as *const libc::c_char,
            default_ptr as *const libc::c_char,
            dest.len(),
            src.len(),
        )
    };
    if bytes_copied < 0 {
        return Err(Error::from(bytes_copied));
    }
    core::str::from_utf8(&dest[..bytes_copied as usize]).map_err(|_| Error::InvalidString)
}

/// Gets the next sequence count value, handling rollovers correctly.
pub fn get_next_sequence_count(current_count: u16) -> u16 {
    unsafe { ffi::CFE_MSG_GetNextSequenceCount(current_count) }
}
