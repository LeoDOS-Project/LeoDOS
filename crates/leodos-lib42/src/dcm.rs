use crate::ffi;
use crate::types::{Mat3, Vec3, Vec4};

/// Convert a direction cosine matrix to a quaternion.
pub fn dcm_to_quat(c: &Mat3) -> Vec4 {
    let mut q = Vec4::default();
    unsafe { ffi::C2Q(c.as_ptr().cast_mut(), q.as_mut_ptr()) };
    q
}

/// Convert a quaternion to a direction cosine matrix.
pub fn quat_to_dcm(q: &Vec4) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::Q2C(q.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Convert three Euler angles to a direction cosine matrix.
pub fn euler_to_dcm(seq: i64, th1: f64, th2: f64, th3: f64) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::A2C(seq, th1, th2, th3, c.as_mut_ptr()) };
    c
}

/// Extract three Euler angles from a direction cosine matrix.
pub fn dcm_to_euler(seq: i64, c: &Mat3) -> (f64, f64, f64) {
    let mut th1 = 0.0;
    let mut th2 = 0.0;
    let mut th3 = 0.0;
    unsafe { ffi::C2A(seq, c.as_ptr().cast_mut(), &mut th1, &mut th2, &mut th3) };
    (th1, th2, th3)
}

/// Build a DCM for a single rotation about an arbitrary axis.
pub fn simple_rotation(axis: &Vec3, theta: f64) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::SimpRot(axis.as_ptr().cast_mut(), theta, c.as_mut_ptr()) };
    c
}

/// Convert a quaternion to an angle-axis (Gibbs) vector.
pub fn quat_to_angle_vec(q: &Vec4) -> Vec3 {
    let mut angle_vec = Vec3::default();
    unsafe { ffi::Q2AngleVec(q.as_ptr().cast_mut(), angle_vec.as_mut_ptr()) };
    angle_vec
}

/// Compute quaternion time-derivative from angular velocity.
pub fn quat_rate(q: &Vec4, w: &Vec3) -> Vec4 {
    let mut qdot = Vec4::default();
    unsafe {
        ffi::QW2QDOT(
            q.as_ptr().cast_mut(),
            w.as_ptr().cast_mut(),
            qdot.as_mut_ptr(),
        )
    };
    qdot
}

/// Compute DCM time-derivative from angular velocity.
pub fn dcm_rate(w: &Vec3, c: &Mat3) -> Mat3 {
    let mut cdot = Mat3::default();
    unsafe {
        ffi::W2CDOT(
            w.as_ptr().cast_mut(),
            c.as_ptr().cast_mut(),
            cdot.as_mut_ptr(),
        )
    };
    cdot
}

/// Extract angular velocity from a DCM and its time-derivative.
pub fn dcm_rate_to_omega(c: &Mat3, cdot: &Mat3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe {
        ffi::CDOT2W(
            c.as_ptr().cast_mut(),
            cdot.as_ptr().cast_mut(),
            w.as_mut_ptr(),
        )
    };
    w
}

/// Convert Euler angle rates to angular velocity.
pub fn euler_rate_to_omega(seq: i64, ang: &Vec3, u: &Vec3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe {
        ffi::ADOT2W(
            0,
            seq,
            ang.as_ptr().cast_mut(),
            u.as_ptr().cast_mut(),
            w.as_mut_ptr(),
        )
    };
    w
}

/// Convert angular velocity to Euler angle rates.
pub fn omega_to_euler_rate(seq: i64, ang: &Vec3, w: &Vec3) -> Vec3 {
    let mut adot = Vec3::default();
    unsafe {
        ffi::W2ADOT(
            seq,
            ang.as_ptr().cast_mut(),
            w.as_ptr().cast_mut(),
            adot.as_mut_ptr(),
        )
    };
    adot
}

/// Translate a moment-of-inertia tensor via the parallel axis theorem.
pub fn parallel_axis(ib: &Mat3, cba: &Mat3, m: f64, offset: &Vec3) -> Mat3 {
    let mut iba = Mat3::default();
    unsafe {
        ffi::PARAXIS(
            ib.as_ptr().cast_mut(),
            cba.as_ptr().cast_mut(),
            m,
            offset.as_ptr().cast_mut(),
            iba.as_mut_ptr(),
        )
    };
    iba
}

/// Compute principal moments of inertia and the principal-axis DCM.
pub fn principal_moi(ib: &Mat3) -> (Vec3, Mat3) {
    let mut ip = Vec3::default();
    let mut cpb = Mat3::default();
    unsafe { ffi::PrincipalMOI(ib.as_ptr().cast_mut(), ip.as_mut_ptr(), cpb.as_mut_ptr()) };
    (ip, cpb)
}

/// Compute joint partial derivative matrices.
pub fn joint_partials(
    init: bool,
    is_spherical: bool,
    rot_seq: i64,
    trn_seq: i64,
    ang: &Vec3,
    sig: &Vec3,
) -> JointPartialsResult {
    let mut gamma = Mat3::default();
    let mut gs = Vec3::default();
    let mut gds = Vec3::default();
    let mut s = Vec3::default();
    let mut delta = Mat3::default();
    let mut ds = Vec3::default();
    let mut dds = Vec3::default();
    unsafe {
        ffi::JointPartials(
            init as i64,
            is_spherical as i64,
            rot_seq,
            trn_seq,
            ang.as_ptr().cast_mut(),
            sig.as_ptr().cast_mut(),
            gamma.as_mut_ptr(),
            gs.as_mut_ptr(),
            gds.as_mut_ptr(),
            s.as_mut_ptr(),
            delta.as_mut_ptr(),
            ds.as_mut_ptr(),
            dds.as_mut_ptr(),
        )
    };
    JointPartialsResult {
        gamma,
        gs,
        gds,
        s,
        delta,
        ds,
        dds,
    }
}

/// Output of [`joint_partials`] — rotation and translation partial derivatives.
#[derive(Debug, Clone, Copy)]
pub struct JointPartialsResult {
    /// Rotational partial derivative matrix.
    pub gamma: Mat3,
    /// Rotational sine terms.
    pub gs: Vec3,
    /// Derivative of rotational sine terms.
    pub gds: Vec3,
    /// Translational sine terms.
    pub s: Vec3,
    /// Translational partial derivative matrix.
    pub delta: Mat3,
    /// Translational direction sines.
    pub ds: Vec3,
    /// Derivative of translational direction sines.
    pub dds: Vec3,
}

/// Extract angular velocity from quaternion and its rate.
pub fn quat_to_omega(q: &Vec4, qdot: &Vec4) -> Vec3 {
    let mut w = Vec3::default();
    unsafe {
        ffi::Q2W(
            q.as_ptr().cast_mut(),
            qdot.as_ptr().cast_mut(),
            w.as_mut_ptr(),
        )
    };
    w
}
