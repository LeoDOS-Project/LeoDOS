//! NovAtel OEM615 GPS receiver.
//!
//! Provides ECEF position/velocity and geodetic coordinates
//! (lat/lon/alt) from GPS constellation signals. Used for
//! orbit determination and time synchronisation. Uses UART.

use crate::error::CfsError;
use crate::ffi;
use crate::nos3::buses::uart::Access;
use crate::nos3::buses::uart::check;
use crate::nos3::buses::uart::Uart;
use crate::nos3::buses::uart::UartError;
use crate::nos3::buses::BusError;

/// GPS position and velocity data.
#[derive(Debug, Clone, Default)]
pub struct GpsData {
    /// GPS weeks since epoch.
    pub weeks: u16,
    /// Seconds into the current week.
    pub seconds_into_week: u32,
    /// Fractional seconds.
    pub fractions: f64,
    /// ECEF X position (m).
    pub ecef_x: f64,
    /// ECEF Y position (m).
    pub ecef_y: f64,
    /// ECEF Z position (m).
    pub ecef_z: f64,
    /// Velocity X (m/s).
    pub vel_x: f64,
    /// Velocity Y (m/s).
    pub vel_y: f64,
    /// Velocity Z (m/s).
    pub vel_z: f64,
    /// Latitude (degrees).
    pub lat: f32,
    /// Longitude (degrees).
    pub lon: f32,
    /// Altitude (m).
    pub alt: f32,
}

/// NovAtel OEM615 GPS receiver handle.
pub struct Gps {
    uart: Uart,
}

#[bon::bon]
impl Gps {
    /// Creates a new GPS receiver on the given UART.
    ///
    /// ```ignore
    /// let gps = Gps::builder()
    ///     .device(c"/dev/ttyS1")
    ///     .baud(115_200)
    ///     .build()?;
    /// ```
    #[builder]
    pub fn new(
        device: &core::ffi::CStr,
        baud: u32,
        #[builder(default = Access::ReadWrite)] access: Access,
    ) -> Result<Self, CfsError> {
        let uart = Uart::open(device, baud, access).map_err(BusError::from)?;
        Ok(Self { uart })
    }
}

impl From<Uart> for Gps {
    fn from(uart: Uart) -> Self {
        Self { uart }
    }
}

impl Gps {
    /// Sends a command to the GPS receiver.
    pub fn command(
        &mut self,
        cmd_code: u8,
        log_type: i8,
        period_option: i8,
    ) -> Result<(), UartError> {
        check(unsafe {
            ffi::NOVATEL_OEM615_CommandDevice(
                &mut self.uart.inner,
                cmd_code,
                log_type,
                period_option,
            )
        })
    }

    /// Requests position/velocity data from the GPS receiver.
    ///
    /// Yields until UART data is available, then performs
    /// the blocking FFI read.
    pub async fn request_data(&mut self) -> Result<GpsData, CfsError> {
        core::future::poll_fn(|_| match self.uart.bytes_available() {
            Ok(n) if n > 0 => core::task::Poll::Ready(Ok(())),
            Ok(_) => core::task::Poll::Pending,
            Err(e) => core::task::Poll::Ready(Err(CfsError::from(BusError::from(e)))),
        })
        .await?;

        let mut raw = ffi::NOVATEL_OEM615_Device_Data_tlm_t::default();
        check(unsafe { ffi::NOVATEL_OEM615_RequestData(&mut self.uart.inner, &mut raw) })
            .map_err(BusError::from)?;
        Ok(gps_from_raw(&raw))
    }

    /// Reads GPS data via the child process interface.
    pub fn child_read_data(&mut self) -> Result<GpsData, UartError> {
        let mut raw = ffi::NOVATEL_OEM615_Device_Data_tlm_t::default();
        check(unsafe {
            ffi::NOVATEL_OEM615_ChildProcessReadData(&mut self.uart.inner, &mut raw)
        })?;
        Ok(gps_from_raw(&raw))
    }
}

fn gps_from_raw(raw: &ffi::NOVATEL_OEM615_Device_Data_tlm_t) -> GpsData {
    GpsData {
        weeks: raw.Weeks,
        seconds_into_week: raw.SecondsIntoWeek,
        fractions: raw.Fractions,
        ecef_x: raw.ECEFX,
        ecef_y: raw.ECEFY,
        ecef_z: raw.ECEFZ,
        vel_x: raw.VelX,
        vel_y: raw.VelY,
        vel_z: raw.VelZ,
        lat: raw.lat,
        lon: raw.lon,
        alt: raw.alt,
    }
}
