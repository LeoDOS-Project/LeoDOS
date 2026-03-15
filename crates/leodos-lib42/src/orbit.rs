use crate::ffi;
use crate::types::{Mat3, Vec3};

/// Classical Keplerian orbital elements.
#[derive(Debug, Clone, Copy)]
pub struct KeplerianElements {
    /// Gravitational parameter (km^3/s^2).
    pub mu: f64,
    /// Semi-major axis (km).
    pub sma: f64,
    /// Eccentricity (dimensionless).
    pub ecc: f64,
    /// Inclination (rad).
    pub inc: f64,
    /// Right ascension of the ascending node (rad).
    pub raan: f64,
    /// Argument of periapsis (rad).
    pub arg_peri: f64,
    /// True anomaly (rad).
    pub true_anom: f64,
}

/// Position and velocity vectors.
#[derive(Debug, Clone, Copy)]
pub struct PosVel {
    /// Position vector (km).
    pub pos: Vec3,
    /// Velocity vector (km/s).
    pub vel: Vec3,
}

/// Convert Keplerian elements to position/velocity after time `dt`.
pub fn eph_to_rv(eph: &KeplerianElements, dt: f64) -> (PosVel, f64) {
    let mut r = Vec3::default();
    let mut v = Vec3::default();
    let mut anom = 0.0;
    // Eph2RV takes: mu, p (SLR), e, i, RAAN, ArgP, dt, r, v, anom
    // where p = sma * (1 - e^2) is the semi-latus rectum
    let p = eph.sma * (1.0 - eph.ecc * eph.ecc);
    unsafe {
        ffi::Eph2RV(
            eph.mu,
            p,
            eph.ecc,
            eph.inc,
            eph.raan,
            eph.arg_peri,
            dt,
            r.as_mut_ptr(),
            v.as_mut_ptr(),
            &mut anom,
        )
    };
    (PosVel { pos: r, vel: v }, anom)
}

/// Keplerian elements plus derived orbital parameters.
#[derive(Debug, Clone, Copy)]
pub struct FullEphemeris {
    /// Classical Keplerian elements.
    pub elements: KeplerianElements,
    /// Semi-latus rectum (km).
    pub slr: f64,
    /// Reciprocal of semi-major axis (1/km).
    pub alpha: f64,
    /// Periapsis radius (km).
    pub rmin: f64,
    /// Mean motion (rad/s).
    pub mean_motion: f64,
    /// Orbital period (s).
    pub period: f64,
}

/// Convert position/velocity to full Keplerian ephemeris.
pub fn rv_to_eph(mu: f64, r: &Vec3, v: &Vec3) -> FullEphemeris {
    let mut sma = 0.0;
    let mut ecc = 0.0;
    let mut inc = 0.0;
    let mut raan = 0.0;
    let mut arg_peri = 0.0;
    let mut true_anom = 0.0;
    let mut tp = 0.0;
    let mut slr = 0.0;
    let mut alpha = 0.0;
    let mut rmin = 0.0;
    let mut mean_motion = 0.0;
    let mut period = 0.0;
    unsafe {
        ffi::RV2Eph(
            0.0,
            mu,
            r.as_ptr().cast_mut(),
            v.as_ptr().cast_mut(),
            &mut sma,
            &mut ecc,
            &mut inc,
            &mut raan,
            &mut arg_peri,
            &mut true_anom,
            &mut tp,
            &mut slr,
            &mut alpha,
            &mut rmin,
            &mut mean_motion,
            &mut period,
        )
    };
    FullEphemeris {
        elements: KeplerianElements {
            mu,
            sma,
            ecc,
            inc,
            raan,
            arg_peri,
            true_anom,
        },
        slr,
        alpha,
        rmin,
        mean_motion,
        period,
    }
}

/// Convert mean anomaly to true anomaly via Kepler's equation.
pub fn mean_anom_to_true_anom(mean_anom: f64, ecc: f64) -> f64 {
    unsafe { ffi::MeanAnomToTrueAnom(mean_anom, ecc) }
}

