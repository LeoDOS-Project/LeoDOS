#![no_std]

//! NOS3 demo app — polls EPS, MAG, and IMU simulators and logs
//! the readings via cFS events.

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::log;
use leodos_libcfs::nos3::buses::can::Can;
use leodos_libcfs::nos3::buses::i2c::I2cBus;
use leodos_libcfs::nos3::buses::spi::Spi;
use leodos_libcfs::nos3::drivers::{eps, imu, mag};
use leodos_libcfs::runtime::time::sleep;
use leodos_libcfs::runtime::Runtime;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(non_snake_case)]
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

/// EPS I2C slave address (7-bit).
const EPS_I2C_ADDR: i32 = 0x2B;
/// EPS I2C bus speed (kbps).
const EPS_I2C_SPEED: u32 = 1000;

/// MAG SPI bus number (for mutex).
const MAG_SPI_BUS: u8 = 0;
/// MAG SPI chip-select line.
const MAG_SPI_CS: u8 = 2;
/// MAG SPI clock speed (Hz).
const MAG_SPI_BAUDRATE: u32 = 1_000_000;
/// MAG SPI mode.
const MAG_SPI_MODE: u8 = 1;
/// MAG SPI bits per word.
const MAG_SPI_BPW: u8 = 8;

/// IMU CAN bus handle.
const IMU_CAN_HANDLE: i32 = 15;
/// IMU CAN bitrate (bps).
const IMU_CAN_BITRATE: u32 = 1_000_000;

/// Polling interval between sensor reads (seconds).
const POLL_SECS: u32 = 2;

#[no_mangle]
pub extern "C" fn NOS3_DEMO_AppMain() {
    Runtime::new().run(async {
        log!("NOS3_DEMO: starting sensor poll demo").ok();

        // Open buses
        let mut i2c = match I2cBus::open(EPS_I2C_ADDR, EPS_I2C_SPEED) {
            Ok(bus) => {
                log!("NOS3_DEMO: I2C bus opened (EPS)").ok();
                Some(bus)
            }
            Err(e) => {
                log!("NOS3_DEMO: I2C open failed: {:?}", e).ok();
                None
            }
        };

        let mut spi = match Spi::open(
            c"spi_2",
            MAG_SPI_BUS,
            MAG_SPI_CS,
            MAG_SPI_BAUDRATE,
            MAG_SPI_MODE,
            MAG_SPI_BPW,
        ) {
            Ok(dev) => {
                log!("NOS3_DEMO: SPI device opened (MAG)").ok();
                Some(dev)
            }
            Err(e) => {
                log!("NOS3_DEMO: SPI open failed: {:?}", e).ok();
                None
            }
        };

        let mut can = match Can::open(IMU_CAN_HANDLE, IMU_CAN_BITRATE) {
            Ok(dev) => {
                log!("NOS3_DEMO: CAN bus opened (IMU)").ok();
                Some(dev)
            }
            Err(e) => {
                log!("NOS3_DEMO: CAN open failed: {:?}", e).ok();
                None
            }
        };

        let mut cycle: u32 = 0;
        loop {
            cycle = cycle.wrapping_add(1);

            // --- EPS (I2C) ---
            if let Some(ref mut bus) = i2c {
                match eps::request_hk(bus) {
                    Ok(hk) => {
                        log!(
                            "NOS3_DEMO: [{}] EPS batt={}  3v3={}  5v={}  12v={}  temp={}",
                            cycle,
                            hk.battery_voltage,
                            hk.bus_3v3_voltage,
                            hk.bus_5v0_voltage,
                            hk.bus_12v_voltage,
                            hk.eps_temperature,
                        )
                        .ok();
                    }
                    Err(e) => {
                        log!("NOS3_DEMO: [{}] EPS read err: {:?}", cycle, e).ok();
                    }
                }
            }

            // --- MAG (SPI) ---
            if let Some(ref mut dev) = spi {
                match mag::request_data(dev) {
                    Ok(d) => {
                        log!("NOS3_DEMO: [{}] MAG x={}  y={}  z={}", cycle, d.x, d.y, d.z,).ok();
                    }
                    Err(e) => {
                        log!("NOS3_DEMO: [{}] MAG read err: {:?}", cycle, e).ok();
                    }
                }
            }

            // --- IMU (CAN) ---
            if let Some(ref mut dev) = can {
                match imu::request_data(dev) {
                    Ok(d) => {
                        log!(
                            "NOS3_DEMO: [{}] IMU gx={} gy={} gz={} ax={} ay={} az={}",
                            cycle,
                            d.x.angular_acc,
                            d.y.angular_acc,
                            d.z.angular_acc,
                            d.x.linear_acc,
                            d.y.linear_acc,
                            d.z.linear_acc,
                        )
                        .ok();
                    }
                    Err(e) => {
                        log!("NOS3_DEMO: [{}] IMU read err: {:?}", cycle, e).ok();
                    }
                }
            }

            sleep(Duration::from_secs(POLL_SECS)).await;
        }
    });
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    leodos_libcfs::cfe::es::app::default_panic_handler(info)
}
