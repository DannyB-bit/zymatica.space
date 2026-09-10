// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! # Invention Class 29: Hyper-KV
//!
//! This module contains two representations:
//! - `HyperKvKnotLUT`: legacy compatibility representation retained for existing callers.
//! - `HyperKvLowRankBlock`: deterministic low-rank block compressor intended for real KV-cache
//!   measurement. It stores an orthonormal basis and per-token coefficients in IEEE f16.
//!
//! The low-rank format does not claim a fixed compression ratio or lossless reconstruction.
//! Both compression ratio and reconstruction error are measured from the actual input block.

use half::f16;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HyperKvError {
    EmptyInput,
    DimensionMismatch { expected: usize, actual: usize },
    InvalidRank { requested: usize, max_rank: usize },
    DegenerateInput,
}

impl fmt::Display for HyperKvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "Hyper-KV requires at least one token and one dimension"),
            Self::DimensionMismatch { expected, actual } => write!(
                f,
                "Hyper-KV input length mismatch: expected {expected} floats, got {actual}"
            ),
            Self::InvalidRank {
                requested,
                max_rank,
            } => write!(
                f,
                "Hyper-KV rank {requested} is invalid; valid range is 1..={max_rank}"
            ),
            Self::DegenerateInput => write!(f, "Hyper-KV input has no non-zero residual direction"),
        }
    }
}

impl std::error::Error for HyperKvError {}

/// Real low-rank compressed representation of one KV block.
///
/// The original block has shape `[tokens, head_dim]` in row-major order. A pivoted
/// Gram-Schmidt basis is learned from the actual block, then the basis and token
/// coefficients are stored in IEEE f16. Reconstruction is:
///
/// `X_hat[t, d] = sum_r coeff[t, r] * basis[r, d]`.
#[derive(Debug, Clone, PartialEq)]
pub struct HyperKvLowRankBlock {
    pub tokens: usize,
    pub head_dim: usize,
    pub rank: usize,
    /// Row-major `[rank, head_dim]`, each value encoded with `f16::to_bits()`.
    basis_f16: Vec<u16>,
    /// Row-major `[tokens, rank]`, each value encoded with `f16::to_bits()`.
    coeffs_f16: Vec<u16>,
    reconstruction_mse: f32,
    max_abs_error: f32,
}

impl HyperKvLowRankBlock {
    const METADATA_BYTES: usize = 8 * 3 + 4 * 2;
    const EPS: f32 = 1.0e-12;

    pub fn compress(
        kv_vectors: &[f32],
        tokens: usize,
        head_dim: usize,
        requested_rank: usize,
    ) -> Result<Self, HyperKvError> {
        if tokens == 0 || head_dim == 0 {
            return Err(HyperKvError::EmptyInput);
        }

        let expected = tokens
            .checked_mul(head_dim)
            .ok_or(HyperKvError::DimensionMismatch {
                expected: usize::MAX,
                actual: kv_vectors.len(),
            })?;
        if kv_vectors.len() != expected {
            return Err(HyperKvError::DimensionMismatch {
                expected,
                actual: kv_vectors.len(),
            });
        }

        let max_rank = tokens.min(head_dim);
        if requested_rank == 0 || requested_rank > max_rank {
            return Err(HyperKvError::InvalidRank {
                requested: requested_rank,
                max_rank,
            });
        }

        // Deterministic pivoted Gram-Schmidt over actual token rows. The pivot at each
        // iteration is the token whose residual has the greatest L2 energy.
        let mut basis: Vec<Vec<f32>> = Vec::with_capacity(requested_rank);
        for _ in 0..requested_rank {
            let mut best_residual: Option<Vec<f32>> = None;
            let mut best_energy = 0.0f32;

            for row in kv_vectors.chunks_exact(head_dim) {
                let mut residual = row.to_vec();
                for b in &basis {
                    let projection = dot(row, b);
                    for (value, &basis_value) in residual.iter_mut().zip(b) {
                        *value -= projection * basis_value;
                    }
                }

                let energy = dot(&residual, &residual);
                if energy > best_energy {
                    best_energy = energy;
                    best_residual = Some(residual);
                }
            }

            if best_energy <= Self::EPS {
                break;
            }

            let mut vector = best_residual.ok_or(HyperKvError::DegenerateInput)?;
            let inv_norm = best_energy.sqrt().recip();
            for value in &mut vector {
                *value *= inv_norm;
            }

            // A second orthogonalization pass is cheap at these ranks and materially
            // reduces accumulated numerical drift.
            for b in &basis {
                let projection = dot(&vector, b);
                for (value, &basis_value) in vector.iter_mut().zip(b) {
                    *value -= projection * basis_value;
                }
            }
            let norm = dot(&vector, &vector).sqrt();
            if norm <= Self::EPS {
                break;
            }
            for value in &mut vector {
                *value /= norm;
            }
            basis.push(vector);
        }

        if basis.is_empty() {
            return Err(HyperKvError::DegenerateInput);
        }

        let rank = basis.len();
        let mut coeffs = Vec::with_capacity(tokens * rank);
        for row in kv_vectors.chunks_exact(head_dim) {
            coeffs.extend(basis.iter().map(|b| dot(row, b)));
        }

        let basis_f16 = basis
            .iter()
            .flat_map(|row| row.iter().copied())
            .map(|value| f16::from_f32(value).to_bits())
            .collect::<Vec<_>>();
        let coeffs_f16 = coeffs
            .iter()
            .copied()
            .map(|value| f16::from_f32(value).to_bits())
            .collect::<Vec<_>>();

        let mut block = Self {
            tokens,
            head_dim,
            rank,
            basis_f16,
            coeffs_f16,
            reconstruction_mse: 0.0,
            max_abs_error: 0.0,
        };

        let reconstructed = block.reconstruct();
        let (mse, max_abs) = error_metrics(kv_vectors, &reconstructed);
        block.reconstruction_mse = mse;
        block.max_abs_error = max_abs;
        Ok(block)
    }