/// Compute true anomaly from gravitational parameter, SLR, eccentricity, and time.
pub fn true_anomaly(mu: f64, p: f64, e: f64, t: f64) -> f64 {
    unsafe { ffi::TrueAnomaly(mu, p, e, t) }
}

/// Compute elapsed time since periapsis for the given true anomaly.
pub fn time_since_periapsis(mu: f64, p: f64, e: f64, th: f64) -> f64 {
    unsafe { ffi::TimeSincePeriapsis(mu, p, e, th) }
}

/// Propagate initial position/velocity to a new true anomaly.
pub fn rv0_to_rv(mu: f64, r0: &Vec3, v0: &Vec3, anom: f64) -> PosVel {
    let mut r = Vec3::default();
    let mut v = Vec3::default();
    unsafe {
        ffi::RV02RV(
            mu,
            r0.as_ptr().cast_mut(),
            v0.as_ptr().cast_mut(),
            anom,
            r.as_mut_ptr(),
            v.as_mut_ptr(),
        )
    };
    PosVel { pos: r, vel: v }
}

/// Compute the CLN (radial/along-track/cross-track) rotation matrix and angular velocity.
pub fn find_cln(r: &Vec3, v: &Vec3) -> (Mat3, Vec3) {
    let mut cln = Mat3::default();
    let mut wln = Vec3::default();
    unsafe {
        ffi::FindCLN(
            r.as_ptr().cast_mut(),
            v.as_ptr().cast_mut(),
            cln.as_mut_ptr(),
            wln.as_mut_ptr(),
        )
    };
    (cln, wln)
}

/// Compute the CEN (ECEF-to-NED) rotation matrix from position.
pub fn find_cen(r: &Vec3) -> Mat3 {
    let mut cen = Mat3::default();
    unsafe { ffi::FindCEN(r.as_ptr().cast_mut(), cen.as_mut_ptr()) };
    cen
}

/// Compute the sphere-of-influence radius for a two-body system.
pub fn radius_of_influence(mu1: f64, mu2: f64, r: f64) -> f64 {
    unsafe { ffi::RadiusOfInfluence(mu1, mu2, r) }
}

/// Compute the Moon's position vector at the given Julian date.
pub fn luna_position(jd: f64) -> Vec3 {
    let mut r = Vec3::default();
    unsafe { ffi::LunaPosition(jd, r.as_mut_ptr()) };
    r
}

/// Compute the Moon's inertial reference frame at the given Julian date.
pub fn luna_inertial_frame(jd: f64) -> Mat3 {
    let mut cnj = Mat3::default();
    unsafe { ffi::LunaInertialFrame(jd, cnj.as_mut_ptr()) };
    cnj
}

/// Compute the Moon's prime meridian angle at the given Julian date.
pub fn luna_pri_mer_ang(jd: f64) -> f64 {
    unsafe { ffi::LunaPriMerAng(jd) }
}

/// Compute the TETE-to-J2000 rotation matrix at the given Julian date.
pub fn tete_to_j2000(jd: f64) -> Mat3 {
    let mut ctj = Mat3::default();
    unsafe { ffi::TETE2J2000(jd, ctj.as_mut_ptr()) };
    ctj
}

/// Compute Lambert time-of-flight for the given transfer geometry.
pub fn lambda_tof(mu: f64, amin: f64, lambda: f64, x: f64) -> f64 {
    unsafe { ffi::LambertTOF(mu, amin, lambda, x) }
}

/// Convert WGS-84 geodetic coordinates (lat, lon, alt) to ECEF position.
pub fn wgs84_to_ecef(lat: f64, lon: f64, alt: f64) -> Vec3 {
    let mut p = Vec3::default();
    unsafe { ffi::WGS84ToECEF(lat, lon, alt, p.as_mut_ptr()) };
    p
}

