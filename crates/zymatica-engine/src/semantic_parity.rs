//! ==============================================================================
//! ZYMATICA CLASS 33: Z-SPAR (Engine Adapter & Unified Semantic Parity Bridge)
//! Author: Danny Bouldiez | Codebase by Devs One
//!
//! Provides the engine-level interface, re-exports, and conversions to the canonical,
//! standalone, dependency-free `zymatica_zspar` finite-field codec crate.
//! ==============================================================================

pub use zymatica_zspar::crc32c;
pub use zymatica_zspar::gf16::Gf16;
pub use zymatica_zspar::rs12_8::{DecodeResult, DecodeStatus, Rs12_8};
pub use zymatica_zspar::semantic::{
    Concept8D, InvariantKind, InvariantPatchFrame, InvariantRecord, InvariantSet, ParityOnlyFrame,
    RepairRequest, RepairResponse, RepairResult, RepairStatus, SystematicFrame, axis_diff_mask,
    semantic_tag, stable_text_id,
};
pub use zymatica_zspar::sha256;

use std::collections::HashMap;

/// Alias Concept8DState to canonical zymatica_zspar Concept8D
pub type Concept8DState = Concept8D;

/// Result of a Semantic Parity Syndrome Verification & Repair Pass
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticSyndromeResult {
    ExactMatch,
    SingleAxisRepaired {
        axis_index: usize,
        original_symbol: u8,
        repaired_symbol: u8,
    },
    TwoAxesRepaired {
        axis_1: usize,
        val_1: u8,
        axis_2: usize,
        val_2: u8,
    },
    IrrecoverableDrift {
        reason: String,
    },
}

/// Z-SPAR Semantic Codeword (12 nibbles = 8 data + 4 parity over GF(16))
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticCodeword8D {
    pub state: Concept8D,
    pub parity: [u8; 4],
}

impl SemanticCodeword8D {
    pub fn encode(state: Concept8D) -> Self {
        let syms = state.symbols();
        let encoded = Rs12_8::encode(&syms).expect("Valid RS(12,8) encoding");
        let parity = [encoded[8], encoded[9], encoded[10], encoded[11]];
        Self { state, parity }
    }

    pub fn to_bytes(&self) -> [u8; 6] {
        let syms = self.to_symbols();
        [
            (syms[0] << 4) | (syms[1] & 0x0F),
            (syms[2] << 4) | (syms[3] & 0x0F),
            (syms[4] << 4) | (syms[5] & 0x0F),
            (syms[6] << 4) | (syms[7] & 0x0F),
            (syms[8] << 4) | (syms[9] & 0x0F),
            (syms[10] << 4) | (syms[11] & 0x0F),
        ]
    }

    pub fn from_bytes(bytes: &[u8; 6]) -> Self {
        let state = Concept8D::new(
            (bytes[0] >> 4) & 0x0F,
            bytes[0] & 0x0F,
            (bytes[1] >> 4) & 0x0F,
            bytes[1] & 0x0F,
            (bytes[2] >> 4) & 0x0F,
            bytes[2] & 0x0F,
            (bytes[3] >> 4) & 0x0F,
            bytes[3] & 0x0F,
        );
        let parity = [
            (bytes[4] >> 4) & 0x0F,
            bytes[4] & 0x0F,
            (bytes[5] >> 4) & 0x0F,
            bytes[5] & 0x0F,
        ];
        Self { state, parity }
    }

    pub fn to_symbols(&self) -> [u8; 12] {
        let s = self.state.symbols();
        [
            s[0],
            s[1],
            s[2],
            s[3],
            s[4],
            s[5],
            s[6],
            s[7],
            self.parity[0],
            self.parity[1],
            self.parity[2],
            self.parity[3],
        ]
    }
}

/// Z-SPAR Semantic Parity Engine Adapter
pub struct ZSparEngine;

impl ZSparEngine {
    /// Compute 4-symbol GF(16) Parity Vector for an 8D semantic state
    pub fn compute_parity(state: Concept8D) -> [u8; 4] {
        let codeword = SemanticCodeword8D::encode(state);
        codeword.parity
    }

