//! Attitude Determination and Control System (ADCS).
//!
//! Runs the full sensor-to-actuator control pipeline: reads
//! IMU, magnetometer, sun sensors, and star tracker data,
//! estimates attitude, then computes reaction wheel torque
//! and magnetorquer dipole commands.
//!
//! Supports four control modes: passive, B-dot detumble,
//! sun-safe pointing, and inertial pointing.

use crate::ffi;
use core::mem::MaybeUninit;

unsafe extern "C" {
    fn fopen(path: *const libc::c_char, mode: *const libc::c_char)
        -> *mut ffi::FILE;
    fn fclose(fp: *mut ffi::FILE) -> libc::c_int;
}

/// ADCS control mode.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AdcsMode {
    /// No active control.
    Passive = 0,
    /// B-dot detumble — damps angular rates using magnetorquers.
    Bdot = 1,
    /// Sun-safe — points a body axis at the sun using wheels.
    Sunsafe = 2,
    /// Inertial — tracks a commanded quaternion using wheels.
    Inertial = 3,
}

/// Sensor inputs for one ADAC cycle.
///
/// All vectors are in the spacecraft body frame.
#[derive(Debug, Clone)]
pub struct SensorInput {
    /// Magnetic field vector [T] (body frame).
    pub mag_field: [f64; 3],

    /// Fine sun sensor valid flag.
    pub fss_valid: bool,
    /// Fine sun sensor sun vector (body frame, unit).
    pub fss_sun_vector: [f64; 3],
    /// FSS sensor-to-body quaternion (calibration).
    pub fss_qbs: [f64; 4],

    /// Coarse sun sensor valid flag.
    pub css_valid: bool,
    /// CSS sun vector (body frame, unit).
    pub css_sun_vector: [f64; 3],

    /// IMU valid flag.
    pub imu_valid: bool,
    /// Angular rate [rad/s] (body frame).
    pub angular_rate: [f64; 3],
    /// Linear acceleration [m/s^2] (body frame).
    pub acceleration: [f64; 3],

    /// Star tracker valid flag.
    pub st_valid: bool,
    /// Star tracker quaternion (inertial-to-sensor).
    pub st_quaternion: [f64; 4],
    /// Star tracker sensor-to-body quaternion (calibration).
    pub st_qbs: [f64; 4],

    /// Reaction wheel angular momentum [Nms] (body frame).
    pub wheel_momentum: [f64; 3],
    /// Reaction wheel max momentum [Nms] (body frame).
    pub wheel_max_momentum: [f64; 3],
}

impl Default for SensorInput {
    fn default() -> Self {
        Self {
            mag_field: [0.0; 3],
            fss_valid: false,
            fss_sun_vector: [0.0; 3],
            fss_qbs: [0.0, 0.0, 0.0, 1.0],
            css_valid: false,
            css_sun_vector: [0.0; 3],
            imu_valid: false,
            angular_rate: [0.0; 3],
            acceleration: [0.0; 3],
            st_valid: false,
            st_quaternion: [0.0, 0.0, 0.0, 1.0],
            st_qbs: [0.0, 0.0, 0.0, 1.0],
            wheel_momentum: [0.0; 3],
            wheel_max_momentum: [0.0; 3],
        }
    }
}

/// Actuator commands produced by the ADAC pipeline.
#[derive(Debug, Clone, Default)]
pub struct ActuatorCmd {
    /// Reaction wheel torque command [Nm] (body frame).
    pub torque_cmd: [f64; 3],
    /// Magnetorquer dipole command [Am^2] (body frame).
    pub magnetic_cmd: [f64; 3],
}

/// ADCS controller state.
///
/// Holds the internal attitude determination, GNC, and
/// attitude control state across cycles.
pub struct Adcs {
    ad: ffi::Generic_ADCS_AD_Tlm_Payload_t,
    gnc: ffi::Generic_ADCS_GNC_Tlm_Payload_t,
    ac: ffi::Generic_ADCS_AC_Tlm_Payload_t,
}

impl Adcs {
    /// Initialises the ADCS from a configuration file.
    ///
    /// The file format matches the NOS3 generic_adcs config
    /// (IMU filter alpha, GNC timestep, controller gains, etc.).
    pub fn from_config(
        path: &core::ffi::CStr,
    ) -> Option<Self> {
        let mode = c"r";
        let fp = unsafe {
            fopen(path.as_ptr(), mode.as_ptr())
        };
        if fp.is_null() {
            return None;
        }
        let mut ad: ffi::Generic_ADCS_AD_Tlm_Payload_t =
            unsafe { MaybeUninit::zeroed().assume_init() };
        let mut gnc: ffi::Generic_ADCS_GNC_Tlm_Payload_t =
            unsafe { MaybeUninit::zeroed().assume_init() };
        let mut ac: ffi::Generic_ADCS_AC_Tlm_Payload_t =
            unsafe { MaybeUninit::zeroed().assume_init() };
        unsafe {
            ffi::Generic_ADCS_init_attitude_determination_and_attitude_control(
                fp, &mut ad, &mut gnc, &mut ac,
            );
            fclose(fp);
        }
        Some(Self { ad, gnc, ac })
    }

    /// Runs one ADAC cycle.
    ///
    /// Feeds sensor data through attitude determination then
    /// the active control law, returning actuator commands.
    pub fn execute(
        &mut self,
        input: &SensorInput,
    ) -> ActuatorCmd {
        let mut di: ffi::Generic_ADCS_DI_Tlm_Payload_t =
            unsafe { MaybeUninit::zeroed().assume_init() };

        // Magnetometer
        di.Mag.bvb = input.mag_field;

        // Fine sun sensor
        di.Fss.valid = input.fss_valid as u8;
        di.Fss.svb = input.fss_sun_vector;
        di.Fss.qbs = input.fss_qbs;

        // Coarse sun sensor
        di.Css.valid = input.css_valid as u8;
        di.Css.svb = input.css_sun_vector;

        // IMU
        di.Imu.valid = input.imu_valid as u8;
        di.Imu.wbn = input.angular_rate;
        di.Imu.acc = input.acceleration;

        // Star tracker
        di.St.valid = input.st_valid as u8;
        di.St.q = input.st_quaternion;
        di.St.qbs = input.st_qbs;

        // Reaction wheels
        di.Rw.HwhlB = input.wheel_momentum;
        di.Rw.H_maxB = input.wheel_max_momentum;

        unsafe {
            ffi::Generic_ADCS_execute_attitude_determination_and_attitude_control(
                &di, &mut self.ad, &mut self.gnc, &mut self.ac,
            );
        }

        ActuatorCmd {
            torque_cmd: self.gnc.Tcmd,
            magnetic_cmd: self.gnc.Mcmd,
        }
    }

    /// Returns the current control mode.
    pub fn mode(&self) -> AdcsMode {
        match self.gnc.Mode {
            1 => AdcsMode::Bdot,
            2 => AdcsMode::Sunsafe,
            3 => AdcsMode::Inertial,
            _ => AdcsMode::Passive,
        }
    }

    /// Sets the control mode.
    pub fn set_mode(&mut self, mode: AdcsMode) {
        self.gnc.Mode = mode as u8;
    }

    /// Enables or disables momentum management.
    pub fn set_momentum_management(&mut self, enabled: bool) {
        self.gnc.HmgmtOn = enabled as u8;
    }

    /// Sets the commanded inertial quaternion.
    pub fn set_target_quaternion(&mut self, q: [f64; 4]) {
        self.ac.Inertial.qbn_cmd = q;
    }
}
