// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
#[cfg(test)]
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// 24-Byte Differential Swarm Intent Chirp Packet with 64-bit Epoch
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmIntentChirp {
    pub sender_node_id: u8,
    pub swarm_epoch: u64,
    pub target_domain: u8,
    pub target_subdomain: u8,
    pub action_opcode: u8,
    pub consensus_weight: u8,
    pub concept_trajectory: [u8; 6],
    pub state_crc: u32,
}

impl SwarmIntentChirp {
    pub fn new(
        sender: u8,
        epoch: u64,
        domain: u8,
        subdomain: u8,
        opcode: u8,
        coords: [u8; 6],
    ) -> Self {
        let mut chirp = Self {
            sender_node_id: sender,
            swarm_epoch: epoch,
            target_domain: domain & 0x0F,
            target_subdomain: subdomain & 0x0F,
            action_opcode: opcode,
            consensus_weight: 100,
            concept_trajectory: coords,
            state_crc: 0,
        };
        chirp.state_crc = chirp.compute_crc();
        chirp
    }

    pub fn to_bytes(&self) -> [u8; 24] {
        let mut bytes = [0u8; 24];
        bytes[0] = self.sender_node_id;
        bytes[1..9].copy_from_slice(&self.swarm_epoch.to_be_bytes());
        bytes[9] = (self.target_domain << 4) | (self.target_subdomain & 0x0F);
        bytes[10] = self.action_opcode;
        bytes[11] = self.consensus_weight;
        bytes[12..18].copy_from_slice(&self.concept_trajectory);
        let crc_bytes = self.state_crc.to_be_bytes();
        bytes[18..22].copy_from_slice(&crc_bytes);
        bytes[22] = 0x5A; // Swarm Sync Sentinel
        bytes[23] = 0xA5;
        bytes
    }

    pub fn from_bytes(bytes: &[u8; 24]) -> Result<Self, &'static str> {
        if bytes[22] != 0x5A || bytes[23] != 0xA5 {
            return Err("Invalid Swarm Sentinel Bytes");
        }
        let epoch = u64::from_be_bytes(bytes[1..9].try_into().unwrap());
        let crc = u32::from_be_bytes(bytes[18..22].try_into().unwrap());
        let mut coords = [0u8; 6];
        coords.copy_from_slice(&bytes[12..18]);

        let chirp = Self {
            sender_node_id: bytes[0],
            swarm_epoch: epoch,
            target_domain: (bytes[9] >> 4) & 0x0F,
            target_subdomain: bytes[9] & 0x0F,
            action_opcode: bytes[10],
            consensus_weight: bytes[11],
            concept_trajectory: coords,
            state_crc: crc,
        };

        if chirp.compute_crc() != crc {
            return Err("CRC Integrity Mismatch");
        }

