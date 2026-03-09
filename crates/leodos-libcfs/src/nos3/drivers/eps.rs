//! Electrical Power System (EPS).
//!
//! Manages battery charging, solar array regulation, and
//! switched power distribution to spacecraft loads. Reports
//! bus voltages, temperatures, and per-switch status over I2C.

use crate::ffi;
use crate::nos3::buses::i2c::{check, I2cError};
use crate::nos3::buses::i2c::I2cBus;

/// Per-switch telemetry.
#[derive(Debug, Clone, Default)]
pub struct SwitchTlm {
    /// Switch voltage (raw ADC).
    pub voltage: u16,
    /// Switch current (raw ADC).
    pub current: u16,
    /// Switch status (0 = off, 1 = on).
    pub status: u16,
}

/// EPS housekeeping telemetry.
#[derive(Debug, Clone, Default)]
pub struct EpsHk {
    /// Battery voltage (raw ADC).
    pub battery_voltage: u16,
    /// Battery temperature (raw ADC).
    pub battery_temperature: u16,
    /// 3.3 V bus voltage (raw ADC).
    pub bus_3v3_voltage: u16,
    /// 5.0 V bus voltage (raw ADC).
    pub bus_5v0_voltage: u16,
    /// 12 V bus voltage (raw ADC).
    pub bus_12v_voltage: u16,
    /// EPS board temperature (raw ADC).
    pub eps_temperature: u16,
    /// Solar array voltage (raw ADC).
    pub solar_array_voltage: u16,
    /// Solar array temperature (raw ADC).
    pub solar_array_temperature: u16,
    /// Per-switch telemetry (8 switches).
    pub switches: [SwitchTlm; 8],
}

/// Computes the EPS 8-bit CRC over `payload`.
pub fn crc8(payload: &[u8]) -> u8 {
    unsafe {
        ffi::GENERIC_EPS_CRC8(
            payload.as_ptr() as *mut _,
            payload.len() as u32,
        )
    }
}

/// Sends a register-write command to the EPS.
pub fn command(
    device: &mut I2cBus,
    reg: u8,
    value: u8,
) -> Result<(), I2cError> {
    check(unsafe {
        ffi::GENERIC_EPS_CommandDevice(
            &mut device.inner,
            reg,
            value,
        )
    })
}

/// Requests housekeeping telemetry from the EPS.
pub fn request_hk(
    device: &mut I2cBus,
) -> Result<EpsHk, I2cError> {
    let mut raw = ffi::GENERIC_EPS_Device_HK_tlm_t::default();
    check(unsafe {
        ffi::GENERIC_EPS_RequestHK(&mut device.inner, &mut raw)
    })?;
    let mut hk = EpsHk {
        battery_voltage: raw.BatteryVoltage,
        battery_temperature: raw.BatteryTemperature,
        bus_3v3_voltage: raw.Bus3p3Voltage,
        bus_5v0_voltage: raw.Bus5p0Voltage,
        bus_12v_voltage: raw.Bus12Voltage,
        eps_temperature: raw.EPSTemperature,
        solar_array_voltage: raw.SolarArrayVoltage,
        solar_array_temperature: raw.SolarArrayTemperature,
        switches: Default::default(),
    };
    for (i, sw) in raw.Switch.iter().enumerate() {
        hk.switches[i] = SwitchTlm {
            voltage: sw.Voltage,
            current: sw.Current,
            status: sw.Status,
        };
    }
    Ok(hk)
}

/// Toggles an EPS power switch and returns updated HK.
pub fn command_switch(
    device: &mut I2cBus,
    switch_num: u8,
    value: u8,
) -> Result<EpsHk, I2cError> {
    let mut raw = ffi::GENERIC_EPS_Device_HK_tlm_t::default();
    check(unsafe {
        ffi::GENERIC_EPS_CommandSwitch(
            &mut device.inner,
            switch_num,
            value,
            &mut raw,
        )
    })?;
    let mut hk = EpsHk {
        battery_voltage: raw.BatteryVoltage,
        battery_temperature: raw.BatteryTemperature,
        bus_3v3_voltage: raw.Bus3p3Voltage,
        bus_5v0_voltage: raw.Bus5p0Voltage,
        bus_12v_voltage: raw.Bus12Voltage,
        eps_temperature: raw.EPSTemperature,
        solar_array_voltage: raw.SolarArrayVoltage,
        solar_array_temperature: raw.SolarArrayTemperature,
        switches: Default::default(),
    };
    for (i, sw) in raw.Switch.iter().enumerate() {
        hk.switches[i] = SwitchTlm {
            voltage: sw.Voltage,
            current: sw.Current,
            status: sw.Status,
        };
    }
    Ok(hk)
}
