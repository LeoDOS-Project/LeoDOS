//! System-level queries, reset control, and startup synchronization.

use crate::error::Result;
use crate::ffi;
use crate::status::check;
use core::mem::MaybeUninit;
use core::time::Duration;

/// The type of reset the processor most recently underwent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResetType {
    /// A processor reset, where volatile memory areas may have been preserved.
    Processor = ffi::CFE_PSP_RST_TYPE_PROCESSOR,
    /// A power-on reset, where all memory has been cleared.
    PowerOn = ffi::CFE_PSP_RST_TYPE_POWERON,
    /// An unknown or unhandled reset type.
    Unknown(u32),
}

impl From<u32> for ResetType {
    fn from(val: u32) -> Self {
        match val {
            ffi::CFE_PSP_RST_TYPE_PROCESSOR => Self::Processor,
            ffi::CFE_PSP_RST_TYPE_POWERON => Self::PowerOn,
            other => Self::Unknown(other),
        }
    }
}

impl From<ResetType> for u32 {
    fn from(val: ResetType) -> Self {
        match val {
            ResetType::Processor => ffi::CFE_PSP_RST_TYPE_PROCESSOR,
            ResetType::PowerOn => ffi::CFE_PSP_RST_TYPE_POWERON,
            ResetType::Unknown(other) => other,
        }
    }
}

/// The specific cause of the most recent reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResetSubtype {
    /// Reset caused by a power cycle.
    PowerCycle = ffi::CFE_PSP_RST_SUBTYPE_POWER_CYCLE,
    /// Reset caused by a push button.
    PushButton = ffi::CFE_PSP_RST_SUBTYPE_PUSH_BUTTON,
    /// Reset caused by a hardware special command.
    HwSpecialCommand = ffi::CFE_PSP_RST_SUBTYPE_HW_SPECIAL_COMMAND,
    /// Reset caused by a hardware watchdog timer expiring.
    HwWatchdog = ffi::CFE_PSP_RST_SUBTYPE_HW_WATCHDOG,
    /// Reset caused by a cFE ES Reset command.
    ResetCommand = ffi::CFE_PSP_RST_SUBTYPE_RESET_COMMAND,
    /// Reset caused by a processor exception.
    Exception = ffi::CFE_PSP_RST_SUBTYPE_EXCEPTION,
    /// Reset cause is undefined.
    Undefined = ffi::CFE_PSP_RST_SUBTYPE_UNDEFINED_RESET,
    /// Reset caused by a hardware debugger.
    HwDebug = ffi::CFE_PSP_RST_SUBTYPE_HWDEBUG_RESET,
    /// Reset reverted to a POWERON due to a boot bank switch.
    BankSwitch = ffi::CFE_PSP_RST_SUBTYPE_BANKSWITCH_RESET,
    /// An unknown or unhandled reset subtype.
    Unknown(u32),
}

impl From<u32> for ResetSubtype {
    fn from(val: u32) -> Self {
        match val {
            ffi::CFE_PSP_RST_SUBTYPE_POWER_CYCLE => Self::PowerCycle,
            ffi::CFE_PSP_RST_SUBTYPE_PUSH_BUTTON => Self::PushButton,
            ffi::CFE_PSP_RST_SUBTYPE_HW_SPECIAL_COMMAND => Self::HwSpecialCommand,
            ffi::CFE_PSP_RST_SUBTYPE_HW_WATCHDOG => Self::HwWatchdog,
            ffi::CFE_PSP_RST_SUBTYPE_RESET_COMMAND => Self::ResetCommand,
            ffi::CFE_PSP_RST_SUBTYPE_EXCEPTION => Self::Exception,
            ffi::CFE_PSP_RST_SUBTYPE_UNDEFINED_RESET => Self::Undefined,
            ffi::CFE_PSP_RST_SUBTYPE_HWDEBUG_RESET => Self::HwDebug,
            ffi::CFE_PSP_RST_SUBTYPE_BANKSWITCH_RESET => Self::BankSwitch,
            other => Self::Unknown(other),
        }
    }
}

