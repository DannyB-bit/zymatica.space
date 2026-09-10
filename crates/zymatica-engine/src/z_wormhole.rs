// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! ==============================================================================
//! ZYMATICA CLASS 34: Z-WORMHOLE — Learned Cross-Model Latent Projection Transport
//! ==============================================================================
//!
//! The production path in this module is a learned low-rank bridge exported from real
//! source/target model activations. `ZWormholeBridge::new_untrained_deterministic` is retained
//! only as a deterministic initialization/demo path and is explicitly marked untrained.
//!
//! A learned bridge factorizes a source→target linear map as:
//!
//! `source_hidden [Dsrc] × W_down [Dsrc,K] × W_up [K,Dtgt]`.
//!
//! The first eight intermediate coordinates are carried through the bounded 8-axis capsule
//! representation using an invertible sigmoid/logit transform; the remaining K-8 values are
//! stored as latent harmonics. No claim of universal semantic equivalence is made unless an
//! external real-model evaluation artifact demonstrates it.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelArch {
    Qwen35_0_8B,
    Qwen35_4B,
    Gemma2_2B,
    Gemma2_9B,
    SmolLM2_135M,
    Custom(usize),
}

impl ModelArch {
    pub fn hidden_dim(&self) -> usize {
        match self {
            Self::Qwen35_0_8B => 896,
            Self::Qwen35_4B => 2048,
            Self::Gemma2_2B => 2304,
            Self::Gemma2_9B => 3584,
            Self::SmolLM2_135M => 576,
            Self::Custom(dim) => *dim,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeKind {
    UntrainedDeterministic,
    Learned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedBridgeWeights {
    pub format_version: u32,
    pub source_dim: usize,
    pub target_dim: usize,
    /// Number of harmonic values. Total intermediate width is `8 + intermediate_dim`.
    pub intermediate_dim: usize,
    pub proj_down: Vec<f32>,
    pub proj_up: Vec<f32>,
    #[serde(default)]
    pub training_fingerprint_sha256: String,
    #[serde(default)]
    pub source_model: String,
    #[serde(default)]
    pub target_model: String,
    #[serde(default)]
    pub heldout_cosine_mean: Option<f32>,
    #[serde(default)]
    pub heldout_top1_agreement: Option<f32>,
}

impl LearnedBridgeWeights {
    pub const FORMAT_VERSION: u32 = 1;

    pub fn validate(&self) -> Result<()> {
        if self.format_version != Self::FORMAT_VERSION {
            bail!(
                "unsupported Z-WORMHOLE bridge format version {}; expected {}",
                self.format_version,
                Self::FORMAT_VERSION
            );
        }
        if self.source_dim == 0 || self.target_dim == 0 {
            bail!("source_dim and target_dim must be non-zero");
        }
        let total_inter = 8usize
            .checked_add(self.intermediate_dim)
            .context("intermediate dimension overflow")?;
        let expected_down = self
            .source_dim
            .checked_mul(total_inter)
            .context("projection-down dimension overflow")?;
        let expected_up = total_inter
            .checked_mul(self.target_dim)
            .context("projection-up dimension overflow")?;
        if self.proj_down.len() != expected_down {
            bail!(
                "proj_down length mismatch: expected {}, got {}",
                expected_down,
                self.proj_down.len()
            );
        }
        if self.proj_up.len() != expected_up {
            bail!(
                "proj_up length mismatch: expected {}, got {}",
                expected_up,
                self.proj_up.len()
            );
        }
        if !self
            .proj_down
            .iter()
            .chain(&self.proj_up)
            .all(|v| v.is_finite())
        {
            bail!("learned projection contains NaN or infinity");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LatentThoughtCapsule {
    pub sequence_id: u64,
    pub axes: [f32; 8],
    pub latent_harmonics: Vec<f32>,
    pub epistemic_confidence: f32,
}

impl LatentThoughtCapsule {
    pub fn new(sequence_id: u64, axes: [f32; 8], harmonics: Vec<f32>, conf: f32) -> Self {
        Self {
            sequence_id,
            axes,
            latent_harmonics: harmonics,
            epistemic_confidence: conf.clamp(0.0, 1.0),
        }
    }

    pub fn geodesic_distance(&self, other: &Self) -> f32 {
        const WEIGHTS: [f32; 8] = [1.0, 1.0, 0.75, 0.75, 0.5, 0.5, 0.25, 0.25];
        self.axes
            .iter()
            .zip(&other.axes)
            .zip(WEIGHTS)
            .map(|((&lhs, &rhs), weight)| {
                let diff = lhs - rhs;
                weight * diff * diff
            })
            .sum::<f32>()
            .sqrt()
    }
}

pub struct ZWormholeBridge {
    pub source_arch: ModelArch,
    pub target_arch: ModelArch,
    pub intermediate_dim: usize,
    bridge_kind: BridgeKind,
    proj_down_weights: Vec<f32>,
    proj_up_weights: Vec<f32>,
    training_fingerprint_sha256: Option<String>,
}

impl ZWormholeBridge {
    /// Compatibility constructor. It creates an untrained deterministic projection and therefore
    /// must not be reported as a learned or validated cross-model semantic bridge.
    pub fn new(source: ModelArch, target: ModelArch, intermediate_dim: usize) -> Self {
        Self::new_untrained_deterministic(source, target, intermediate_dim)
    }

    pub fn new_untrained_deterministic(
        source: ModelArch,
        target: ModelArch,
        intermediate_dim: usize,
    ) -> Self {
        let src_dim = source.hidden_dim();
        let tgt_dim = target.hidden_dim();
        let total_inter = 8 + intermediate_dim;
        let mut proj_down = vec![0.0f32; src_dim * total_inter];
        let mut proj_up = vec![0.0f32; total_inter * tgt_dim];

        let mut state = 0x811c9dc5u64;
        let scale_down = (2.0 / (src_dim + total_inter) as f32).sqrt();
        let scale_up = (2.0 / (total_inter + tgt_dim) as f32).sqrt();

        for weight in &mut proj_down {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let sample = (state >> 32) as f32 / u32::MAX as f32 - 0.5;
            *weight = sample * scale_down;
        }
        for weight in &mut proj_up {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let sample = (state >> 32) as f32 / u32::MAX as f32 - 0.5;
            *weight = sample * scale_up;
        }

        Self {
            source_arch: source,
            target_arch: target,
            intermediate_dim,
            bridge_kind: BridgeKind::UntrainedDeterministic,
            proj_down_weights: proj_down,
            proj_up_weights: proj_up,
            training_fingerprint_sha256: None,
        }
    }

    pub fn from_learned_weights(
        source: ModelArch,
        target: ModelArch,
        weights: LearnedBridgeWeights,
    ) -> Result<Self> {
        weights.validate()?;
        if source.hidden_dim() != weights.source_dim {
            bail!(
                "source architecture dimension {} does not match learned bridge dimension {}",
                source.hidden_dim(),
                weights.source_dim
            );
        }
        if target.hidden_dim() != weights.target_dim {
            bail!(
                "target architecture dimension {} does not match learned bridge dimension {}",
                target.hidden_dim(),
                weights.target_dim
            );
        }

        Ok(Self {
            source_arch: source,
            target_arch: target,
            intermediate_dim: weights.intermediate_dim,
            bridge_kind: BridgeKind::Learned,
            proj_down_weights: weights.proj_down,
            proj_up_weights: weights.proj_up,
            training_fingerprint_sha256: (!weights.training_fingerprint_sha256.is_empty())
                .then_some(weights.training_fingerprint_sha256),
        })
    }

    pub fn load_learned_json(
        source: ModelArch,
        target: ModelArch,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read learned bridge {}", path.display()))?;
        let weights: LearnedBridgeWeights = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse learned bridge {}", path.display()))?;
        Self::from_learned_weights(source, target, weights)
    }

    pub fn bridge_kind(&self) -> BridgeKind {
        self.bridge_kind
    }

    pub fn is_learned(&self) -> bool {
        self.bridge_kind == BridgeKind::Learned
    }

    pub fn training_fingerprint_sha256(&self) -> Option<&str> {
        self.training_fingerprint_sha256.as_deref()
    }

    pub fn compress_thought(
        &self,
        source_hidden: &[f32],
        sequence_id: u64,
    ) -> Result<LatentThoughtCapsule> {
        let src_dim = self.source_arch.hidden_dim();
        if source_hidden.len() != src_dim {
            bail!(
                "source activation dimension mismatch: expected {}, got {}",
                src_dim,
                source_hidden.len()
            );
        }
        if !source_hidden.iter().all(|value| value.is_finite()) {
            bail!("source activation contains NaN or infinity");
        }

        let total_inter = 8 + self.intermediate_dim;
        let mut intermediate = vec![0.0f32; total_inter];
        for (column, dst) in intermediate.iter_mut().enumerate() {
            *dst = source_hidden
                .iter()
                .enumerate()
                .map(|(row, &value)| value * self.proj_down_weights[row * total_inter + column])
                .sum();
        }

        let mut axes = [0.0f32; 8];
        for (dst, &value) in axes.iter_mut().zip(&intermediate[..8]) {
            *dst = sigmoid(value) * 15.0;
        }

        let norm = source_hidden
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        // Confidence here is only a bounded numerical signal. It is not a calibrated probability.
        let confidence = sigmoid(norm * 0.1).clamp(0.1, 1.0);

        Ok(LatentThoughtCapsule::new(
            sequence_id,
            axes,
            intermediate[8..].to_vec(),
            confidence,
        ))
    }

    pub fn expand_thought(&self, capsule: &LatentThoughtCapsule) -> Result<Vec<f32>> {
        if capsule.latent_harmonics.len() != self.intermediate_dim {
            bail!(
                "capsule harmonic dimension mismatch: expected {}, got {}",
                self.intermediate_dim,
                capsule.latent_harmonics.len()
            );
        }
        if !capsule
            .axes
            .iter()
            .chain(&capsule.latent_harmonics)
            .all(|value| value.is_finite())
        {
            bail!("capsule contains NaN or infinity");
        }

        let total_inter = 8 + self.intermediate_dim;
        let mut intermediate = Vec::with_capacity(total_inter);
        for &axis in &capsule.axes {
            let normalized = (axis / 15.0).clamp(1.0e-5, 1.0 - 1.0e-5);
            intermediate.push(logit(normalized));
        }
        intermediate.extend_from_slice(&capsule.latent_harmonics);

        let tgt_dim = self.target_arch.hidden_dim();
        let mut target_hidden = vec![0.0f32; tgt_dim];
        for (column, dst) in target_hidden.iter_mut().enumerate() {
            *dst = intermediate
                .iter()
                .enumerate()
                .map(|(row, &value)| value * self.proj_up_weights[row * tgt_dim + column])
                .sum();
        }
        Ok(target_hidden)
    }

    /// Cosine similarity for equal-dimensional representations only.
    pub fn alignment_similarity(&self, lhs: &[f32], rhs: &[f32]) -> f32 {
        if lhs.len() != rhs.len() || lhs.is_empty() {
            return 0.0;
        }
        cosine_similarity(lhs, rhs)
    }

    /// Evaluate a predicted target hidden state against a real target-model reference state.
    pub fn target_reference_similarity(&self, predicted: &[f32], reference: &[f32]) -> Result<f32> {
        let expected = self.target_arch.hidden_dim();
        if predicted.len() != expected || reference.len() != expected {
            bail!(
                "target comparison requires two vectors of dimension {}; got {} and {}",
                expected,
                predicted.len(),
                reference.len()
            );
        }
        Ok(cosine_similarity(predicted, reference))
    }
}

fn sigmoid(value: f32) -> f32 {
    if value >= 0.0 {
        1.0 / (1.0 + (-value).exp())
    } else {
        let exp = value.exp();
        exp / (1.0 + exp)
    }
}

fn logit(probability: f32) -> f32 {
    (probability / (1.0 - probability)).ln()
}

fn cosine_similarity(lhs: &[f32], rhs: &[f32]) -> f32 {
    let dot = lhs.iter().zip(rhs).map(|(&a, &b)| a * b).sum::<f32>();
    let lhs_norm = lhs.iter().map(|value| value * value).sum::<f32>().sqrt();
    let rhs_norm = rhs.iter().map(|value| value * value).sum::<f32>().sqrt();
    if lhs_norm <= f32::EPSILON || rhs_norm <= f32::EPSILON {
        0.0
    } else {
        dot / (lhs_norm * rhs_norm)
    }
}

impl fmt::Display for ModelArch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Qwen35_0_8B => write!(f, "Qwen-3.5-0.8B (d=896)"),
            Self::Qwen35_4B => write!(f, "Qwen-3.5-4B (d=2048)"),
            Self::Gemma2_2B => write!(f, "Gemma-2-2B (d=2304)"),
            Self::Gemma2_9B => write!(f, "Gemma-2-9B (d=3584)"),
            Self::SmolLM2_135M => write!(f, "SmolLM2-135M (d=576)"),
            Self::Custom(dim) => write!(f, "CustomArch (d={dim})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_initialization_is_explicitly_untrained() {
        let bridge = ZWormholeBridge::new(ModelArch::Custom(16), ModelArch::Custom(24), 8);
        assert_eq!(bridge.bridge_kind(), BridgeKind::UntrainedDeterministic);
        assert!(!bridge.is_learned());
    }

    #[test]
    fn learned_bridge_validates_dimensions_and_executes() {
        let source_dim = 4;
        let target_dim = 3;
        let intermediate_dim = 2;
        let total_inter = 8 + intermediate_dim;

        let mut down = vec![0.0f32; source_dim * total_inter];
        let mut up = vec![0.0f32; total_inter * target_dim];
        for (row, chunk) in down.chunks_exact_mut(total_inter).enumerate() {
            chunk[row] = 0.25;
        }
        for (row, chunk) in up.chunks_exact_mut(target_dim).enumerate().take(target_dim) {
            chunk[row] = 2.0;
        }

        let weights = LearnedBridgeWeights {
            format_version: LearnedBridgeWeights::FORMAT_VERSION,
            source_dim,
            target_dim,
            intermediate_dim,
            proj_down: down,
            proj_up: up,
            training_fingerprint_sha256: "abc123".to_string(),
            source_model: "source-test".to_string(),
            target_model: "target-test".to_string(),
            heldout_cosine_mean: Some(0.9),
            heldout_top1_agreement: Some(0.8),
        };

        let bridge = ZWormholeBridge::from_learned_weights(
            ModelArch::Custom(source_dim),
            ModelArch::Custom(target_dim),
            weights,
        )
        .expect("learned bridge should validate");
        assert!(bridge.is_learned());

        let capsule = bridge
            .compress_thought(&[1.0, 2.0, 3.0, 4.0], 7)
            .expect("compression should succeed");
        let target = bridge
            .expand_thought(&capsule)
            .expect("expansion should succeed");
        assert_eq!(target.len(), target_dim);
        assert!(target.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn geodesic_distance_is_zero_for_identical_capsules() {
        let capsule = LatentThoughtCapsule::new(1, [3.0; 8], vec![0.0; 4], 0.5);
        assert!(capsule.geodesic_distance(&capsule) <= f32::EPSILON);
    }
}