    pub fn reconstruct(&self) -> Vec<f32> {
        let basis = self
            .basis_f16
            .iter()
            .map(|&bits| f16::from_bits(bits).to_f32())
            .collect::<Vec<_>>();
        let coeffs = self
            .coeffs_f16
            .iter()
            .map(|&bits| f16::from_bits(bits).to_f32())
            .collect::<Vec<_>>();

        let mut out = vec![0.0f32; self.tokens * self.head_dim];
        for (token_index, row_out) in out.chunks_exact_mut(self.head_dim).enumerate() {
            let coeff_row = &coeffs[token_index * self.rank..(token_index + 1) * self.rank];
            for (rank_index, &coefficient) in coeff_row.iter().enumerate() {
                let basis_row =
                    &basis[rank_index * self.head_dim..(rank_index + 1) * self.head_dim];
                for (dst, &basis_value) in row_out.iter_mut().zip(basis_row) {
                    *dst += coefficient * basis_value;
                }
            }
        }
        out
    }

    pub fn reconstruction_mse(&self) -> f32 {
        self.reconstruction_mse
    }

    pub fn max_abs_error(&self) -> f32 {
        self.max_abs_error
    }

    pub fn original_bytes(&self) -> usize {
        self.tokens * self.head_dim * std::mem::size_of::<f32>()
    }

    pub fn compressed_payload_bytes(&self) -> usize {
        Self::METADATA_BYTES
            + self.basis_f16.len() * std::mem::size_of::<u16>()
            + self.coeffs_f16.len() * std::mem::size_of::<u16>()
    }

    pub fn compression_ratio(&self) -> f32 {
        self.original_bytes() as f32 / self.compressed_payload_bytes() as f32
    }

    pub fn basis_bits(&self) -> &[u16] {
        &self.basis_f16
    }

    pub fn coefficient_bits(&self) -> &[u16] {
        &self.coeffs_f16
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(&x, &y)| x * y).sum()
}

fn error_metrics(reference: &[f32], reconstructed: &[f32]) -> (f32, f32) {
    debug_assert_eq!(reference.len(), reconstructed.len());
    if reference.is_empty() {
        return (0.0, 0.0);
    }

    let mut squared_error = 0.0f64;
    let mut max_abs = 0.0f32;
    for (&a, &b) in reference.iter().zip(reconstructed) {
        let diff = a - b;
        squared_error += f64::from(diff) * f64::from(diff);
        max_abs = max_abs.max(diff.abs());
    }
    ((squared_error / reference.len() as f64) as f32, max_abs)
}

/// Legacy compatibility representation retained so existing code does not break.
///
/// `PHASE_LUT` is intentionally quantized to four decimal places, so the approximate
/// constants are deliberate table values rather than accidental mathematical constants.
#[derive(Debug, Clone, PartialEq)]
pub struct HyperKvKnotLUT {
    pub base_coord: [f32; 6],
    pub delta_tangent: [f32; 6],
    pub lut_index: u8,
    pub span_tokens: u8,
}

impl HyperKvKnotLUT {
    #[allow(clippy::approx_constant)]
    pub const PHASE_LUT: [f32; 16] = [
        0.0000, 0.3826, 0.7071, 0.9238, 1.0000, 0.9238, 0.7071, 0.3826, 0.0000, -0.3826, -0.7071,
        -0.9238, -1.0000, -0.9238, -0.7071, -0.3826,
    ];

