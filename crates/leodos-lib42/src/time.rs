use crate::ffi;

/// Calendar date and time broken into components.
#[derive(Debug, Clone, Copy, Default)]
pub struct Date {
    /// Calendar year.
    pub year: i64,
    /// Month of the year (1-12).
    pub month: i64,
    /// Day of the month.
    pub day: i64,
    /// Hour of the day (0-23).
    pub hour: i64,
    /// Minute of the hour (0-59).
    pub minute: i64,
    /// Second of the minute (fractional).
    pub second: f64,
}

/// GPS date broken into rollover, week, and second.
#[derive(Debug, Clone, Copy)]
pub struct GpsDate {
    /// GPS week rollover count.
    pub rollover: i64,
    /// GPS week number within the current rollover.
    pub week: i64,
    /// Seconds elapsed within the GPS week.
    pub second: f64,
}

/// Convert dynamic time (seconds since J2000) to Julian date.
pub fn time_to_jd(time: f64) -> f64 {
    unsafe { ffi::TimeToJD(time) }
}

/// Convert Julian date to dynamic time (seconds since J2000).
pub fn jd_to_time(jd: f64) -> f64 {
    unsafe { ffi::JDToTime(jd) }
}

/// Convert a calendar date to dynamic time (seconds since J2000).
pub fn date_to_time(d: &Date) -> f64 {
    unsafe { ffi::DateToTime(d.year, d.month, d.day, d.hour, d.minute, d.second) }
}

/// Convert a calendar date to Julian date.
pub fn date_to_jd(d: &Date) -> f64 {
    unsafe { ffi::DateToJD(d.year, d.month, d.day, d.hour, d.minute, d.second) }
}

/// Convert a Julian date to a calendar date.
pub fn jd_to_date(jd: f64) -> Date {
    let mut d = Date::default();
    unsafe {
        ffi::JDToDate(
            jd,
            &mut d.year,
            &mut d.month,
            &mut d.day,
            &mut d.hour,
            &mut d.minute,
            &mut d.second,
        )
    };
    d
}

/// Convert dynamic time to a calendar date with the given LSB granularity.
pub fn time_to_date(time: f64, lsb: f64) -> Date {
    let mut d = Date::default();
    unsafe {
        ffi::TimeToDate(
            time,
            &mut d.year,
            &mut d.month,
            &mut d.day,
            &mut d.hour,
            &mut d.minute,
            &mut d.second,
            lsb,
        )
    };
    d
}

/// Convert month and day to day-of-year for the given year.
pub fn md_to_doy(year: i64, month: i64, day: i64) -> i64 {
    unsafe { ffi::MD2DOY(year, month, day) }
}

/// Convert day-of-year to (month, day) for the given year.
pub fn doy_to_md(year: i64, doy: i64) -> (i64, i64) {
    let mut month = 0;
    let mut day = 0;
    unsafe { ffi::DOY2MD(year, doy, &mut month, &mut day) };
    (month, day)
}

/// Convert Julian date to Greenwich Mean Sidereal Time (radians).
pub fn jd_to_gmst(jd: f64) -> f64 {
    unsafe { ffi::JD2GMST(jd) }
}

/// Convert GPS time (seconds) to a [`GpsDate`].
pub fn gps_time_to_date(gps_time: f64) -> GpsDate {
    let mut d = GpsDate {
        rollover: 0,
        week: 0,
        second: 0.0,
    };
    unsafe { ffi::GpsTimeToGpsDate(gps_time, &mut d.rollover, &mut d.week, &mut d.second) };
    d
}

/// Convert a [`GpsDate`] to GPS time (seconds).
pub fn gps_date_to_time(d: &GpsDate) -> f64 {
    unsafe { ffi::GpsDateToGpsTime(d.rollover, d.week, d.second) }
}

/// Get the current time in microseconds (monotonic clock).
pub fn usec() -> f64 {
    unsafe { ffi::usec() }
}

/// Get real system wall-clock time.
pub fn real_system_time(lsb: f64) -> Date {
    let mut d = Date::default();
    let mut doy = 0i64;
    unsafe {
        ffi::RealSystemTime(
            &mut d.year, &mut doy, &mut d.month, &mut d.day,
            &mut d.hour, &mut d.minute, &mut d.second, lsb,
        )
    };
    d
}

/// Get elapsed real run time. Returns (total_time, dt).
pub fn real_run_time(lsb: f64) -> (f64, f64) {
    let mut dt = 0.0;
    let total = unsafe { ffi::RealRunTime(&mut dt, lsb) };
    (total, dt)
}