/// Convert ECEF position to WGS-84 geodetic (lat, lon, alt).
pub fn ecef_to_wgs84(p: &Vec3) -> (f64, f64, f64) {
    let mut lat = 0.0;
    let mut lon = 0.0;
    let mut alt = 0.0;
    unsafe { ffi::ECEFToWGS84(p.as_ptr().cast_mut(), &mut lat, &mut lon, &mut alt) };
    (lat, lon, alt)
}

/// Compute the ENU (East-North-Up) frame and angular velocity from position.
pub fn find_enu(pos_n: &Vec3, world_w: f64) -> (Mat3, Vec3) {
    let mut cln = Mat3::default();
    let mut wln = Vec3::default();
    unsafe {
        ffi::FindENU(
            pos_n.as_ptr().cast_mut(),
            world_w,
            cln.as_mut_ptr(),
            wln.as_mut_ptr(),
        )
    };
    (cln, wln)
}

/// Compute simplified Earth precession/nutation matrices (TEME-TETE and TETE-J2000).
pub fn simple_earth_prec_nute(jd: f64) -> (Mat3, Mat3) {
    let mut c_teme_tete = Mat3::default();
    let mut c_tete_j2000 = Mat3::default();
    unsafe { ffi::SimpleEarthPrecNute(jd, c_teme_tete.as_mut_ptr(), c_tete_j2000.as_mut_ptr()) };
    (c_teme_tete, c_tete_j2000)
}

/// Compute high-fidelity Earth precession/nutation matrices (TEME-TETE and TETE-J2000).
pub fn hifi_earth_prec_nute(jd: f64) -> (Mat3, Mat3) {
    let mut c_teme_tete = Mat3::default();
    let mut c_tete_j2000 = Mat3::default();
    unsafe { ffi::HiFiEarthPrecNute(jd, c_teme_tete.as_mut_ptr(), c_tete_j2000.as_mut_ptr()) };
    (c_teme_tete, c_tete_j2000)
}

/// Compute approximate planetary ephemeris for the given planet at a Julian date.
pub fn planet_ephemerides(planet_id: i64, jd: f64, mu: f64) -> FullEphemeris {
    let mut sma = 0.0;
    let mut ecc = 0.0;
    let mut inc = 0.0;
    let mut raan = 0.0;
    let mut arg_peri = 0.0;
    let mut tp = 0.0;
    let mut anom = 0.0;
    let mut slr = 0.0;
    let mut alpha = 0.0;
    let mut rmin = 0.0;
    let mut mean_motion = 0.0;
    let mut period = 0.0;
    unsafe {
        ffi::PlanetEphemerides(
            planet_id,
            jd,
            mu,
            &mut sma,
            &mut ecc,
            &mut inc,
            &mut raan,
            &mut arg_peri,
            &mut tp,
            &mut anom,
            &mut slr,
            &mut alpha,
            &mut rmin,
            &mut mean_motion,
            &mut period,
        )
    };
    FullEphemeris {
        elements: KeplerianElements {
            mu,
            sma,
            ecc,
            inc,
            raan,
            arg_peri,
            true_anom: anom,
        },
        slr,
        alpha,
        rmin,
        mean_motion,
        period,
    }
}

/// Propagate position/velocity to the next periapsis, returning the new state and elapsed time.
pub fn rv_to_rv_prime(mu: f64, r: &Vec3, v: &Vec3) -> (PosVel, f64) {
    let mut rp = Vec3::default();
    let mut vp = Vec3::default();
    let dt = unsafe {
        ffi::RV2RVp(
            mu,
            r.as_ptr().cast_mut(),
            v.as_ptr().cast_mut(),
            rp.as_mut_ptr(),
            vp.as_mut_ptr(),
        )
    };
    (PosVel { pos: rp, vel: vp }, dt)
}

/// Orbital elements describing a Lambert transfer arc.
#[derive(Debug, Clone, Copy)]
pub struct LambertSolution {
    /// Semi-latus rectum (km).
    pub slr: f64,
    /// Eccentricity (dimensionless).
    pub ecc: f64,
    /// Inclination (rad).
    pub inc: f64,
    /// Right ascension of the ascending node (rad).
    pub raan: f64,
    /// Argument of periapsis (rad).
    pub arg_peri: f64,
    /// Time of periapsis passage (s).
    pub tp: f64,
}

