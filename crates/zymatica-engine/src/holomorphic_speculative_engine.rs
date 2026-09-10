// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! # Invention Class 30: Zymatica Holomorphic Speculative Engine (Z-HQSpec) - Production Hardened

pub struct HolomorphicSpeculatorFused {
    pub hidden_dim: usize,
    pub speculative_depth: usize,
    pub manifold_velocity_gain: f32,
}

impl HolomorphicSpeculatorFused {
    pub fn new(hidden_dim: usize, depth: usize, gain: f32) -> Self {
        Self {
            hidden_dim,
            speculative_depth: depth.max(1),
            manifold_velocity_gain: gain,
        }
    }

    /// Predict draft speculative token candidates by projecting extrapolated hidden states via unembedding matrix
    pub fn generate_speculative_candidates(
        &self,
        current_hidden: &[f32],
        velocity_vector: &[f32],
        unembed_weights: &[f32], // [vocab_size x hidden_dim]
        vocab_size: usize,
    ) -> Vec<usize> {
        assert_eq!(current_hidden.len(), self.hidden_dim);
        assert_eq!(velocity_vector.len(), self.hidden_dim);
        assert_eq!(unembed_weights.len(), vocab_size * self.hidden_dim);

        let mut draft_tokens = Vec::with_capacity(self.speculative_depth);
        let mut extrapolated = current_hidden.to_vec();

        for step in 1..=self.speculative_depth {
            // Apply holomorphic momentum step: h_{t+k} = h_t + k * gain * v_t
            let step_scale = step as f32 * self.manifold_velocity_gain;
            for d in 0..self.hidden_dim {
                extrapolated[d] = current_hidden[d] + step_scale * velocity_vector[d];
            }

            // Project onto vocabulary logits: logits = W_unembed * extrapolated
            let mut best_token = 0;
            let mut max_logit = f32::NEG_INFINITY;
            for v in 0..vocab_size {
                let row_offset = v * self.hidden_dim;
                let mut logit = 0.0f32;
                for d in 0..self.hidden_dim {
                    logit += unembed_weights[row_offset + d] * extrapolated[d];
                }
                if logit > max_logit {
                    max_logit = logit;
                    best_token = v;
                }
            }
            draft_tokens.push(best_token);
        }

        draft_tokens
    }

    #[inline(always)]
    pub fn verify_fused_tree_attention(
        &self,
        verification_matrix: &[f32],
        draft_tokens: &[usize],
        vocab_size: usize,
    ) -> usize {
        let mut accepted = 0;
        for (step, &candidate) in draft_tokens.iter().enumerate() {
            let offset = step * vocab_size;
            if offset + vocab_size > verification_matrix.len() {
                break;
            }

            let mut max_idx = 0;
            let mut max_val = f32::NEG_INFINITY;
            for v in 0..vocab_size {
                let val = verification_matrix[offset + v];
                if val > max_val {
                    max_val = val;
                    max_idx = v;
                }
            }

            if max_idx == candidate {
                accepted += 1;
            } else {
                break;
            }
        }
        accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speculative_candidate_generation_and_verification() {
        let hidden_dim = 4;
        let depth = 3;
        let gain = 1.0;
        let speculator = HolomorphicSpeculatorFused::new(hidden_dim, depth, gain);

        let current_hidden = vec![1.0, 0.0, 0.0, 0.0];
        let velocity = vec![0.0, 1.0, 0.0, 0.0];
        let vocab_size = 3;
        // Orthogonal projection matrix for 3 tokens
        let mut unembed = vec![0.0f32; vocab_size * hidden_dim];
        unembed[0] = 1.0; // Token 0 aligns with axis 0
        unembed[hidden_dim + 1] = 2.0; // Token 1 aligns with axis 1
        unembed[2 * hidden_dim + 2] = 1.0; // Token 2 aligns with axis 2

        let draft = speculator.generate_speculative_candidates(
            &current_hidden,
            &velocity,
            &unembed,
            vocab_size,
        );
        assert_eq!(draft.len(), depth);

        // Verification matrix matching draft
        let mut verif = vec![0.0f32; depth * vocab_size];
        for (step, &token) in draft.iter().enumerate() {
            verif[step * vocab_size + token] = 10.0;
        }

        let accepted = speculator.verify_fused_tree_attention(&verif, &draft, vocab_size);
        assert_eq!(accepted, depth);
    }
}
