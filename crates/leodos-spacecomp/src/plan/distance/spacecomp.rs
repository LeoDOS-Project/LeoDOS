//! SpaceCoMP cost model from the paper.
//!
//! Computes task-to-processor cost using the formula from Section IV-B:
//!
//! ```text
//! C(t,p) = processing_time + hops × hop_overhead + S(distance, volume)
//! ```
//!
//! Where S is the transmission time based on Shannon capacity:
//!
//! ```text
//! S(d,V) = d/c + V / (B × log2(1 + SNR(d)))
//!        = propagation_delay + transfer_time
//! ```
//!
//! SNR is computed using the Friis transmission formula and free-space path loss:
//!
//! ```text
//! SNR(d) = P_rx / N
//! P_rx = (P_tx × G_tx × G_rx) / FSPL(d)
//! FSPL(d) = (4πd/λ)²
//! N = k_B × T × B
//! ```

use leodos_protocols::network::isl::torus::{Point, Torus};

use super::CostModel;

/// SpaceCoMP cost model with configurable link parameters.
#[derive(Debug, Clone, Copy)]
pub struct SpaceCompCost {
    /// Time to process data locally (microseconds).
    pub processing_time_us: u64,
    /// Overhead per hop for routing/forwarding (microseconds).
    pub hop_overhead_us: u64,
    /// Physical distance between adjacent satellites (meters).
    pub inter_satellite_distance_m: u64,
    /// ISL channel bandwidth (Hz). Typical: 10 GHz.
    pub bandwidth_hz: u64,
    /// Transmit power (Watts).
    pub tx_power_w: f32,
    /// Transmitter antenna gain (linear, not dB).
    pub tx_gain: f32,
    /// Receiver antenna gain (linear, not dB).
    pub rx_gain: f32,
    /// Laser wavelength (meters). Typical: 1550nm = 1550e-9.
    pub wavelength_m: f32,
    /// System noise temperature (Kelvin).
    pub noise_temp_k: f32,
    /// Data volume to transfer from collector to processor (bytes).
    pub data_volume_bytes: u64,
}

impl Default for SpaceCompCost {
    fn default() -> Self {
        Self {
            processing_time_us: 1000,
            hop_overhead_us: 100,
            inter_satellite_distance_m: 1_000_000,
            bandwidth_hz: 10_000_000_000,
            tx_power_w: 5.0,
            tx_gain: 1000.0,
            rx_gain: 1000.0,
            wavelength_m: 1550e-9,
            noise_temp_k: 300.0,
            data_volume_bytes: 10_000_000_000,
        }
    }
}

impl SpaceCompCost {
    const SPEED_OF_LIGHT_M_PER_S: f32 = 299_792_458.0;
    const BOLTZMANN_J_PER_K: f32 = 1.38e-23;
    const PI: f32 = core::f32::consts::PI;

    /// Free-space path loss: FSPL(d) = (4πd/λ)²
    ///
    /// Models signal attenuation due to spreading over distance.
    fn fspl(&self, distance_m: f32) -> f32 {
        let term = (4.0 * Self::PI * distance_m) / self.wavelength_m;
        term * term
    }

    /// Received power using Friis formula: P_rx = (P_tx × G_tx × G_rx) / FSPL
    fn received_power(&self, distance_m: f32) -> f32 {
        (self.tx_power_w * self.tx_gain * self.rx_gain) / self.fspl(distance_m)
    }

    /// Thermal noise power: N = k_B × T × B (Boltzmann-Nyquist formula)
    fn noise_power(&self) -> f32 {
        Self::BOLTZMANN_J_PER_K * self.noise_temp_k * self.bandwidth_hz as f32
    }

    /// Signal-to-noise ratio: SNR = P_rx / N
    fn snr(&self, distance_m: f32) -> f32 {
        self.received_power(distance_m) / self.noise_power()
    }

    /// Transmission time: S(d,V) = d/c + V / (B × log2(1 + SNR))
    ///
    /// First term is propagation delay (speed of light).
    /// Second term is transfer time based on Shannon channel capacity.
    fn transmission_time_us(&self, distance_m: f32, volume_bytes: u64) -> u64 {
        let propagation_us = (distance_m / Self::SPEED_OF_LIGHT_M_PER_S) * 1e6;

        let snr = self.snr(distance_m);
        let capacity_bps = self.bandwidth_hz as f32 * libm::log2f(1.0 + snr);
        let volume_bits = volume_bytes as f32 * 8.0;
        let transfer_s = volume_bits / capacity_bps;
        let transfer_us = transfer_s * 1e6;

        (propagation_us + transfer_us) as u64
    }
}

impl CostModel for SpaceCompCost {
    type Cost = u64;

    fn cost(&self, torus: &Torus, task: Point, processor: Point) -> u64 {
        let dx = torus.distance_sat(task, processor).min(torus.distance_sat(processor, task));
        let dy = torus.distance_orb(task, processor).min(torus.distance_orb(processor, task));
        let hops = (dx + dy) as u64;

        if hops == 0 {
            return self.processing_time_us;
        }

        let distance_m = hops * self.inter_satellite_distance_m;
        let transmission_us = self.transmission_time_us(distance_m as f32, self.data_volume_bytes);

        self.processing_time_us
            .saturating_add(hops.saturating_mul(self.hop_overhead_us))
            .saturating_add(transmission_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_location_only_processing() {
        let torus = Torus::new(4, 4);
        let cost_model = SpaceCompCost::default();
        let p = Point::new(1, 1);
        assert_eq!(cost_model.cost(&torus, p, p), cost_model.processing_time_us);
    }

    #[test]
    fn test_cost_increases_with_distance() {
        let torus = Torus::new(8, 8);
        let cost_model = SpaceCompCost {
            data_volume_bytes: 1_000_000,
            inter_satellite_distance_m: 100_000,
            ..Default::default()
        };
        let origin = Point::new(0, 0);
        let near = Point::new(0, 1);
        let far = Point::new(0, 3);

        let cost_near = cost_model.cost(&torus, origin, near);
        let cost_far = cost_model.cost(&torus, origin, far);

        assert!(cost_far > cost_near, "far={} should be > near={}", cost_far, cost_near);
    }

    #[test]
    fn test_snr_decreases_with_distance() {
        let cost_model = SpaceCompCost::default();
        let snr_near = cost_model.snr(1000.0);
        let snr_far = cost_model.snr(10000.0);
        assert!(snr_near > snr_far);
    }
}
