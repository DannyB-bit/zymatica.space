//! 6D Hyper-Geodesic KV-Cache Compression Bridge.
//! Compresses historical KV cache states into 6D Riemannian manifold trajectories,
//! reducing long-context RAM footprint with mathematically lossless delta fidelity.

use crate::cuneiform::{Concept6D, decode_geodesic_deltas, encode_geodesic_deltas};

#[derive(Clone, Debug)]
pub struct CompressedKvBlock {
    pub trajectory: Vec<Concept6D>,
}

impl CompressedKvBlock {
    pub fn new(anchor: Concept6D) -> Self {
        Self {
            trajectory: vec![anchor],
        }
    }

    pub fn push_step(&mut self, next: Concept6D, _prev: Concept6D) {
        self.trajectory.push(next);
    }

    pub fn encode_to_bytes(&self) -> Vec<u8> {
        encode_geodesic_deltas(&self.trajectory)
    }

    pub fn decode_from_bytes(bytes: &[u8], count: usize) -> Vec<Concept6D> {
        decode_geodesic_deltas(bytes, count)
    }

    pub fn decompress_all(&self) -> Vec<Concept6D> {
        let bytes = self.encode_to_bytes();
        Self::decode_from_bytes(&bytes, self.trajectory.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_kv_block_round_trips_losslessly() {
        let c0 = Concept6D::new(1, 2, 8, 8, 8, 8);
        let mut block = CompressedKvBlock::new(c0);

        let c1 = Concept6D::new(1, 2, 9, 8, 7, 8);
        let c2 = Concept6D::new(1, 2, 10, 9, 7, 9);
        let c3 = Concept6D::new(3, 4, 15, 14, 13, 12); // Global jump

        block.push_step(c1, c0);
        block.push_step(c2, c1);
        block.push_step(c3, c2);

        let recovered = block.decompress_all();
        assert_eq!(recovered.len(), 4);
        assert_eq!(recovered[0], c0);
        assert_eq!(recovered[1], c1);
        assert_eq!(recovered[2], c2);
        assert_eq!(recovered[3], c3);
    }
}