/// Solve the Lambert boundary-value problem for a transfer orbit.
pub fn lambert_problem(
    t0: f64,
    mu: f64,
    r1: &Vec3,
    r2: &Vec3,
    tof: f64,
    transfer_type: f64,
) -> LambertSolution {
    let mut slr = 0.0;
    let mut ecc = 0.0;
    let mut inc = 0.0;
    let mut raan = 0.0;
    let mut arg_peri = 0.0;
    let mut tp = 0.0;
    unsafe {
        ffi::LambertProblem(
            t0,
            mu,
            r1.as_ptr().cast_mut(),
            r2.as_ptr().cast_mut(),
            tof,
            transfer_type,
            &mut slr,
            &mut ecc,
            &mut inc,
            &mut raan,
            &mut arg_peri,
            &mut tp,
        )
    };
    LambertSolution {
        slr,
        ecc,
        inc,
        raan,
        arg_peri,
        tp,
    }
}

/// Plan a two-impulse rendezvous, returning burn times and delta-v vectors.
pub fn plan_two_impulse_rendezvous(
    mu: f64,
    r1e: &Vec3,
    v1e: &Vec3,
    r2e: &Vec3,
    v2e: &Vec3,
) -> (f64, f64, Vec3, Vec3) {
    let mut t1 = 0.0;
    let mut t2 = 0.0;
    let mut dv1 = Vec3::default();
    let mut dv2 = Vec3::default();
    unsafe {
        ffi::PlanTwoImpulseRendezvous(
            mu,
            r1e.as_ptr().cast_mut(),
            v1e.as_ptr().cast_mut(),
            r2e.as_ptr().cast_mut(),
            v2e.as_ptr().cast_mut(),
            &mut t1,
            &mut t2,
            dv1.as_mut_ptr(),
            dv2.as_mut_ptr(),
        )
    };
    (t1, t2, dv1, dv2)
}

/// Compute TDRS constellation positions and velocities at the given time.
pub fn tdrs_pos_vel(pri_mer_ang: f64, time: f64) -> (Mat3, Mat3) {
    let mut ptn = Mat3::default();
    let mut vtn = Mat3::default();
    unsafe { ffi::TDRSPosVel(pri_mer_ang, time, ptn.as_mut_ptr(), vtn.as_mut_ptr()) };
    (ptn, vtn)
}

/// Convert osculating elements to mean elements (J2 averaging).
pub fn osc_eph_to_mean_eph(mu: f64, j2: f64, rw: f64, dyn_time: f64, orb: &mut ffi::OrbitType) {
    unsafe { ffi::OscEphToMeanEph(mu, j2, rw, dyn_time, orb) }
}

/// Convert mean elements to osculating elements.
pub fn mean_eph_to_osc_eph(orb: &mut ffi::OrbitType, dyn_time: f64) {
    unsafe { ffi::MeanEphToOscEph(orb, dyn_time) }
}

/// Convert mean elements to position/velocity in-place.
pub fn mean_eph_to_rv(orb: &mut ffi::OrbitType, dyn_time: f64) {
    unsafe { ffi::MeanEph2RV(orb, dyn_time) }
}

// --- TLE ---

/// Parse TLE lines into mean ephemeris on an OrbitType.
pub fn tle_to_mean_eph(
    line1: &[u8; 80],
    line2: &[u8; 80],
    jd: f64,
    leap_sec: f64,
    orb: &mut ffi::OrbitType,
) {
    unsafe {
        ffi::TLE2MeanEph(
            line1.as_ptr().cast(),
            line2.as_ptr().cast(),
            jd,
            leap_sec,
            orb,
        )
    }
}

