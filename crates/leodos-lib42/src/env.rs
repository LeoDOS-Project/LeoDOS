use crate::ffi;
use crate::types::{Mat3, Vec3};
use core::ffi::CStr;

/// Convert geomagnetic Kp index to Ap index.
pub fn kp_to_ap(kp: f64) -> f64 {
    unsafe { ffi::KpToAp(kp) }
}

/// Mars atmosphere density model.
pub fn mars_atmosphere_model(r: &Vec3) -> f64 {
    unsafe { ffi::MarsAtmosphereModel(r.as_ptr().cast_mut()) }
}

/// Dipole magnetic field model.
pub fn dipole_mag_field(
    dipole_moment: f64,
    dipole_axis: &Vec3,
    dipole_offset: &Vec3,
    pos: &Vec3,
    pri_mer_ang: f64,
) -> Vec3 {
    let mut mag_vec = Vec3::default();
    unsafe {
        ffi::DipoleMagField(
            dipole_moment,
            dipole_axis.as_ptr().cast_mut(),
            dipole_offset.as_ptr().cast_mut(),
            pos.as_ptr().cast_mut(),
            pri_mer_ang,
            mag_vec.as_mut_ptr(),
        )
    };
    mag_vec
}

/// Compute gravity gradient torque from gradient and inertia.
pub fn grav_grad_times_inertia(g: &Mat3, inertia: &Mat3) -> Vec3 {
    let mut ggxi = Vec3::default();
    unsafe {
        ffi::GravGradTimesInertia(
            g.as_ptr().cast_mut(),
            inertia.as_ptr().cast_mut(),
            ggxi.as_mut_ptr(),
        )
    };
    ggxi
}

/// Earth gravity model EGM96.
pub fn egm96(model_path: &CStr, n: i64, m: i64, mass: f64, pbn: &Vec3, pri_mer_ang: f64) -> Vec3 {
    let mut fgeo = Vec3::default();
    unsafe {
        ffi::EGM96(
            model_path.as_ptr(),
            n,
            m,
            mass,
            pbn.as_ptr().cast_mut(),
            pri_mer_ang,
            fgeo.as_mut_ptr(),
        )
    };
    fgeo
}

/// Mars gravity model GMM-2B.
pub fn gmm2b(model_path: &CStr, n: i64, m: i64, mass: f64, pbn: &Vec3, pri_mer_ang: f64) -> Vec3 {
    let mut fgeo = Vec3::default();
    unsafe {
        ffi::GMM2B(
            model_path.as_ptr(),
            n,
            m,
            mass,
            pbn.as_ptr().cast_mut(),
            pri_mer_ang,
            fgeo.as_mut_ptr(),
        )
    };
    fgeo
}

/// Lunar gravity model GLGM-2.
pub fn glgm2(model_path: &CStr, n: i64, m: i64, mass: f64, pbn: &Vec3, pri_mer_ang: f64) -> Vec3 {
    let mut fgeo = Vec3::default();
    unsafe {
        ffi::GLGM2(
            model_path.as_ptr(),
            n,
            m,
            mass,
            pbn.as_ptr().cast_mut(),
            pri_mer_ang,
            fgeo.as_mut_ptr(),
        )
    };
    fgeo
}

/// IGRF magnetic field model.
pub fn igrf_mag_field(model_path: &CStr, n: i64, m: i64, pbn: &Vec3, pri_mer_ang: f64) -> Vec3 {
    let mut mag_vec = Vec3::default();
    unsafe {
        ffi::IGRFMagField(
            model_path.as_ptr(),
            n,
            m,
            pbn.as_ptr().cast_mut(),
            pri_mer_ang,
            mag_vec.as_mut_ptr(),
        )
    };
    mag_vec
}

/// NRLMSISE-00 atmosphere density model.
pub fn nrlmsise00(
    year: i64,
    doy: i64,
    hour: i64,
    minute: i64,
    second: f64,
    pos_w: &Vec3,
    f10p7: f64,
    ap: f64,
) -> f64 {
    unsafe {
        ffi::NRLMSISE00(
            year,
            doy,
            hour,
            minute,
            second,
            pos_w.as_ptr().cast_mut(),
            f10p7,
            ap,
        )
    }
}

/// Simplified MSIS atmosphere model.
pub fn simple_msis(pbn: &Vec3, col: i64) -> f64 {
    unsafe { ffi::SimpleMSIS(pbn.as_ptr().cast_mut(), col) }
}

/// Jacchia-Roberts atmosphere density model.
pub fn jacchia_roberts(pbn: &Vec3, svn: &Vec3, f10p7: f64, ap: f64) -> f64 {
    unsafe { ffi::JacchiaRoberts(pbn.as_ptr().cast_mut(), svn.as_ptr().cast_mut(), f10p7, ap) }
}

/// Polyhedron gravity acceleration.
pub fn polyhedron_grav_acc(
    mesh: &mut ffi::MeshType,
    density: f64,
    pos_n: &Vec3,
    cwn: &Mat3,
) -> Option<Vec3> {
    let mut grav_acc = Vec3::default();
    let ok = unsafe {
        ffi::PolyhedronGravAcc(
            mesh,
            density,
            pos_n.as_ptr().cast_mut(),
            cwn.as_ptr().cast_mut(),
            grav_acc.as_mut_ptr(),
        )
    };
    if ok != 0 {
        Some(grav_acc)
    } else {
        None
    }
}

/// Polyhedron gravity gradient.
pub fn polyhedron_grav_grad(
    mesh: &mut ffi::MeshType,
    density: f64,
    pos_n: &Vec3,
    cwn: &Mat3,
) -> Option<Mat3> {
    let mut grad = Mat3::default();
    let ok = unsafe {
        ffi::PolyhedronGravGrad(
            mesh,
            density,
            pos_n.as_ptr().cast_mut(),
            cwn.as_ptr().cast_mut(),
            grad.as_mut_ptr(),
        )
    };
    if ok != 0 {
        Some(grad)
    } else {
        None
    }
}
