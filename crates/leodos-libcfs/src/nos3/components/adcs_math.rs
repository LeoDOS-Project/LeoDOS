//! Quaternion and vector math for ADCS.
//!
//! Three-dimensional vector operations and unit-quaternion
//! algebra used in attitude determination and control.

use crate::ffi;

/// Dot product of two 3-vectors.
pub fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    unsafe { ffi::VoV(a.as_ptr() as *mut _, b.as_ptr() as *mut _) }
}

/// Cross product: `c = a x b`.
pub fn cross(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    let mut c = [0.0; 3];
    unsafe {
        ffi::VxV(
            a.as_ptr() as *mut _,
            b.as_ptr() as *mut _,
            c.as_mut_ptr(),
        );
    }
    c
}

/// Scalar-vector multiply: `w = s * v`.
pub fn scale(s: f64, v: &[f64; 3]) -> [f64; 3] {
    let mut w = [0.0; 3];
    unsafe {
        ffi::SxV(s, v.as_ptr() as *mut _, w.as_mut_ptr());
    }
    w
}

/// Euclidean magnitude of a 3-vector.
pub fn magnitude(v: &[f64; 3]) -> f64 {
    unsafe { ffi::MAGV(v.as_ptr() as *mut _) }
}

/// Normalises a 3-vector in place.
pub fn normalise_vec(v: &mut [f64; 3]) {
    unsafe { ffi::UNITV(v.as_mut_ptr()) }
}

/// Copies `v` into `w`, normalised. Returns the original magnitude.
pub fn copy_unit(v: &[f64; 3]) -> ([f64; 3], f64) {
    let mut w = [0.0; 3];
    let mag = unsafe {
        ffi::CopyUnitV(v.as_ptr() as *mut _, w.as_mut_ptr())
    };
    (w, mag)
}

/// Quaternion product: `c = a * b`.
pub fn quat_mul(a: &[f64; 4], b: &[f64; 4]) -> [f64; 4] {
    let mut c = [0.0; 4];
    unsafe { ffi::QxQ(a.as_ptr(), b.as_ptr(), c.as_mut_ptr()) }
    c
}

/// Quaternion product with conjugate: `c = a * b^T`.
pub fn quat_mul_conj(a: &[f64; 4], b: &[f64; 4]) -> [f64; 4] {
    let mut c = [0.0; 4];
    unsafe { ffi::QxQT(a.as_ptr(), b.as_ptr(), c.as_mut_ptr()) }
    c
}

/// Rotates vector `vb` by quaternion `q`: `va = q * vb`.
pub fn quat_rotate(q: &[f64; 4], vb: &[f64; 3]) -> [f64; 3] {
    let mut va = [0.0; 3];
    unsafe {
        ffi::QxV(q.as_ptr(), vb.as_ptr(), va.as_mut_ptr());
    }
    va
}

/// Inverse rotation: `vb = q^T * va`.
pub fn quat_rotate_inv(
    q: &[f64; 4],
    va: &[f64; 3],
) -> [f64; 3] {
    let mut vb = [0.0; 3];
    unsafe {
        ffi::QTxV(q.as_ptr(), va.as_ptr(), vb.as_mut_ptr());
    }
    vb
}

/// Normalises a quaternion in place.
pub fn normalise_quat(q: &mut [f64; 4]) {
    unsafe { ffi::UNITQ(q.as_mut_ptr()) }
}

/// Ensures the scalar part of a quaternion is positive.
pub fn rectify_quat(q: &mut [f64; 4]) {
    unsafe { ffi::RECTIFYQ(q.as_mut_ptr()) }
}

/// Clamped arc-cosine (avoids NaN for values outside [-1, 1]).
pub fn safe_acos(x: f64) -> f64 {
    unsafe { ffi::arccos(x) }
}

/// Clamps `x` to `[min, max]`.
pub fn clamp(x: f64, min: f64, max: f64) -> f64 {
    unsafe { ffi::Limit(x, min, max) }
}
