use crate::ffi;
use core::ptr::NonNull;

// --- Random process ---

/// Owned handle to a 42 random process (PRNG state).
pub struct RandomProcess {
    inner: NonNull<ffi::RandomProcessType>,
}

impl RandomProcess {
    /// Creates a new random process with the given seed.
    pub fn new(seed: i64) -> Self {
        let ptr = unsafe { ffi::CreateRandomProcess(seed) };
        Self {
            inner: NonNull::new(ptr).expect("CreateRandomProcess returned null"),
        }
    }

    /// Returns a uniform random sample in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        unsafe { ffi::UniformRandom(self.inner.as_ptr()) }
    }

    /// Returns a Gaussian (normal) random sample.
    pub fn gaussian(&mut self) -> f64 {
        unsafe { ffi::GaussianRandom(self.inner.as_ptr()) }
    }
}

impl Drop for RandomProcess {
    fn drop(&mut self) {
        unsafe { ffi::DestroyRandomProcess(self.inner.as_ptr()) }
    }
}

// --- Filters ---

/// Owned handle to a 42 IIR filter (general or first/second order).
pub struct Filter {
    inner: NonNull<ffi::FilterType>,
}

impl Filter {
    /// Creates a general IIR filter with the given coefficients.
    pub fn general(ns: i64, a: &mut [f64], b: &mut [f64], dx_max: f64, y_min: f64) -> Self {
        let ptr =
            unsafe { ffi::CreateGeneralFilter(ns, a.as_mut_ptr(), b.as_mut_ptr(), dx_max, y_min) };
        Self {
            inner: NonNull::new(ptr).expect("CreateGeneralFilter returned null"),
        }
    }

    /// Creates a first-order lowpass filter.
    pub fn first_order_lowpass(w: f64, t: f64, dx_max: f64, y_min: f64) -> Self {
        let ptr = unsafe { ffi::CreateFirstOrderLowpassFilter(w, t, dx_max, y_min) };
        Self {
            inner: NonNull::new(ptr).expect("CreateFirstOrderLowpassFilter returned null"),
        }
    }

    /// Creates a first-order highpass filter.
    pub fn first_order_highpass(w: f64, t: f64, dx_max: f64, y_min: f64) -> Self {
        let ptr = unsafe { ffi::CreateFirstOrderHighpassFilter(w, t, dx_max, y_min) };
        Self {
            inner: NonNull::new(ptr).expect("CreateFirstOrderHighpassFilter returned null"),
        }
    }

    /// Creates a second-order lowpass filter.
    pub fn second_order_lowpass(w: f64, z: f64, t: f64, dx_max: f64, y_min: f64) -> Self {
        let ptr = unsafe { ffi::CreateSecondOrderLowpassFilter(w, z, t, dx_max, y_min) };
        Self {
            inner: NonNull::new(ptr).expect("CreateSecondOrderLowpassFilter returned null"),
        }
    }

    /// Creates a second-order highpass filter.
    pub fn second_order_highpass(w: f64, z: f64, t: f64, dx_max: f64, y_min: f64) -> Self {
        let ptr = unsafe { ffi::CreateSecondOrderHighpassFilter(w, z, t, dx_max, y_min) };
        Self {
            inner: NonNull::new(ptr).expect("CreateSecondOrderHighpassFilter returned null"),
        }
    }

    /// Applies the general filter to input `x` and returns the output.
    pub fn apply_general(&mut self, x: f64) -> f64 {
        unsafe { ffi::GeneralFilter(self.inner.as_ptr(), x) }
    }

    /// Applies the first-order lowpass filter to input `x`.
    pub fn apply_first_order_lowpass(&mut self, x: f64) -> f64 {
        unsafe { ffi::FirstOrderLowpassFilter(self.inner.as_ptr(), x) }
    }

    /// Applies the first-order highpass filter to input `x`.
    pub fn apply_first_order_highpass(&mut self, x: f64) -> f64 {
        unsafe { ffi::FirstOrderHighpassFilter(self.inner.as_ptr(), x) }
    }

    /// Applies the second-order lowpass filter to input `x`.
    pub fn apply_second_order_lowpass(&mut self, x: f64) -> f64 {
        unsafe { ffi::SecondOrderLowpassFilter(self.inner.as_ptr(), x) }
    }

    /// Applies the second-order highpass filter to input `x`.
    pub fn apply_second_order_highpass(&mut self, x: f64) -> f64 {
        unsafe { ffi::SecondOrderHighpassFilter(self.inner.as_ptr(), x) }
    }
}

impl Drop for Filter {
    fn drop(&mut self) {
        unsafe { ffi::DestroyFilter(self.inner.as_ptr()) }
    }
}

// --- Delay ---

/// Owned handle to a 42 circular-buffer delay line.
pub struct Delay {
    inner: NonNull<ffi::DelayType>,
}

impl Delay {
    /// Creates a delay line with the given delay time and timestep.
    pub fn new(delay_time: f64, dt: f64) -> Self {
        let ptr = unsafe { ffi::CreateDelay(delay_time, dt) };
        Self {
            inner: NonNull::new(ptr).expect("CreateDelay returned null"),
        }
    }

    /// Resizes the delay buffer for a new delay time and timestep.
    pub fn resize(&mut self, delay_time: f64, dt: f64) {
        let ptr = unsafe { ffi::ResizeDelay(self.inner.as_ptr(), delay_time, dt) };
        if let Some(new) = NonNull::new(ptr) {
            self.inner = new;
        }
    }

    /// Pushes `x` into the delay line and returns the delayed output.
    pub fn apply(&mut self, x: f64) -> f64 {
        unsafe { ffi::Delay(self.inner.as_ptr(), x) }
    }
}

// Note: no DestroyDelay in 42's API — it uses malloc
// internally but provides no cleanup function. The
// CircBuffer will leak when Delay is dropped.