/// Load TLE from file into an OrbitType.
/// Returns nonzero on success.
pub fn load_tle_from_file(
    path: &core::ffi::CStr,
    tle_file: &core::ffi::CStr,
    tle_label: &core::ffi::CStr,
    dyn_time: f64,
    jd: f64,
    leap_sec: f64,
    orb: &mut ffi::OrbitType,
) -> i64 {
    unsafe {
        ffi::LoadTleFromFile(
            path.as_ptr(),
            tle_file.as_ptr(),
            tle_label.as_ptr(),
            dyn_time,
            jd,
            leap_sec,
            orb,
        )
    }
}

// --- Lagrange point operations ---

/// Compute Lagrange-point parameters for a two-body system.
pub fn find_lag_pt_parms(ls: &mut ffi::LagrangeSystemType) {
    unsafe { ffi::FindLagPtParms(ls) }
}

/// Compute position, velocity, and CLN frame for a Lagrange point.
pub fn find_lag_pt_pos_vel(
    sec_since_j2000: f64,
    ls: &mut ffi::LagrangeSystemType,
    ilp: i64,
) -> (Vec3, Vec3, Mat3) {
    let mut pos = Vec3::default();
    let mut vel = Vec3::default();
    let mut cln = Mat3::default();
    unsafe {
        ffi::FindLagPtPosVel(
            sec_since_j2000,
            ls,
            ilp,
            pos.as_mut_ptr(),
            vel.as_mut_ptr(),
            cln.as_mut_ptr(),
        )
    };
    (pos, vel, cln)
}

/// Decompose position/velocity into Lagrange-point modal amplitudes.
pub fn rv_to_lag_modes(
    sec_since_j2000: f64,
    ls: &mut ffi::LagrangeSystemType,
    orb: &mut ffi::OrbitType,
) {
    unsafe { ffi::RV2LagModes(sec_since_j2000, ls, orb) }
}

/// Reconstruct position/velocity from Lagrange-point modal amplitudes.
pub fn lag_modes_to_rv(
    sec_since_j2000: f64,
    ls: &mut ffi::LagrangeSystemType,
    orb: &mut ffi::OrbitType,
) -> PosVel {
    let mut r = Vec3::default();
    let mut v = Vec3::default();
    unsafe { ffi::LagModes2RV(sec_since_j2000, ls, orb, r.as_mut_ptr(), v.as_mut_ptr()) };
    PosVel { pos: r, vel: v }
}

/// Reduce to the two stable Lagrange-point modes (remove unstable component).
pub fn r2_stable_lag_mode(
    sec_since_j2000: f64,
    ls: &mut ffi::LagrangeSystemType,
    orb: &mut ffi::OrbitType,
) {
    unsafe { ffi::R2StableLagMode(sec_since_j2000, ls, orb) }
}

/// Convert Cartesian displacements to Lagrange-point modal amplitudes.
pub fn xyz_to_lag_modes(
    time_since_epoch: f64,
    ls: &mut ffi::LagrangeSystemType,
    orb: &mut ffi::OrbitType,
) {
    unsafe { ffi::XYZ2LagModes(time_since_epoch, ls, orb) }
}

/// Initialize Lagrange-point modes from amplitude/phase pairs.
pub fn amp_phase_to_lag_modes(
    time_since_epoch: f64,
    amp_xy1: f64,
    phi_xy1: f64,
    sense_xy1: f64,
    amp_xy2: f64,
    phi_xy2: f64,
    sense_xy2: f64,
    amp_z: f64,
    phi_z: f64,
    ls: &mut ffi::LagrangeSystemType,
    orb: &mut ffi::OrbitType,
) {
    unsafe {
        ffi::AmpPhase2LagModes(
            time_since_epoch,
            amp_xy1,
            phi_xy1,
            sense_xy1,
            amp_xy2,
            phi_xy2,
            sense_xy2,
            amp_z,
            phi_z,
            ls,
            orb,
        )
    }
}

// --- Euler-Hill / Clohessy-Wiltshire ---