    /// Verify received/reconstructed state against expected parity, auto-repairing if within bounded distance
    pub fn verify_and_repair(
        reconstructed_state: Concept8D,
        authoritative_parity: [u8; 4],
    ) -> SemanticSyndromeResult {
        let s = reconstructed_state.symbols();
        let received_codeword = [
            s[0],
            s[1],
            s[2],
            s[3],
            s[4],
            s[5],
            s[6],
            s[7],
            authoritative_parity[0],
            authoritative_parity[1],
            authoritative_parity[2],
            authoritative_parity[3],
        ];

        let result = Rs12_8::decode(received_codeword, &[]);
        match result.status {
            DecodeStatus::Clean => SemanticSyndromeResult::ExactMatch,
            DecodeStatus::Corrected => {
                let repaired = Concept8D::from_symbols([
                    result.codeword[0],
                    result.codeword[1],
                    result.codeword[2],
                    result.codeword[3],
                    result.codeword[4],
                    result.codeword[5],
                    result.codeword[6],
                    result.codeword[7],
                ]);
                let mask = axis_diff_mask(&reconstructed_state, &repaired);
                let diff_indices: Vec<usize> = (0..8).filter(|&i| (mask & (1 << i)) != 0).collect();
                if diff_indices.len() == 1 {
                    let idx = diff_indices[0];
                    SemanticSyndromeResult::SingleAxisRepaired {
                        axis_index: idx,
                        original_symbol: s[idx],
                        repaired_symbol: result.codeword[idx],
                    }
                } else if diff_indices.len() == 2 {
                    let idx1 = diff_indices[0];
                    let idx2 = diff_indices[1];
                    SemanticSyndromeResult::TwoAxesRepaired {
                        axis_1: idx1,
                        val_1: result.codeword[idx1],
                        axis_2: idx2,
                        val_2: result.codeword[idx2],
                    }
                } else {
                    SemanticSyndromeResult::ExactMatch
                }
            }
            DecodeStatus::Uncorrectable | DecodeStatus::InvalidInput => {
                SemanticSyndromeResult::IrrecoverableDrift {
                    reason: "Semantic syndrome exceeds RS(12,8) correction bound (distance > 2) or invalid input"
                        .to_string(),
                }
            }
        }
    }
}

/// Critical Invariant Guard
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticInvariantGuard {
    pub negation_bit: bool,
    pub critical_numerical_values: HashMap<String, i64>,
    pub entity_hashes: HashMap<String, u32>,
}

impl SemanticInvariantGuard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn protect_entity(&mut self, entity_type: &str, entity_val: &str) {
        let mut hasher = 0x811c9dc5u32;
        for &b in entity_val.as_bytes() {
            hasher ^= b as u32;
            hasher = hasher.wrapping_mul(0x01000193);
        }
        self.entity_hashes.insert(entity_type.to_string(), hasher);
    }

    pub fn protect_number(&mut self, parameter_name: &str, value: i64) {
        self.critical_numerical_values
            .insert(parameter_name.to_string(), value);
    }

    pub fn set_negation(&mut self, is_negated: bool) {
        self.negation_bit = is_negated;
    }

    pub fn verify_invariants(&self, other: &Self) -> bool {
        if self.negation_bit != other.negation_bit {
            return false;
        }
        for (k, v) in &self.critical_numerical_values {
            if other.critical_numerical_values.get(k) != Some(v) {
                return false;
            }
        }
        for (k, v) in &self.entity_hashes {
            if other.entity_hashes.get(k) != Some(v) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_z_spar_bridge_roundtrip() {
        let original = Concept8D::new(1, 2, 8, 4, 12, 1, 6, 15);
        let codeword = SemanticCodeword8D::encode(original);

        let bytes = codeword.to_bytes();
        assert_eq!(bytes.len(), 6);

        let decoded = SemanticCodeword8D::from_bytes(&bytes);
        assert_eq!(decoded.state, original);

        let verify_result = ZSparEngine::verify_and_repair(decoded.state, decoded.parity);
        assert_eq!(verify_result, SemanticSyndromeResult::ExactMatch);
    }

    #[test]
    fn test_canonical_z_spar_single_axis_repair() {
        let original = Concept8D::new(1, 4, 8, 15, 10, 1, 2, 14);
        let authoritative_parity = ZSparEngine::compute_parity(original);

        let drifted_state = Concept8D::new(1, 4, 3, 15, 10, 1, 2, 14);
        let repair_result = ZSparEngine::verify_and_repair(drifted_state, authoritative_parity);

        match repair_result {
            SemanticSyndromeResult::SingleAxisRepaired {
                axis_index,
                original_symbol,
                repaired_symbol,
            } => {
                assert_eq!(axis_index, 2);
                assert_eq!(original_symbol, 3);
                assert_eq!(repaired_symbol, 8);
            }
            other => panic!("Expected single axis repair, got {:?}", other),
        }
    }
}
