extern crate alloc;

use crate::ffi;
use crate::types::{Mat3, Vec3, Vec4};
use alloc::vec;
use alloc::vec::Vec;

// --- 3x3 matrix operations ---

/// Computes the 3x3 matrix product A * B.
pub fn mxm(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::MxM(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Computes A * B^T (matrix times transpose).
pub fn mxmt(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::MxMT(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Computes A^T * B (transpose times matrix).
pub fn mtxm(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::MTxM(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Computes A^T * B^T (transpose times transpose).
pub fn mtxmt(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut c = Mat3::default();
    unsafe { ffi::MTxMT(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Returns the transpose of a 3x3 matrix.
pub fn transpose(a: &Mat3) -> Mat3 {
    let mut b = Mat3::default();
    unsafe { ffi::MT(a.as_ptr().cast_mut(), b.as_mut_ptr()) };
    b
}

/// Scales a 3x3 matrix by a scalar.
pub fn sxm(s: f64, a: &Mat3) -> Mat3 {
    let mut b = Mat3::default();
    unsafe { ffi::SxM(s, a.as_ptr().cast_mut(), b.as_mut_ptr()) };
    b
}

/// Computes the inverse of a 3x3 matrix.
pub fn minv3(a: &Mat3) -> Mat3 {
    let mut b = Mat3::default();
    unsafe { ffi::MINV3(a.as_ptr().cast_mut(), b.as_mut_ptr()) };
    b
}

/// Computes the inverse of a 2x2 matrix.
pub fn minv2(a: &[[f64; 2]; 2]) -> [[f64; 2]; 2] {
    let mut b = [[0.0; 2]; 2];
    unsafe { ffi::MINV2(a.as_ptr().cast_mut(), b.as_mut_ptr()) };
    b
}

/// Computes the pseudoinverse of a 4x3 matrix.
pub fn pinv4x3(a: &[[f64; 3]; 4]) -> [[f64; 4]; 3] {
    let mut aplus = [[0.0; 4]; 3];
    unsafe { ffi::PINV4x3(a.as_ptr().cast_mut(), aplus.as_mut_ptr()) };
    aplus
}

// --- Matrix-vector operations ---

/// Computes the matrix-vector product M * v.
pub fn mxv(m: &Mat3, v: &Vec3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe { ffi::MxV(m.as_ptr().cast_mut(), v.as_ptr().cast_mut(), w.as_mut_ptr()) };
    w
}

/// Computes M^T * v (transpose times vector).
pub fn mtxv(m: &Mat3, v: &Vec3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe { ffi::MTxV(m.as_ptr().cast_mut(), v.as_ptr().cast_mut(), w.as_mut_ptr()) };
    w
}

/// Computes v * M (row vector times matrix).
pub fn vxm(v: &Vec3, m: &Mat3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe { ffi::VxM(v.as_ptr().cast_mut(), m.as_ptr().cast_mut(), w.as_mut_ptr()) };
    w
}

/// Computes v * M^T (row vector times transpose).
pub fn vxmt(v: &Vec3, m: &Mat3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe { ffi::VxMT(v.as_ptr().cast_mut(), m.as_ptr().cast_mut(), w.as_mut_ptr()) };
    w
}

// --- Vector operations ---

/// Computes the dot product of two 3-vectors.
pub fn dot(a: &Vec3, b: &Vec3) -> f64 {
    unsafe { ffi::VoV(a.as_ptr().cast_mut(), b.as_ptr().cast_mut()) }
}

/// Computes the cross product a x b.
pub fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
    let mut c = Vec3::default();
    unsafe { ffi::VxV(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Returns the magnitude (Euclidean norm) of a 3-vector.
pub fn mag(v: &Vec3) -> f64 {
    unsafe { ffi::MAGV(v.as_ptr().cast_mut()) }
}

/// Scales a 3-vector by a scalar.
pub fn sxv(s: f64, v: &Vec3) -> Vec3 {
    let mut w = Vec3::default();
    unsafe { ffi::SxV(s, v.as_ptr().cast_mut(), w.as_mut_ptr()) };
    w
}

/// Normalizes a vector in place, returning the unit vector and its original magnitude.
pub fn unit(v: &Vec3) -> (Vec3, f64) {
    let mut w = *v;
    let mag = unsafe { ffi::UNITV(w.as_mut_ptr()) };
    (w, mag)
}

/// Returns a copy of the unit vector and the original magnitude without modifying the input.
pub fn copy_unit(v: &Vec3) -> (Vec3, f64) {
    let mut w = Vec3::default();
    let mag = unsafe { ffi::CopyUnitV(v.as_ptr().cast_mut(), w.as_mut_ptr()) };
    (w, mag)
}

/// Constructs two vectors perpendicular to the given vector, forming an orthonormal basis.
pub fn perp_basis(a: &Vec3) -> (Vec3, Vec3) {
    let mut b = Vec3::default();
    let mut c = Vec3::default();
    unsafe { ffi::PerpBasis(a.as_ptr().cast_mut(), b.as_mut_ptr(), c.as_mut_ptr()) };
    (b, c)
}

/// Computes the unit normal of the triangle defined by three vertices.
pub fn find_normal(v1: &Vec3, v2: &Vec3, v3: &Vec3) -> Vec3 {
    let mut n = Vec3::default();
    unsafe {
        ffi::FindNormal(
            v1.as_ptr().cast_mut(),
            v2.as_ptr().cast_mut(),
            v3.as_ptr().cast_mut(),
            n.as_mut_ptr(),
        )
    };
    n
}

/// Builds the skew-symmetric (cross-product) matrix from a 3-vector.
pub fn skew_matrix(v: &Vec3) -> Mat3 {
    let mut m = Mat3::default();
    unsafe { ffi::V2CrossM(v.as_ptr().cast_mut(), m.as_mut_ptr()) };
    m
}

/// Converts a 3-vector to longitude and latitude angles.
pub fn vec_to_lng_lat(a: &Vec3) -> (f64, f64) {
    let mut lng = 0.0;
    let mut lat = 0.0;
    unsafe { ffi::VecToLngLat(a.as_ptr().cast_mut(), &mut lng, &mut lat) };
    (lng, lat)
}

// --- Quaternion operations ---

/// Computes the quaternion product a * b.
pub fn qxq(a: &Vec4, b: &Vec4) -> Vec4 {
    let mut c = Vec4::default();
    unsafe { ffi::QxQ(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Computes a^* * b (conjugate of a times b).
pub fn qtxq(a: &Vec4, b: &Vec4) -> Vec4 {
    let mut c = Vec4::default();
    unsafe { ffi::QTxQ(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Computes a * b^* (quaternion times conjugate of b).
pub fn qxqt(a: &Vec4, b: &Vec4) -> Vec4 {
    let mut c = Vec4::default();
    unsafe { ffi::QxQT(a.as_ptr().cast_mut(), b.as_ptr().cast_mut(), c.as_mut_ptr()) };
    c
}

/// Rotates a vector by a quaternion (forward rotation).
pub fn vxq(va: &Vec3, qab: &Vec4) -> Vec3 {
    let mut vb = Vec3::default();
    unsafe {
        ffi::VxQ(
            va.as_ptr().cast_mut(),
            qab.as_ptr().cast_mut(),
            vb.as_mut_ptr(),
        )
    };
    vb
}

/// Rotates a vector by a quaternion (quaternion-first convention).
pub fn qxv(qab: &Vec4, vb: &Vec3) -> Vec3 {
    let mut va = Vec3::default();
    unsafe {
        ffi::QxV(
            qab.as_ptr().cast_mut(),
            vb.as_ptr().cast_mut(),
            va.as_mut_ptr(),
        )
    };
    va
}

/// Rotates a vector by the conjugate of a quaternion (inverse rotation).
pub fn qtxv(qab: &Vec4, va: &Vec3) -> Vec3 {
    let mut vb = Vec3::default();
    unsafe {
        ffi::QTxV(
            qab.as_ptr().cast_mut(),
            va.as_ptr().cast_mut(),
            vb.as_mut_ptr(),
        )
    };
    vb
}

/// Normalizes a quaternion to unit length.
pub fn unitq(q: &Vec4) -> Vec4 {
    let mut out = *q;
    unsafe { ffi::UNITQ(out.as_mut_ptr()) };
    out
}

/// Rectifies a quaternion so that the scalar part is non-negative.
pub fn rectifyq(q: &Vec4) -> Vec4 {
    let mut out = *q;
    unsafe { ffi::RECTIFYQ(out.as_mut_ptr()) };
    out
}

/// Spherical linear interpolation (SLERP) between two quaternions at parameter u in [0, 1].
pub fn slerp(q1: &Vec4, q2: &Vec4, u: f64) -> Vec4 {
    let mut q = Vec4::default();
    unsafe {
        ffi::SphereInterp(
            q1.as_ptr().cast_mut(),
            q2.as_ptr().cast_mut(),
            u,
            q.as_mut_ptr(),
        )
    };
    q
}

// --- Scalar functions ---

/// Returns the sign of x (-1, 0, or +1).
pub fn signum(x: f64) -> f64 {
    unsafe { ffi::signum(x) }
}

/// Computes sinc(x) = sin(x)/x, with sinc(0) = 1.
pub fn sinc(x: f64) -> f64 {
    unsafe { ffi::sinc(x) }
}

/// Computes n! (factorial).
pub fn factorial(n: i64) -> f64 {
    unsafe { ffi::fact(n) }
}

/// Computes n!! (double factorial / odd factorial).
pub fn double_factorial(n: i64) -> f64 {
    unsafe { ffi::oddfact(n) }
}

// --- Interpolation ---

/// Cubic interpolation in one dimension between f0 and f1 at parameter x.
pub fn cubic_interp_1d(f0: f64, f1: f64, x: f64) -> f64 {
    unsafe { ffi::CubicInterp1D(f0, f1, x) }
}

/// Cubic interpolation in two dimensions over four corner values at (x, y).
pub fn cubic_interp_2d(f00: f64, f10: f64, f01: f64, f11: f64, x: f64, y: f64) -> f64 {
    unsafe { ffi::CubicInterp2D(f00, f10, f01, f11, x, y) }
}

/// Evaluates a cubic spline at x given four (x, y) knot pairs.
pub fn cubic_spline(x: f64, knots_x: &mut [f64; 4], knots_y: &mut [f64; 4]) -> f64 {
    unsafe { ffi::CubicSpline(x, knots_x.as_mut_ptr(), knots_y.as_mut_ptr()) }
}

// --- Step functions ---

/// Heaviside step function: returns 0 if x < a, else 1.
pub fn step(a: f64, x: f64) -> f64 {
    unsafe { ffi::Step(a, x) }
}

/// Clamps x to the interval [a, b].
pub fn clamp(a: f64, b: f64, x: f64) -> f64 {
    unsafe { ffi::Clamp(a, b, x) }
}

/// Linear ramp from 0 to 1 over the interval [a, b].
pub fn ramp_step(a: f64, b: f64, x: f64) -> f64 {
    unsafe { ffi::RampStep(a, b, x) }
}

/// Smooth cubic step (Hermite) from 0 to 1 over the interval [a, b].
pub fn cubic_step(a: f64, b: f64, x: f64) -> f64 {
    unsafe { ffi::CubicStep(a, b, x) }
}

/// 2D pseudo-random noise function.
pub fn prn_2d(x: i64, y: i64) -> f64 {
    unsafe { ffi::PRN2D(x, y) }
}

/// 3D pseudo-random noise function.
pub fn prn_3d(x: i64, y: i64, z: i64) -> f64 {
    unsafe { ffi::PRN3D(x, y, z) }
}

// --- Additional matrix/vector operations ---

/// Computes the inverse of a 4x4 matrix.
pub fn minv4(a: &[[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let mut b = [[0.0; 4]; 4];
    unsafe { ffi::MINV4(a.as_ptr().cast_mut(), b.as_mut_ptr()) };
    b
}

/// Builds the double cross-product matrix [v x]^2 from a 3-vector.
pub fn double_skew_matrix(v: &Vec3) -> Mat3 {
    let mut m = Mat3::default();
    unsafe { ffi::V2DoubleCrossM(v.as_ptr().cast_mut(), m.as_mut_ptr()) };
    m
}

/// Computes [v x] * M (skew-symmetric matrix of v times M).
pub fn vcross_m(v: &Vec3, m: &Mat3) -> Mat3 {
    let mut a = Mat3::default();
    unsafe { ffi::VcrossM(v.as_ptr().cast_mut(), m.as_ptr().cast_mut(), a.as_mut_ptr()) };
    a
}

/// Computes [v x] * M^T (skew-symmetric matrix of v times transpose of M).
pub fn vcross_mt(v: &Vec3, m: &Mat3) -> Mat3 {
    let mut a = Mat3::default();
    unsafe { ffi::VcrossMT(v.as_ptr().cast_mut(), m.as_ptr().cast_mut(), a.as_mut_ptr()) };
    a
}

/// Computes v x (M * v) — cross product of v with M applied to v.
pub fn vx_mov(w: &Vec3, m: &Mat3) -> Vec3 {
    let mut result = Vec3::default();
    unsafe {
        ffi::vxMov(
            w.as_ptr().cast_mut(),
            m.as_ptr().cast_mut(),
            result.as_mut_ptr(),
        )
    };
    result
}

// --- Interpolation (extended) ---

/// Cubic interpolation in three dimensions over eight corner values at (x, y, z).
pub fn cubic_interp_3d(
    f000: f64,
    f100: f64,
    f010: f64,
    f110: f64,
    f001: f64,
    f101: f64,
    f011: f64,
    f111: f64,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    unsafe { ffi::CubicInterp3D(f000, f100, f010, f110, f001, f101, f011, f111, x, y, z) }
}

/// Piecewise linear interpolation of tabulated (x, y) data at the given x.
pub fn lin_interp(data_x: &mut [f64], data_y: &mut [f64], x: f64) -> f64 {
    let n = data_x.len().min(data_y.len()) as i64;
    unsafe { ffi::LinInterp(data_x.as_mut_ptr(), data_y.as_mut_ptr(), x, n) }
}

// --- Geometry ---

/// Computes the shortest distance from a point to a line segment, and the perpendicular vector.
pub fn distance_to_line(end1: &Vec3, end2: &Vec3, point: &Vec3) -> (f64, Vec3) {
    let mut vec_to_line = Vec3::default();
    let d = unsafe {
        ffi::DistanceToLine(
            end1.as_ptr().cast_mut(),
            end2.as_ptr().cast_mut(),
            point.as_ptr().cast_mut(),
            vec_to_line.as_mut_ptr(),
        )
    };
    (d, vec_to_line)
}

/// Projects a point onto a triangle along a direction, returning the projection and barycentric coordinates.
pub fn project_point_onto_triangle(
    a: &Vec3,
    b: &Vec3,
    c: &Vec3,
    dir: &Vec3,
    pt: &Vec3,
) -> Option<(Vec3, [f64; 4])> {
    let mut proj = Vec3::default();
    let mut bary = [0.0; 4];
    let ok = unsafe {
        ffi::ProjectPointOntoTriangle(
            a.as_ptr().cast_mut(),
            b.as_ptr().cast_mut(),
            c.as_ptr().cast_mut(),
            dir.as_ptr().cast_mut(),
            pt.as_ptr().cast_mut(),
            proj.as_mut_ptr(),
            bary.as_mut_ptr(),
        )
    };
    if ok != 0 {
        Some((proj, bary))
    } else {
        None
    }
}

// --- Spherical harmonics ---

/// Computes associated Legendre polynomials P(n,m) and their scaled derivatives at x.
pub fn legendre(n: i64, m: i64, x: f64) -> ([[f64; 19]; 19], [[f64; 19]; 19]) {
    let mut p = [[0.0f64; 19]; 19];
    let mut sdp = [[0.0f64; 19]; 19];
    unsafe { ffi::Legendre(n, m, x, p.as_mut_ptr(), sdp.as_mut_ptr()) };
    (p, sdp)
}

/// Evaluates a spherical harmonic gravity field and returns the gradient vector.
pub fn spherical_harmonics(
    n: i64,
    m: i64,
    r: f64,
    phi: f64,
    theta: f64,
    re: f64,
    k: f64,
    c_coeffs: &mut [[f64; 19]; 19],
    s_coeffs: &mut [[f64; 19]; 19],
) -> Vec3 {
    let mut grad_v = Vec3::default();
    unsafe {
        ffi::SphericalHarmonics(
            n,
            m,
            r,
            phi,
            theta,
            re,
            k,
            c_coeffs.as_mut_ptr(),
            s_coeffs.as_mut_ptr(),
            grad_v.as_mut_ptr(),
        )
    };
    grad_v
}

// --- Chebyshev ---

/// Computes Chebyshev polynomials T and U of the first and second kind up to degree n at u.
pub fn cheby_polys(u: f64, n: i64) -> ([f64; 20], [f64; 20]) {
    let mut t = [0.0; 20];
    let mut u_arr = [0.0; 20];
    unsafe { ffi::ChebyPolys(u, n, t.as_mut_ptr(), u_arr.as_mut_ptr()) };
    (t, u_arr)
}

/// Evaluates a Chebyshev interpolant and its derivative from precomputed polynomials and coefficients.
pub fn cheby_interp(
    t: &mut [f64; 20],
    u: &mut [f64; 20],
    coef: &mut [f64; 20],
    n: i64,
) -> (f64, f64) {
    let mut p = 0.0;
    let mut dp = 0.0;
    unsafe {
        ffi::ChebyInterp(
            t.as_mut_ptr(),
            u.as_mut_ptr(),
            coef.as_mut_ptr(),
            n,
            &mut p,
            &mut dp,
        )
    };
    (p, dp)
}

/// Fits Chebyshev coefficients to tabulated (u, p) data with nc terms.
pub fn find_cheby_coefs(u: &mut [f64], p: &mut [f64], nc: i64) -> [f64; 20] {
    let nu = u.len().min(p.len()) as i64;
    let mut coef = [0.0; 20];
    unsafe { ffi::FindChebyCoefs(u.as_mut_ptr(), p.as_mut_ptr(), nu, nc, coef.as_mut_ptr()) };
    coef
}

// --- Solvers (note: these take double** so use fixed-size) ---

/// Computes the inverse of a 6x6 matrix using an optimized algorithm.
pub fn fast_minv6(a: &[[f64; 6]; 6]) -> [[f64; 6]; 6] {
    let mut ai = [[0.0; 6]; 6];
    unsafe { ffi::FastMINV6(a.as_ptr().cast_mut(), ai.as_mut_ptr(), 6) };
    ai
}

/// Finds all roots of a polynomial using Bairstow's method, returning (real, imaginary) parts.
pub fn bairstow(coeffs: &mut [f64], tol: f64) -> (Vec<f64>, Vec<f64>) {
    let n = coeffs.len() as i64;
    let mut real = vec![0.0; n as usize];
    let mut imag = vec![0.0; n as usize];
    unsafe {
        ffi::Bairstow(
            n,
            coeffs.as_mut_ptr(),
            tol,
            real.as_mut_ptr(),
            imag.as_mut_ptr(),
        )
    };
    (real, imag)
}
