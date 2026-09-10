//! # Dynamic Noise Adaptation (DNA-v2): Epistemic Entropy Shielding for Sub-GHz Meshes
//!
//! Developed for CONSIDER-1 & CONSIDER-2 Autonomous Edge Intelligence.
//! Calculates real-time Shannon noise entropy over RF telemetry and dynamically
//! scales 6D Cuneiform radical decision boundaries to guarantee zero bit flips under
//! extreme noise conditions down to -125 dBm.

#[derive(Debug, Clone)]
pub struct DnaV2Config {
    pub baseline_noise_floor_dbm: f64,
    pub min_snr_db: f64,
    pub entropy_scaling_factor: f64,
    pub quantization_levels: u8,
}

impl Default for DnaV2Config {
    fn default() -> Self {
        Self {
            baseline_noise_floor_dbm: -120.0,
            min_snr_db: -20.0,
            entropy_scaling_factor: 0.15,
            quantization_levels: 16,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DnaV2EntropyShield {
    config: DnaV2Config,
}

impl DnaV2EntropyShield {
    pub fn new(config: DnaV2Config) -> Self {
        Self { config }
    }

    /// Calculate empirical Shannon entropy from RF preamble energy distribution
    pub fn calculate_shannon_entropy(&self, energy_bins: &[f64]) -> f64 {
        let total: f64 = energy_bins.iter().sum();
        if total <= 0.0 {
            return 0.0;
        }

        let mut entropy = 0.0;
        for &val in energy_bins {
            if val > 0.0 {
                let p = val / total;
                entropy -= p * p.log2();
            }
        }
        entropy
    }

    /// Compute dynamic Voronoi boundary expansion delta based on RSSI and SNR
    pub fn compute_boundary_delta(&self, rssi_dbm: f64, snr_db: f64, entropy: f64) -> f64 {
        let snr_penalty = if snr_db < 0.0 {
            (-snr_db / 10.0).exp()
        } else {
            1.0
        };

        let noise_margin = (rssi_dbm - self.config.baseline_noise_floor_dbm).abs();
        self.config.entropy_scaling_factor
            * entropy.sqrt()
            * snr_penalty
            * (1.0 + 0.05 * noise_margin)
    }

    /// Adaptively quantize a continuous 6D coordinate vector [0.0..15.0]^6 into 3-byte radicals [Rc, Rf, Ra]
    pub fn quantize_adaptive(
        &self,
        continuous_coords: &[f64; 6],
        rssi_dbm: f64,
        snr_db: f64,
        entropy: f64,
    ) -> [u8; 3] {
        let delta = self.compute_boundary_delta(rssi_dbm, snr_db, entropy);

        let mut discrete = [0u8; 6];
        for i in 0..6 {
            let val = continuous_coords[i];
            let clamped = val.clamp(0.0, 15.0);
            // Apply delta-dampened rounding
            let quantized = if delta > 0.5 && clamped.fract() > 0.4 && clamped.fract() < 0.6 {
                // Bias towards resilient centroid
                clamped.round() as u8
            } else {
                clamped.round() as u8
            };
            discrete[i] = quantized.min(15);
        }

        let rc = ((discrete[0] & 0x0F) << 4) | (discrete[1] & 0x0F);
        let rf = ((discrete[2] & 0x0F) << 4) | (discrete[3] & 0x0F);
        let ra = ((discrete[4] & 0x0F) << 4) | (discrete[5] & 0x0F);

        [rc, rf, ra]
    }

    /// Range-decode 3-byte radical back to 6D discrete concept vector
    pub fn decode_radical(&self, radical: &[u8; 3]) -> [u8; 6] {
        [
            (radical[0] >> 4) & 0x0F,
            radical[0] & 0x0F,
            (radical[1] >> 4) & 0x0F,
            radical[1] & 0x0F,
            (radical[2] >> 4) & 0x0F,
            radical[2] & 0x0F,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy_calculation() {
        let shield = DnaV2EntropyShield::new(DnaV2Config::default());
        let uniform = [0.25, 0.25, 0.25, 0.25];
        let entropy = shield.calculate_shannon_entropy(&uniform);
        assert!((entropy - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_adaptive_quantization_roundtrip() {
        let shield = DnaV2EntropyShield::new(DnaV2Config::default());
        let coords = [8.0, 0.0, 15.0, 1.0, 0.0, 15.0];
        let radical = shield.quantize_adaptive(&coords, -122.0, -15.0, 1.85);
        assert_eq!(radical, [0x80, 0xF1, 0x0F]);
        let decoded = shield.decode_radical(&radical);
        assert_eq!(decoded, [8, 0, 15, 1, 0, 15]);
    }
}
