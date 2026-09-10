// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! # Invention Class 31: Epigenetic Weight Crystallizer / Nullspace Update Projector
//!
//! The mathematical guarantee implemented here is deliberately narrow and testable:
//! a projected update is numerically orthogonal to the supplied activation basis, up to
//! floating-point tolerance. That property is useful for constrained continual-learning
//! experiments, but it is not by itself a proof of zero catastrophic forgetting.

#[derive(Debug, Clone, PartialEq)]
pub struct EpigeneticCrystal {
    pub domain_id: u8,
    pub nullspace_basis_rank: u8,
    pub crystal_weights: [f32; 16],
    pub activation_hash: u32,
}

impl EpigeneticCrystal {
    pub const WIRE_BYTE_LEN: usize = 70;

    pub const fn new(domain: u8, rank: u8, weights: [f32; 16], hash: u32) -> Self {
        Self {
            domain_id: domain,
            nullspace_basis_rank: rank,
            crystal_weights: weights,
            activation_hash: hash,
        }
    }

    pub fn to_bytes(&self) -> [u8; Self::WIRE_BYTE_LEN] {
        let mut out = [0u8; Self::WIRE_BYTE_LEN];
        out[0] = self.domain_id;
        out[1] = self.nullspace_basis_rank;
        for (index, &weight) in self.crystal_weights.iter().enumerate() {
            let offset = 2 + index * 4;
            out[offset..offset + 4].copy_from_slice(&weight.to_be_bytes());
        }
        out[66..70].copy_from_slice(&self.activation_hash.to_be_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8; Self::WIRE_BYTE_LEN]) -> Self {
        let mut weights = [0.0f32; 16];
        for (index, slot) in weights.iter_mut().enumerate() {
            let offset = 2 + index * 4;
            *slot = f32::from_be_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
        }
        Self {
            domain_id: bytes[0],
            nullspace_basis_rank: bytes[1],
            crystal_weights: weights,
            activation_hash: u32::from_be_bytes([bytes[66], bytes[67], bytes[68], bytes[69]]),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectionMetrics {
    pub projected_l2_norm: f32,
    pub max_abs_basis_dot: f32,
    pub basis_vectors_checked: usize,
}

pub struct EpigeneticManifoldEngine {
    pub hidden_dim: usize,
    pub update_counter: usize,
}

impl EpigeneticManifoldEngine {
    pub const fn new(hidden_dim: usize) -> Self {
        Self {
            hidden_dim,
            update_counter: 0,
        }
    }

    /// Two-pass Modified Gram-Schmidt projection into the numerical nullspace of `base_basis`.
    ///
    /// Panics with a precise message if dimensions are inconsistent; use
    /// `compute_nullspace_projection_checked` when dimensions come from untrusted input.
    pub fn compute_nullspace_projection_mgs(
        &mut self,
        base_basis: &[Vec<f32>],
        new_concept: &[f32],
    ) -> Vec<f32> {
        self.compute_nullspace_projection_checked(base_basis, new_concept)
            .expect("invalid dimensions supplied to nullspace projection")
    }

    pub fn compute_nullspace_projection_checked(
        &mut self,
        base_basis: &[Vec<f32>],
        new_concept: &[f32],
    ) -> Result<Vec<f32>, &'static str> {
        if self.hidden_dim == 0 {
            return Err("hidden_dim must be non-zero");
        }
        if new_concept.len() != self.hidden_dim {
            return Err("new concept dimension does not match hidden_dim");
        }
        if base_basis
            .iter()
            .any(|basis| basis.len() != self.hidden_dim)
        {
            return Err("basis vector dimension does not match hidden_dim");
        }
        if !new_concept.iter().all(|value| value.is_finite())
            || !base_basis.iter().flatten().all(|value| value.is_finite())
        {
            return Err("projection input contains NaN or infinity");
        }

        self.update_counter = self.update_counter.saturating_add(1);
        let mut projected = new_concept.to_vec();
        self.reorthogonalize(base_basis, &mut projected);
        self.reorthogonalize(base_basis, &mut projected);
        Ok(projected)
    }

    /// Project every update row independently against the same activation basis. For a linear
    /// layer weight update `ΔW[out, in]`, each row is an input-space direction and therefore
    /// can be projected against a basis spanning old input activations.
    pub fn project_update_rows(
        &mut self,
        base_basis: &[Vec<f32>],
        update_rows: &mut [Vec<f32>],
    ) -> Result<ProjectionMetrics, &'static str> {
        if update_rows.iter().any(|row| row.len() != self.hidden_dim) {
            return Err("update row dimension does not match hidden_dim");
        }

        for row in update_rows.iter_mut() {
            let projected = self.compute_nullspace_projection_checked(base_basis, row)?;
            row.copy_from_slice(&projected);
        }

        let mut max_abs_basis_dot = 0.0f32;
        let mut norm_sq = 0.0f32;
        for row in update_rows {
            norm_sq += dot(row, row);
            for basis in base_basis {
                max_abs_basis_dot = max_abs_basis_dot.max(dot(row, basis).abs());
            }
        }

        Ok(ProjectionMetrics {
            projected_l2_norm: norm_sq.sqrt(),
            max_abs_basis_dot,
            basis_vectors_checked: base_basis.len(),
        })
    }

    pub fn projection_metrics(
        &self,
        base_basis: &[Vec<f32>],
        projected: &[f32],
    ) -> Result<ProjectionMetrics, &'static str> {
        if projected.len() != self.hidden_dim
            || base_basis
                .iter()
                .any(|basis| basis.len() != self.hidden_dim)
        {
            return Err("projection metric dimension mismatch");
        }

        let max_abs_basis_dot = base_basis
            .iter()
            .map(|basis| dot(projected, basis).abs())
            .fold(0.0f32, f32::max);
        Ok(ProjectionMetrics {
            projected_l2_norm: dot(projected, projected).sqrt(),
            max_abs_basis_dot,
            basis_vectors_checked: base_basis.len(),
        })
    }

