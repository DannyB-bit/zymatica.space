//! 381-Byte Genesis Seed Cold-Start Morphogenetic Tensor Instantiator.
//! Instantiates dense latent neural projection matrices from a deterministic 381-byte radio capsule.

pub const GENESIS_SEED_CAPSULE_SIZE: usize = 381;

#[derive(Clone, Debug)]
pub struct GenesisCapsule {
    pub raw_bytes: [u8; GENESIS_SEED_CAPSULE_SIZE],
}

impl GenesisCapsule {
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() != GENESIS_SEED_CAPSULE_SIZE {
            return None;
        }
        let mut raw = [0u8; GENESIS_SEED_CAPSULE_SIZE];
        raw.copy_from_slice(slice);
        Some(Self { raw_bytes: raw })
    }

    /// Procedurally inflate seed into a dense [dim x dim] latent weight projection matrix.
    pub fn inflate_latent_matrix(&self, dim: usize) -> Vec<f32> {
        let total_elements = dim * dim;
        let mut matrix = Vec::with_capacity(total_elements);

        // Linear Congruential PRNG keyed by genesis anchor
        let mut state = u64::from_le_bytes([
            self.raw_bytes[0],
            self.raw_bytes[1],
            self.raw_bytes[2],
            self.raw_bytes[3],
            self.raw_bytes[4],
            self.raw_bytes[5],
            self.raw_bytes[6],
            self.raw_bytes[7],
        ]);

        for i in 0..total_elements {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let byte_mod = self.raw_bytes[i % GENESIS_SEED_CAPSULE_SIZE] as f32;
            let norm_val = ((state >> 32) as f32) / (u32::MAX as f32) - 0.5;
            let val = (norm_val * 0.02) + (byte_mod * 0.0001);
            matrix.push(val);
        }

        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_morphogenesis_instantiates_stable_projection_matrix() {
        let sample_bytes = [0x42u8; GENESIS_SEED_CAPSULE_SIZE];
        let capsule = GenesisCapsule::from_slice(&sample_bytes).expect("Valid capsule");
        let matrix = capsule.inflate_latent_matrix(64);
        assert_eq!(matrix.len(), 4096);
        assert!(matrix.iter().all(|&v| v.is_finite()));
    }
}