/// The overall state of the cFE system, used for startup synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SystemState {
    /// Single-threaded mode during early cFE setup.
    EarlyInit = ffi::CFE_ES_SystemState_CFE_ES_SystemState_EARLY_INIT,
    /// Core cFE apps are starting.
    CoreStartup = ffi::CFE_ES_SystemState_CFE_ES_SystemState_CORE_STARTUP,
    /// Core apps are ready, external apps/libraries are starting.
    CoreReady = ffi::CFE_ES_SystemState_CFE_ES_SystemState_CORE_READY,
    /// External apps have completed early initialization.
    AppsInit = ffi::CFE_ES_SystemState_CFE_ES_SystemState_APPS_INIT,
    /// Normal operation; all apps are running.
    Operational = ffi::CFE_ES_SystemState_CFE_ES_SystemState_OPERATIONAL,
    /// Shutdown state.
    Shutdown = ffi::CFE_ES_SystemState_CFE_ES_SystemState_SHUTDOWN,
}

/// Returns the type and subtype of the most recent processor reset.
pub fn get_reset_type() -> (ResetType, ResetSubtype) {
    let mut subtype = MaybeUninit::uninit();
    let reset_type = unsafe { ffi::CFE_ES_GetResetType(subtype.as_mut_ptr()) };
    (
        (reset_type as u32).into(),
        unsafe { subtype.assume_init() }.into(),
    )
}

/// Allows an application to wait until the cFE system reaches a minimum state.
///
/// This is useful for synchronizing application startup phases. For example, an
/// application can wait until `SystemState::CoreReady` before attempting to
/// subscribe to messages from core cFE services.
///
/// # Arguments
/// * `state`: The minimum system state to wait for.
/// * `timeout`: The maximum duration to wait.
pub fn wait_for_system_state(state: SystemState, timeout: Duration) -> Result<()> {
    let millis = timeout.as_millis();
    let millis_u32 = millis.try_into().unwrap_or(u32::MAX);
    check(unsafe { ffi::CFE_ES_WaitForSystemState(state as u32, millis_u32) })?;
    Ok(())
}

/// Returns the cFE-defined processor ID for the current CPU.
pub fn get_processor_id() -> u32 {
    unsafe { ffi::CFE_PSP_GetProcessorId() }
}

/// Returns the cFE-defined spacecraft ID.
pub fn get_spacecraft_id() -> u32 {
    unsafe { ffi::CFE_PSP_GetSpacecraftId() }
}

/// Reset the cFE Core and all cFE Applications. This function does not return.
///
/// # Arguments
/// * `reset_type`: The type of reset to perform (`PowerOn` or `Processor`).
pub fn reset_cfe(reset_type: ResetType) -> ! {
    let _status = unsafe { ffi::CFE_ES_ResetCFE(reset_type.into()) };

    // CFE_ES_ResetCFE does not return on success. If it returns, it's an error.
    loop {}
}

/// Allows an application to wait until all cFE apps have reached the `OPERATIONAL` state.
///
/// This is a convenience wrapper for `wait_for_system_state(SystemState::Operational, ...)`.
/// It is most useful for applications that need to wait until the entire system is running
/// before proceeding with their own logic.
///
/// # Arguments
/// * `timeout`: The maximum duration to wait.
pub fn wait_for_startup_sync(timeout: Duration) {
    let millis = timeout.as_millis();
    let millis_u32 = millis.try_into().unwrap_or(u32::MAX);
    unsafe {
        ffi::CFE_ES_WaitForStartupSync(millis_u32);
    }
}

/// Wakes up the ES background task to process pending jobs.
///
/// Normally the ES background task wakes up at a periodic interval.
/// Whenever new background work is added, this can be used to wake the task
/// early, which may reduce the delay before the job is processed.
pub fn background_wakeup() {
    unsafe { ffi::CFE_ES_BackgroundWakeup() };
}

/// Notifies ES that an asynchronous event was detected by the underlying OS/PSP.
///
/// This hook routine is called from the PSP when an exception or
/// other asynchronous system event occurs.
pub fn process_async_event() {
    unsafe { ffi::CFE_ES_ProcessAsyncEvent() };
}