    fn reorthogonalize(&self, base_basis: &[Vec<f32>], vector: &mut [f32]) {
        for basis in base_basis {
            let norm_sq = dot(basis, basis);
            if norm_sq <= 1.0e-12 {
                continue;
            }
            let scale = dot(basis, vector) / norm_sq;
            for (value, &basis_value) in vector.iter_mut().zip(basis) {
                *value -= scale * basis_value;
            }
        }
    }
}

fn dot(lhs: &[f32], rhs: &[f32]) -> f32 {
    lhs.iter().zip(rhs).map(|(&a, &b)| a * b).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystal_70_byte_wire_roundtrip() {
        let mut weights = [0.0f32; 16];
        for (index, value) in weights.iter_mut().enumerate() {
            *value = index as f32 * 1.5 - 4.0;
        }
        let crystal = EpigeneticCrystal::new(7, 3, weights, 0xCAFE_BABE);
        let bytes = crystal.to_bytes();
        assert_eq!(bytes.len(), EpigeneticCrystal::WIRE_BYTE_LEN);

        let decoded = EpigeneticCrystal::from_bytes(&bytes);
        assert_eq!(decoded.domain_id, 7);
        assert_eq!(decoded.nullspace_basis_rank, 3);
        assert_eq!(decoded.activation_hash, 0xCAFE_BABE);
        for (&actual, &expected) in decoded.crystal_weights.iter().zip(&weights) {
            assert!((actual - expected).abs() < 1.0e-7);
        }
    }

    #[test]
    fn two_pass_projection_is_orthogonal_to_supplied_basis() {
        let mut engine = EpigeneticManifoldEngine::new(4);
        let base_basis = vec![vec![1.0, 0.0, 0.0, 0.0], vec![0.0, 1.0, 0.0, 0.0]];
        let new_concept = vec![3.0, 4.0, 5.0, 6.0];

        let projected = engine.compute_nullspace_projection_mgs(&base_basis, &new_concept);
        let metrics = engine
            .projection_metrics(&base_basis, &projected)
            .expect("metric dimensions are valid");
        assert!(metrics.max_abs_basis_dot < 1.0e-6);
        assert!((projected[2] - 5.0).abs() < 1.0e-6);
        assert!((projected[3] - 6.0).abs() < 1.0e-6);
    }

    #[test]
    fn matrix_update_rows_are_projected_independently() {
        let mut engine = EpigeneticManifoldEngine::new(3);
        let basis = vec![vec![1.0, 0.0, 0.0]];
        let mut update = vec![vec![2.0, 3.0, 4.0], vec![-7.0, 1.0, 2.0]];
        let metrics = engine
            .project_update_rows(&basis, &mut update)
            .expect("matrix projection should succeed");

        assert!(metrics.max_abs_basis_dot < 1.0e-6);
        assert!(update.iter().all(|row| row[0].abs() < 1.0e-6));
    }
}