    #[inline(always)]
    pub fn evaluate_lut(&self, t: usize, head_dim: usize) -> Vec<f32> {
        let t_norm = if self.span_tokens > 1 {
            t as f32 / f32::from(self.span_tokens - 1)
        } else {
            0.0
        };
        let mut out = vec![0.0f32; head_dim];
        let lut_val = Self::PHASE_LUT[(usize::from(self.lut_index) + t) % Self::PHASE_LUT.len()];

        for (index, dst) in out.iter_mut().enumerate() {
            let axis = index % 6;
            let base_val = self.base_coord[axis] + self.delta_tangent[axis] * t_norm;
            *dst = base_val + lut_val * 0.05;
        }
        out
    }

    pub fn compress_block(kv_vectors: &[f32], tokens: usize, head_dim: usize) -> Self {
        assert!(tokens > 0, "tokens must be non-zero");
        assert!(head_dim > 0, "head_dim must be non-zero");
        assert_eq!(kv_vectors.len(), tokens * head_dim);

        let mut base_coord = [0.0f32; 6];
        let mut end_coord = [0.0f32; 6];

        for (index, &value) in kv_vectors[..head_dim].iter().enumerate() {
            base_coord[index % 6] += value;
        }
        for (axis, value) in base_coord.iter_mut().enumerate() {
            let count = (head_dim + 5 - axis) / 6;
            if count > 0 {
                *value /= count as f32;
            }
        }

        let last_offset = (tokens - 1) * head_dim;
        for (index, &value) in kv_vectors[last_offset..last_offset + head_dim]
            .iter()
            .enumerate()
        {
            end_coord[index % 6] += value;
        }
        for (axis, value) in end_coord.iter_mut().enumerate() {
            let count = (head_dim + 5 - axis) / 6;
            if count > 0 {
                *value /= count as f32;
            }
        }

        let mut delta_tangent = [0.0f32; 6];
        for ((dst, &end), &base) in delta_tangent.iter_mut().zip(&end_coord).zip(&base_coord) {
            *dst = end - base;
        }

        Self {
            base_coord,
            delta_tangent,
            lut_index: (tokens % 16) as u8,
            span_tokens: tokens.min(usize::from(u8::MAX)) as u8,
        }
    }

    pub fn reconstruct_block(&self, tokens: usize, head_dim: usize) -> Vec<f32> {
        let mut block = Vec::with_capacity(tokens * head_dim);
        for token in 0..tokens {
            block.extend(self.evaluate_lut(token, head_dim));
        }
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_rank_block_compresses_real_matrix_structure() {
        let tokens = 32;
        let head_dim = 64;
        let rank = 4;

        // Construct an actual rank-4 matrix with smooth, non-trivial coefficients.
        let mut basis = vec![vec![0.0f32; head_dim]; rank];
        for r in 0..rank {
            let (prev_slice, curr_slice) = basis.split_at_mut(r);
            let row = &mut curr_slice[0];
            for (d, value) in row.iter_mut().enumerate() {
                *value = ((d as f32 + 1.0) * (r as f32 + 0.75) * 0.031).sin();
            }
            // Orthogonalize against preceding basis vectors to guarantee rank-4 independence
            for prev_row in prev_slice {
                let projection = dot(row, prev_row);
                for (dst, &src) in row.iter_mut().zip(prev_row.iter()) {
                    *dst -= projection * src;
                }
            }
            let norm = dot(row, row).sqrt();
            for value in row.iter_mut() {
                *value /= norm;
            }
        }

        let mut original = vec![0.0f32; tokens * head_dim];
        for (token, out_row) in original.chunks_exact_mut(head_dim).enumerate() {
            for (r, basis_row) in basis.iter().enumerate() {
                let coeff =
                    ((token as f32 * 0.13 + 0.5) * (r as f32 + 1.0)).sin() * (r as f32 + 1.0);
                for (dst, &basis_value) in out_row.iter_mut().zip(basis_row) {
                    *dst += coeff * basis_value;
                }
            }
        }

        let compressed = HyperKvLowRankBlock::compress(&original, tokens, head_dim, rank)
            .expect("low-rank compression should succeed");
        let reconstructed = compressed.reconstruct();

        assert_eq!(reconstructed.len(), original.len());
        assert_eq!(compressed.rank, rank);
        assert!(compressed.reconstruction_mse() < 1.0e-4);
        assert!(compressed.compression_ratio() > 2.0);
    }

    #[test]
    fn legacy_knot_remains_compatible() {
        let tokens = 16;
        let head_dim = 64;
        let mut original_kv = vec![0.0f32; tokens * head_dim];
        for (index, value) in original_kv.iter_mut().enumerate() {
            let token = index / head_dim;
            let dim = index % head_dim;
            *value = token as f32 * 0.1 + (dim % 6) as f32 * 0.5;
        }

        let knot = HyperKvKnotLUT::compress_block(&original_kv, tokens, head_dim);
        let reconstructed = knot.reconstruct_block(tokens, head_dim);
        assert_eq!(reconstructed.len(), original_kv.len());
    }
}