/// Convert relative pos/vel to Euler-Hill pos/vel.
pub fn rel_rv_to_ehrv(
    orb_radius: f64,
    orb_rate: f64,
    orb_cln: &Mat3,
    r_rel: &Vec3,
    v_rel: &Vec3,
) -> PosVel {
    let mut re = Vec3::default();
    let mut ve = Vec3::default();
    unsafe {
        ffi::RelRV2EHRV(
            orb_radius,
            orb_rate,
            orb_cln.as_ptr().cast_mut(),
            r_rel.as_ptr().cast_mut(),
            v_rel.as_ptr().cast_mut(),
            re.as_mut_ptr(),
            ve.as_mut_ptr(),
        )
    };
    PosVel { pos: re, vel: ve }
}

/// Convert Euler-Hill pos/vel to relative pos/vel.
pub fn ehrv_to_rel_rv(
    orb_radius: f64,
    orb_rate: f64,
    orb_cln: &Mat3,
    re: &Vec3,
    ve: &Vec3,
) -> PosVel {
    let mut r_rel = Vec3::default();
    let mut v_rel = Vec3::default();
    unsafe {
        ffi::EHRV2RelRV(
            orb_radius,
            orb_rate,
            orb_cln.as_ptr().cast_mut(),
            re.as_ptr().cast_mut(),
            ve.as_ptr().cast_mut(),
            r_rel.as_mut_ptr(),
            v_rel.as_mut_ptr(),
        )
    };
    PosVel {
        pos: r_rel,
        vel: v_rel,
    }
}

/// Modal amplitudes for Euler-Hill (Clohessy-Wiltshire) relative motion.
#[derive(Debug, Clone, Copy)]
pub struct EulerHillModes {
    /// Along-track drift amplitude.
    pub a: f64,
    /// In-plane cosine amplitude.
    pub bc: f64,
    /// In-plane sine amplitude.
    pub bs: f64,
    /// Cross-track drift amplitude.
    pub c: f64,
    /// Cross-track cosine amplitude.
    pub dc: f64,
    /// Cross-track sine amplitude.
    pub ds: f64,
}

/// Convert Euler-Hill pos/vel to modal amplitudes.
pub fn ehrv_to_eh_modes(r: &Vec3, v: &Vec3, n: f64, nt: f64) -> EulerHillModes {
    let mut a = 0.0;
    let mut bc = 0.0;
    let mut bs = 0.0;
    let mut c = 0.0;
    let mut dc = 0.0;
    let mut ds = 0.0;
    unsafe {
        ffi::EHRV2EHModes(
            r.as_ptr().cast_mut(),
            v.as_ptr().cast_mut(),
            n,
            nt,
            &mut a,
            &mut bc,
            &mut bs,
            &mut c,
            &mut dc,
            &mut ds,
        )
    };
    EulerHillModes {
        a,
        bc,
        bs,
        c,
        dc,
        ds,
    }
}

/// Convert modal amplitudes to Euler-Hill pos/vel.
pub fn eh_modes_to_ehrv(modes: &EulerHillModes, n: f64, nt: f64) -> PosVel {
    let mut r = Vec3::default();
    let mut v = Vec3::default();
    unsafe {
        ffi::EHModes2EHRV(
            modes.a,
            modes.bc,
            modes.bs,
            modes.c,
            modes.dc,
            modes.ds,
            n,
            nt,
            r.as_mut_ptr(),
            v.as_mut_ptr(),
        )
    };
    PosVel { pos: r, vel: v }
}

/// Find light-lag offsets between observer and target.
pub fn find_light_lag_offsets(
    dyn_time: f64,
    observer: &mut ffi::OrbitType,
    target: &mut ffi::OrbitType,
) -> (Vec3, Vec3) {
    let mut past_pos = Vec3::default();
    let mut future_pos = Vec3::default();
    unsafe {
        ffi::FindLightLagOffsets(
            dyn_time,
            observer,
            target,
            past_pos.as_mut_ptr(),
            future_pos.as_mut_ptr(),
        )
    };
    (past_pos, future_pos)
}