        Ok(chirp)
    }

    fn compute_crc(&self) -> u32 {
        let mut hash = 0x811c9dc5u32;
        hash ^= self.sender_node_id as u32;
        hash = hash.wrapping_mul(0x01000193);
        for &b in &self.swarm_epoch.to_be_bytes() {
            hash ^= b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash ^= ((self.target_domain << 4) | self.target_subdomain) as u32;
        hash = hash.wrapping_mul(0x01000193);
        hash ^= self.action_opcode as u32;
        hash = hash.wrapping_mul(0x01000193);
        for &b in &self.concept_trajectory {
            hash ^= b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash
    }
}

/// Morphogenetic Ephemeral Subagent Spawner
pub struct EphemeralSubagentSpawner;

impl EphemeralSubagentSpawner {
    /// Cold-start spawn a lightweight subagent from a 381-byte procedural seed
    pub fn spawn_from_seed(seed_381: &[u8; 381], domain_id: u8) -> Vec<f32> {
        let mut synthesized_weights = vec![0.0f32; 1024];
        let mut prng_state = (seed_381[0] as u64) << 24
            | (seed_381[1] as u64) << 16
            | (seed_381[2] as u64) << 8
            | (seed_381[3] as u64);

        for (i, w) in synthesized_weights.iter_mut().enumerate() {
            prng_state = prng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let seed_byte = seed_381[(i + domain_id as usize) % 381] as f32 / 255.0;
            let normal_sample = ((prng_state >> 32) as f32 / (u32::MAX as f32)) - 0.5;
            *w = seed_byte * 0.1 + normal_sample * 0.05;
        }

        synthesized_weights
    }
}

/// Swarm Quorum Certificate establishing authenticated agreement across nodes with SHA-256 transcript binding
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmQuorumCertificate {
    pub epoch: u64,
    pub participant_nodes: Vec<u8>,
    pub total_weight: u32,
    pub consensus_trajectory: [u8; 6],
    pub certificate_transcript_sha256: [u8; 32],
}

/// Swarm Multi-Node Hypergraph Consensus Engine with Cryptographic Ed25519 Enforcement
pub struct SwarmConsensusEngine {
    pub registered_nodes: Vec<u8>,
    pub registered_keys: HashMap<u8, VerifyingKey>,
    pub pending_proposals: HashMap<u64, Vec<(SwarmIntentChirp, Signature)>>,
}

impl SwarmConsensusEngine {
    pub fn new(nodes: Vec<u8>) -> Self {
        Self {
            registered_nodes: nodes,
            registered_keys: HashMap::new(),
            pending_proposals: HashMap::new(),
        }
    }

    /// Register a cryptographic Ed25519 verifying key for a node
    pub fn register_key(&mut self, node_id: u8, key: VerifyingKey) {
        if !self.registered_nodes.contains(&node_id) {
            self.registered_nodes.push(node_id);
        }
        self.registered_keys.insert(node_id, key);
    }

    /// Submit a cryptographically signed swarm intent proposal with Ed25519 verification
    pub fn submit_signed_intent(
        &mut self,
        chirp: SwarmIntentChirp,
        signature: &Signature,
    ) -> Result<(), &'static str> {
        // Enforce cryptographic identity verification
        let verifying_key = self
            .registered_keys
            .get(&chirp.sender_node_id)
            .ok_or("Node public key not registered for cryptographic verification")?;

        let msg_bytes = chirp.to_bytes();
        verifying_key
            .verify(&msg_bytes, signature)
            .map_err(|_| "Ed25519 cryptographic signature verification failed: invalid signature for intent chirp")?;

        let proposals = self.pending_proposals.entry(chirp.swarm_epoch).or_default();

        // Enforce one vote per registered node per epoch (prevent duplicate/Sybil replay)
        if proposals
            .iter()
            .any(|(p, _)| p.sender_node_id == chirp.sender_node_id)
        {
            return Err(
                "Duplicate vote detected: node has already submitted a proposal for this epoch",
            );
        }

        proposals.push((chirp, *signature));
        Ok(())
    }

    /// Test-only submission helper for unauthenticated simulation harnesses
    #[cfg(test)]
    pub fn submit_test_intent(&mut self, chirp: SwarmIntentChirp) -> Result<(), &'static str> {
        if !self.registered_nodes.contains(&chirp.sender_node_id) {
            return Err("Unauthorized node ID in test harness");
        }
        let dummy_sig = Signature::from_bytes(&[0u8; 64]);
        let proposals = self.pending_proposals.entry(chirp.swarm_epoch).or_default();
        if proposals
            .iter()
            .any(|(p, _)| p.sender_node_id == chirp.sender_node_id)
        {
            return Err("Duplicate vote in test harness");
        }
        proposals.push((chirp, dummy_sig));
        Ok(())
    }

    /// Resolve weighted centroid consensus with cryptographic quorum certificate generation
    pub fn resolve_consensus(
        &self,
        epoch: u64,
        quorum_threshold: usize,
    ) -> Option<SwarmQuorumCertificate> {
        let proposals = self.pending_proposals.get(&epoch)?;
        if proposals.len() < quorum_threshold {
            return None;
        }

        let mut sum_coords = [0u32; 6];
        let mut total_weight = 0u32;
        let mut participants = Vec::new();
        let mut transcript_hasher = Sha256::new();

        transcript_hasher.update(b"ZYMATICA_SWARM_QUORUM_V2");
        transcript_hasher.update(epoch.to_be_bytes());

        for (p, sig) in proposals {
            let w = p.consensus_weight as u32;
            total_weight += w;
            participants.push(p.sender_node_id);
            transcript_hasher.update([p.sender_node_id]);
            transcript_hasher.update(p.to_bytes());
            transcript_hasher.update(sig.to_bytes());
            for (sum, &coord) in sum_coords.iter_mut().zip(&p.concept_trajectory) {
                *sum += (coord as u32) * w;
            }
        }

        if total_weight == 0 {
            return None;
        }

        let mut consensus_coords = [0u8; 6];
        for (coord, &sum) in consensus_coords.iter_mut().zip(&sum_coords) {
            *coord = ((sum + total_weight / 2) / total_weight) as u8;
        }

        let mut cert_hasher = Sha256::new();
        cert_hasher.update(transcript_hasher.finalize());
        let digest: [u8; 32] = cert_hasher.finalize().into();

        Some(SwarmQuorumCertificate {
            epoch,
            participant_nodes: participants,
            total_weight,
            consensus_trajectory: consensus_coords,
            certificate_transcript_sha256: digest,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn test_swarm_chirp_24b_crc_and_deserialization() {
        let chirp = SwarmIntentChirp::new(1, 100, 2, 4, 0x01, [1, 2, 3, 4, 5, 6]);
        let bytes = chirp.to_bytes();
        assert_eq!(bytes.len(), 24);

        let decoded = SwarmIntentChirp::from_bytes(&bytes).expect("Valid 24B chirp decode");
        assert_eq!(chirp, decoded);
    }

    #[test]
    fn test_ephemeral_subagent_spawning_determinism() {
        let mut seed = [0u8; 381];
        for (i, byte) in seed.iter_mut().enumerate() {
            *byte = ((i * 17 + 31) % 256) as u8;
        }

        let weights1 = EphemeralSubagentSpawner::spawn_from_seed(&seed, 3);
        let weights2 = EphemeralSubagentSpawner::spawn_from_seed(&seed, 3);
        assert_eq!(weights1, weights2);
        assert_eq!(weights1.len(), 1024);
    }

    #[test]
    fn test_swarm_ed25519_cryptographic_signature_consensus() {
        let mut csprng = OsRng;
        let signing_key1 = SigningKey::generate(&mut csprng);
        let signing_key2 = SigningKey::generate(&mut csprng);
        let signing_key3 = SigningKey::generate(&mut csprng);

        let mut engine = SwarmConsensusEngine::new(vec![]);
        engine.register_key(1, signing_key1.verifying_key());
        engine.register_key(2, signing_key2.verifying_key());
        engine.register_key(3, signing_key3.verifying_key());

        let c1 = SwarmIntentChirp::new(1, 20, 1, 1, 0x01, [10, 20, 30, 40, 50, 60]);
        let sig1 = signing_key1.sign(&c1.to_bytes());

        let c2 = SwarmIntentChirp::new(2, 20, 1, 1, 0x01, [12, 22, 32, 42, 52, 62]);
        let sig2 = signing_key2.sign(&c2.to_bytes());

        let c3 = SwarmIntentChirp::new(3, 20, 1, 1, 0x01, [11, 21, 31, 41, 51, 61]);
        let sig3 = signing_key3.sign(&c3.to_bytes());

        // Test forged signature from wrong key
        let forged_sig = signing_key2.sign(&c1.to_bytes());
        assert!(
            engine.submit_signed_intent(c1, &forged_sig).is_err(),
            "Forged signature must be rejected"
        );

        // Submit authentic signed intents
        assert!(engine.submit_signed_intent(c1, &sig1).is_ok());
        assert!(engine.submit_signed_intent(c2, &sig2).is_ok());
        assert!(engine.submit_signed_intent(c3, &sig3).is_ok());

        let cert = engine
            .resolve_consensus(20, 3)
            .expect("Quorum reached with valid Ed25519 signatures");
        assert_eq!(cert.consensus_trajectory, [11, 21, 31, 41, 51, 61]);
        assert_eq!(cert.participant_nodes, vec![1, 2, 3]);
        assert_ne!(cert.certificate_transcript_sha256, [0u8; 32]);
    }
}
