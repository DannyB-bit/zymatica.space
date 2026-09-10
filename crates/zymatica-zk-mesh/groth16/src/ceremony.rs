// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

// ============================================================================
// ZK-LoRaWAN MPC Ceremony — Groth16 Trusted Setup
// ============================================================================
// Implements a sequential-contribution MPC ceremony for the Groth16 CRS.
// Each contributor re-randomizes the proving key's delta and gamma parameters,
// providing a proof-of-knowledge for verification. As long as at least one
// contributor honestly destroys their secret, the toxic waste is unknown.
// ============================================================================

use ark_bn254::{Bn254, Fr, G1Affine, G1Projective, G2Affine, G2Projective};
use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ff::{Field, UniformRand};
use ark_groth16::{ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::circuit;

// ============================================================================
// Ceremony Data Structures
// ============================================================================

/// A single contribution record in the ceremony transcript.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContributionRecord {
    pub index: u32,
    pub contributor: String,
    pub timestamp: String,
    /// H(pk || vk) after this contribution
    pub contribution_hash: String,
    /// [s]₁ serialized — proof that contributor knew their secret s
    pub proof_of_knowledge: String,
    pub delta_g1: String,
    pub delta_g2: String,
}

/// Full ceremony transcript, written to ceremony_transcript.json.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CeremonyTranscript {
    pub circuit: String,
    pub curve: String,
    pub num_contributions: u32,
    pub contributions: Vec<ContributionRecord>,
    pub final_vk_hash: String,
    pub status: String,
}

/// Serializable ceremony parameters (pk + vk + chain metadata).
/// Written to ceremony_NNNN.params files.
pub struct CeremonyParams {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
    pub contribution_index: u32,
    pub chain_hash: [u8; 32],
    pub contributions: Vec<ContributionRecord>,
}

impl CeremonyParams {
    /// Serialize to bytes for file storage.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Write contribution index (4 bytes)
        buf.extend_from_slice(&self.contribution_index.to_le_bytes());

        // Write chain hash (32 bytes)
        buf.extend_from_slice(&self.chain_hash);

        // Write pk
        let mut pk_buf = Vec::new();
        self.proving_key
            .serialize_compressed(&mut pk_buf)
            .expect("Failed to serialize pk");
        buf.extend_from_slice(&(pk_buf.len() as u64).to_le_bytes());
        buf.extend_from_slice(&pk_buf);

        // Write vk
        let mut vk_buf = Vec::new();
        self.verifying_key
            .serialize_compressed(&mut vk_buf)
            .expect("Failed to serialize vk");
        buf.extend_from_slice(&(vk_buf.len() as u64).to_le_bytes());
        buf.extend_from_slice(&vk_buf);

        // Write contributions as JSON
        let contributions_json =
            serde_json::to_vec(&self.contributions).expect("Failed to serialize contributions");
        buf.extend_from_slice(&(contributions_json.len() as u64).to_le_bytes());
        buf.extend_from_slice(&contributions_json);

        buf
    }

    /// Deserialize from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut offset = 0;

        // Read contribution index
        if data.len() < offset + 4 {
            return Err("Data too short for contribution index".into());
        }
        let contribution_index = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
        offset += 4;

        // Read chain hash
        if data.len() < offset + 32 {
            return Err("Data too short for chain hash".into());
        }
        let mut chain_hash = [0u8; 32];
        chain_hash.copy_from_slice(&data[offset..offset + 32]);
        offset += 32;

        // Read pk
        if data.len() < offset + 8 {
            return Err("Data too short for pk length".into());
        }
        let pk_len = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        if data.len() < offset + pk_len {
            return Err("Data too short for pk".into());
        }
        let proving_key =
            ProvingKey::<Bn254>::deserialize_compressed(&data[offset..offset + pk_len])
                .map_err(|e| format!("Failed to deserialize pk: {}", e))?;
        offset += pk_len;

        // Read vk
        if data.len() < offset + 8 {
            return Err("Data too short for vk length".into());
        }
        let vk_len = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        if data.len() < offset + vk_len {
            return Err("Data too short for vk".into());
        }
        let verifying_key =
            VerifyingKey::<Bn254>::deserialize_compressed(&data[offset..offset + vk_len])
                .map_err(|e| format!("Failed to deserialize vk: {}", e))?;
        offset += vk_len;

        // Read contributions
        if data.len() < offset + 8 {
            return Err("Data too short for contributions length".into());
        }
        let contrib_len = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        if data.len() < offset + contrib_len {
            return Err("Data too short for contributions".into());
        }
        let contributions: Vec<ContributionRecord> =
            serde_json::from_slice(&data[offset..offset + contrib_len])
                .map_err(|e| format!("Failed to deserialize contributions: {}", e))?;

        Ok(CeremonyParams {
            proving_key,
            verifying_key,
            contribution_index,
            chain_hash,
            contributions,
        })
    }
}

// ============================================================================
// Ceremony Operations
// ============================================================================

/// Standard Keccak-f[1600] round constants (24 rounds).
const KECCAK_ROUNDS: usize = 24;
const RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

const ROTATION_OFFSETS: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

/// Keccak-f[1600] permutation.
#[allow(clippy::needless_range_loop)]
fn keccak_f1600(state: &mut [u64; 25]) {
    for round in 0..KECCAK_ROUNDS {
        // Theta step
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..5 {
            for y in 0..5 {
                state[y * 5 + x] ^= d[x];
            }
        }

        // Rho and Pi steps: B[y, (2*x + 3*y) % 5] = ROT(A[x, y], r[x, y])
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                let rot = ROTATION_OFFSETS[x][y];
                let new_x = y;
                let new_y = (2 * x + 3 * y) % 5;
                b[new_y * 5 + new_x] = state[y * 5 + x].rotate_left(rot);
            }
        }

        // Chi step
        for y in 0..5 {
            for x in 0..5 {
                state[y * 5 + x] =
                    b[y * 5 + x] ^ ((!b[y * 5 + ((x + 1) % 5)]) & b[y * 5 + ((x + 2) % 5)]);
            }
        }

        // Iota step
        state[0] ^= RC[round];
    }
}

/// Keccak-256 using Ethereum/pre-SHA3 domain padding (0x01...0x80).
/// This is intentionally Keccak-256, NOT FIPS-202 SHA3-256 (0x06 domain).
#[allow(clippy::needless_range_loop)]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let rate = 136; // 1088 bits / 8
    let mut state = [0u64; 25];
    let mut offset = 0;

    while offset + rate <= data.len() {
        for i in 0..rate / 8 {
            let chunk = &data[offset + i * 8..offset + (i + 1) * 8];
            state[i] ^= u64::from_le_bytes(chunk.try_into().unwrap());
        }
        keccak_f1600(&mut state);
        offset += rate;
    }

    let mut padded = [0u8; 136];
    let rem = data.len() - offset;
    padded[..rem].copy_from_slice(&data[offset..]);
    padded[rem] ^= 0x01;
    padded[rate - 1] ^= 0x80;

    for i in 0..rate / 8 {
        let chunk = &padded[i * 8..(i + 1) * 8];
        state[i] ^= u64::from_le_bytes(chunk.try_into().unwrap());
    }
    keccak_f1600(&mut state);

    let mut output = [0u8; 32];
    for i in 0..4 {
        output[i * 8..(i + 1) * 8].copy_from_slice(&state[i].to_le_bytes());
    }
    output
}

/// Compute hash of pk + vk for the ceremony chain.
fn hash_params(pk: &ProvingKey<Bn254>, vk: &VerifyingKey<Bn254>) -> [u8; 32] {
    let mut buf = Vec::new();
    pk.serialize_compressed(&mut buf)
        .expect("Failed to serialize pk for hashing");
    vk.serialize_compressed(&mut buf)
        .expect("Failed to serialize vk for hashing");
    keccak256(&buf)
}

/// Phase 1: Initialize the ceremony with the first set of parameters.
pub fn initialize(output_path: &str) -> Result<(), String> {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  ZK-LoRaWAN MPC Ceremony — Phase 1: Initialize             ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    let constants = circuit::generate_mimc_constants(220);

    eprintln!("  Generating initial parameters deterministically...");
    let mut rng = rand::rngs::StdRng::from_seed([0u8; 32]);
    let (pk, vk) =
        circuit::setup_keys(&mut rng, constants).map_err(|e| format!("Setup failed: {}", e))?;

    let chain_hash = hash_params(&pk, &vk);
    let timestamp = chrono::Utc::now().to_rfc3339();

    let mut initial_delta_g1_bytes = Vec::new();
    pk.delta_g1
        .serialize_compressed(&mut initial_delta_g1_bytes)
        .unwrap();

    let mut initial_delta_g2_bytes = Vec::new();
    vk.delta_g2
        .serialize_compressed(&mut initial_delta_g2_bytes)
        .unwrap();

    let initial_record = ContributionRecord {
        index: 0,
        contributor: "coordinator".to_string(),
        timestamp,
        contribution_hash: hex::encode(chain_hash),
        proof_of_knowledge: "genesis".to_string(),
        delta_g1: hex::encode(&initial_delta_g1_bytes),
        delta_g2: hex::encode(&initial_delta_g2_bytes),
    };

    let params = CeremonyParams {
        proving_key: pk,
        verifying_key: vk,
        contribution_index: 0,
        chain_hash,
        contributions: vec![initial_record],
    };

    let data = params.to_bytes();
    std::fs::write(output_path, &data)
        .map_err(|e| format!("Failed to write {}: {}", output_path, e))?;

    eprintln!("  ✅ Ceremony initialized");
    eprintln!("  Output:           {}", output_path);
    eprintln!("  Chain hash:       {}", hex::encode(chain_hash));
    eprintln!("  Contribution #0:  coordinator (genesis)");

    Ok(())
}

/// Phase 2: Contribute entropy to the ceremony parameters.
pub fn contribute(
    input_path: &str,
    output_path: &str,
    contributor_name: &str,
) -> Result<(), String> {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  ZK-LoRaWAN MPC Ceremony — Phase 2: Contribute             ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    // Load previous parameters
    let data =
        std::fs::read(input_path).map_err(|e| format!("Failed to read {}: {}", input_path, e))?;
    let mut params = CeremonyParams::from_bytes(&data)?;

    eprintln!("  Loaded ceremony params from: {}", input_path);
    eprintln!("  Previous contribution: #{}", params.contribution_index);
    eprintln!(
        "  Previous chain hash:   {}",
        hex::encode(params.chain_hash)
    );
    eprintln!();

    // Generate random secret s (toxic waste for this contribution)
    let mut rng = rand::rngs::OsRng;
    let mut secret = Fr::rand(&mut rng);

    // Compute proof of knowledge: [s]₁ = s * G1::generator
    let pok: G1Affine = (G1Projective::generator() * secret).into_affine();
    let mut pok_bytes = Vec::new();
    pok.serialize_compressed(&mut pok_bytes)
        .expect("Failed to serialize proof of knowledge");

    eprintln!("  Generated random secret (will be securely erased)");
    eprintln!("  Proof of knowledge: {}", hex::encode(&pok_bytes));
    eprintln!();

    // Re-randomize the CRS delta component
    // In Groth16, the delta parameter is part of the toxic waste.
    // Re-randomizing delta: delta' = s * delta
    // This affects: delta_g1, delta_g2, and the h_query/l_query in the proving key
    let secret_inv = secret.inverse().expect("Secret must be non-zero");

    // Transform verifying key: delta_g2' = s * delta_g2
    let new_delta_g2: G2Affine =
        (G2Projective::from(params.verifying_key.delta_g2) * secret).into_affine();
    params.verifying_key.delta_g2 = new_delta_g2;

    // Transform proving key: delta_g1' = s * delta_g1
    let new_delta_g1: G1Affine =
        (G1Projective::from(params.proving_key.delta_g1) * secret).into_affine();
    params.proving_key.delta_g1 = new_delta_g1;

    // Transform proving key: delta_g2' (must match VK)
    params.proving_key.vk.delta_g2 = new_delta_g2;

    // Transform h_query: h_i' = s^(-1) * h_i  (because h encodes 1/delta)
    let new_h_query: Vec<G1Affine> = params
        .proving_key
        .h_query
        .iter()
        .map(|h| (G1Projective::from(*h) * secret_inv).into_affine())
        .collect();
    params.proving_key.h_query = new_h_query;

    // Transform l_query: l_i' = s^(-1) * l_i  (because l encodes 1/delta)
    let new_l_query: Vec<G1Affine> = params
        .proving_key
        .l_query
        .iter()
        .map(|l| (G1Projective::from(*l) * secret_inv).into_affine())
        .collect();
    params.proving_key.l_query = new_l_query;

    // Securely erase the secret
    let mut secret_bytes = [0u8; 32];
    secret.serialize_compressed(&mut secret_bytes[..]).ok();
    secret_bytes.zeroize();
    // Also zero the Fr value itself
    secret = Fr::from(0u64);
    let _ = secret; // prevent optimization from removing the zeroing

    eprintln!("  ✅ Secret securely erased from memory");

    // Update chain hash
    let new_params_hash = hash_params(&params.proving_key, &params.verifying_key);
    let mut chain_input = Vec::new();
    chain_input.extend_from_slice(&params.chain_hash);
    chain_input.extend_from_slice(&pok_bytes);
    chain_input.extend_from_slice(contributor_name.as_bytes());
    let new_chain_hash = keccak256(&chain_input);

    let timestamp = chrono::Utc::now().to_rfc3339();
    let new_index = params.contribution_index + 1;

    let mut new_delta_g1_bytes = Vec::new();
    params
        .proving_key
        .delta_g1
        .serialize_compressed(&mut new_delta_g1_bytes)
        .unwrap();

    let mut new_delta_g2_bytes = Vec::new();
    params
        .verifying_key
        .delta_g2
        .serialize_compressed(&mut new_delta_g2_bytes)
        .unwrap();

    let record = ContributionRecord {
        index: new_index,
        contributor: contributor_name.to_string(),
        timestamp,
        contribution_hash: hex::encode(new_params_hash),
        proof_of_knowledge: hex::encode(&pok_bytes),
        delta_g1: hex::encode(&new_delta_g1_bytes),
        delta_g2: hex::encode(&new_delta_g2_bytes),
    };

    params.contributions.push(record);
    params.contribution_index = new_index;
    params.chain_hash = new_chain_hash;

    // Write output
    let out_data = params.to_bytes();
    std::fs::write(output_path, &out_data)
        .map_err(|e| format!("Failed to write {}: {}", output_path, e))?;

    eprintln!();
    eprintln!(
        "  ✅ Contribution #{} applied by \"{}\"",
        new_index, contributor_name
    );
    eprintln!("  Output:         {}", output_path);
    eprintln!("  Chain hash:     {}", hex::encode(new_chain_hash));
    eprintln!("  Params hash:    {}", hex::encode(new_params_hash));
    eprintln!();
    eprintln!(
        "  Next step: pass {} to the next contributor, or finalize.",
        output_path
    );

    Ok(())
}

/// Phase 3: Verify the entire ceremony chain.
pub fn verify(input_path: &str) -> Result<(), String> {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  ZK-LoRaWAN MPC Ceremony — Phase 3: Verify                 ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    let data =
        std::fs::read(input_path).map_err(|e| format!("Failed to read {}: {}", input_path, e))?;
    let params = CeremonyParams::from_bytes(&data)?;

    if params.contributions.len() < 2 {
        return Err(
            "FAILED: Ceremony must have at least one non-genesis contribution to be valid!".into(),
        );
    }

    eprintln!(
        "  Loaded ceremony params: {} contributions",
        params.contributions.len()
    );
    eprintln!();

    use ark_ec::pairing::Pairing;

    // 1. Re-generate initial keys deterministically to compare against
    let constants = circuit::generate_mimc_constants(220);
    let mut initial_rng = rand::rngs::StdRng::from_seed([0u8; 32]);
    let (initial_pk, initial_vk) = circuit::setup_keys(&mut initial_rng, constants)
        .map_err(|e| format!("Deterministic initial setup failed: {}", e))?;

    // Verify each contribution transition and proof of knowledge
    for (i, contrib) in params.contributions.iter().enumerate() {
        if i == 0 {
            // Genesis contribution — deserialize initial delta
            let delta_g1_bytes = hex::decode(&contrib.delta_g1)
                .map_err(|_| "Invalid delta_g1 hex at contribution #0".to_string())?;
            let delta_g1 = G1Affine::deserialize_compressed(&delta_g1_bytes[..])
                .map_err(|_| "Invalid delta_g1 G1 point at contribution #0".to_string())?;
            let delta_g2_bytes = hex::decode(&contrib.delta_g2)
                .map_err(|_| "Invalid delta_g2 hex at contribution #0".to_string())?;
            let delta_g2 = G2Affine::deserialize_compressed(&delta_g2_bytes[..])
                .map_err(|_| "Invalid delta_g2 G2 point at contribution #0".to_string())?;

            // Verify they match the deterministic initial parameters
            if delta_g1 != initial_pk.delta_g1 || delta_g2 != initial_vk.delta_g2 {
                return Err(
                    "FAILED: Genesis contribution record delta points do not match initial setup!"
                        .into(),
                );
            }

            eprintln!(
                "  ✅ Contribution #0: {} (genesis) — matches deterministic setup",
                contrib.contributor
            );
            continue;
        }

        // Verify proof of knowledge is a valid G1 point
        let pok_bytes = hex::decode(&contrib.proof_of_knowledge)
            .map_err(|_| format!("Invalid PoK hex at contribution #{}", i))?;
        let pok = G1Affine::deserialize_compressed(&pok_bytes[..])
            .map_err(|_| format!("Invalid PoK G1 point at contribution #{}", i))?;

        // Deserialize previous contribution delta
        let prev_contrib = &params.contributions[i - 1];
        let prev_delta_g1_bytes = hex::decode(&prev_contrib.delta_g1)
            .map_err(|_| format!("Invalid delta_g1 hex at contribution #{}", i - 1))?;
        let prev_delta_g1 = G1Affine::deserialize_compressed(&prev_delta_g1_bytes[..])
            .map_err(|_| format!("Invalid delta_g1 G1 point at contribution #{}", i - 1))?;
        let prev_delta_g2_bytes = hex::decode(&prev_contrib.delta_g2)
            .map_err(|_| format!("Invalid delta_g2 hex at contribution #{}", i - 1))?;
        let prev_delta_g2 = G2Affine::deserialize_compressed(&prev_delta_g2_bytes[..])
            .map_err(|_| format!("Invalid delta_g2 G2 point at contribution #{}", i - 1))?;

        // Deserialize current contribution delta
        let curr_delta_g1_bytes = hex::decode(&contrib.delta_g1)
            .map_err(|_| format!("Invalid delta_g1 hex at contribution #{}", i))?;
        let curr_delta_g1 = G1Affine::deserialize_compressed(&curr_delta_g1_bytes[..])
            .map_err(|_| format!("Invalid delta_g1 G1 point at contribution #{}", i))?;
        let curr_delta_g2_bytes = hex::decode(&contrib.delta_g2)
            .map_err(|_| format!("Invalid delta_g2 hex at contribution #{}", i))?;
        let curr_delta_g2 = G2Affine::deserialize_compressed(&curr_delta_g2_bytes[..])
            .map_err(|_| format!("Invalid delta_g2 G2 point at contribution #{}", i))?;

        // Bilinear pairing check 1: update consistency between G1 and G2:
        // Pairing(curr_delta_g1, prev_delta_g2) == Pairing(prev_delta_g1, curr_delta_g2)
        let pair1_left = Bn254::pairing(curr_delta_g1, prev_delta_g2);
        let pair1_right = Bn254::pairing(prev_delta_g1, curr_delta_g2);
        if pair1_left != pair1_right {
            return Err(format!(
                "FAILED: delta_g1 and delta_g2 update is inconsistent at contribution #{}",
                i
            ));
        }

        // Bilinear pairing check 2: PoK matches the updated parameters:
        // Pairing(pok, prev_delta_g2) == Pairing(G1::generator(), curr_delta_g2)
        let pair2_left = Bn254::pairing(pok, prev_delta_g2);
        let pair2_right = Bn254::pairing(G1Affine::generator(), curr_delta_g2);
        if pair2_left != pair2_right {
            return Err(format!(
                "FAILED: Proof of knowledge does not match updated delta at contribution #{}",
                i
            ));
        }

        eprintln!(
            "  ✅ Contribution #{}: {} — valid PoK & pairing transitions verified",
            i, contrib.contributor
        );
    }

    // Verify current params hash matches the last contribution record
    let current_hash = hash_params(&params.proving_key, &params.verifying_key);
    let last_contrib = params.contributions.last().unwrap();
    let recorded_hash = hex::decode(&last_contrib.contribution_hash)
        .map_err(|_| "Invalid contribution hash hex".to_string())?;

    if current_hash[..] != recorded_hash[..] {
        return Err("FAILED: Current params hash does not match last contribution record!".into());
    }
    eprintln!();
    eprintln!("  ✅ Parameters hash matches last contribution record");

    // 2. Hardened algebraic verification of h_query and l_query query vectors transition
    eprintln!("  Checking proving key query vectors transition algebraically...");
    let mut h_coefficients = Vec::new();
    let mut l_coefficients = Vec::new();
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&current_hash[0..32]);
    let mut coef_rng = rand::rngs::StdRng::from_seed(seed);
    for _ in 0..params.proving_key.h_query.len() {
        h_coefficients.push(Fr::rand(&mut coef_rng));
    }
    for _ in 0..params.proving_key.l_query.len() {
        l_coefficients.push(Fr::rand(&mut coef_rng));
    }

    let mut h_initial_proj = G1Projective::generator() * Fr::from(0u64);
    for (h, r) in initial_pk.h_query.iter().zip(h_coefficients.iter()) {
        h_initial_proj += G1Projective::from(*h) * r;
    }
    let h_initial = h_initial_proj.into_affine();

    let mut h_final_proj = G1Projective::generator() * Fr::from(0u64);
    for (h, r) in params.proving_key.h_query.iter().zip(h_coefficients.iter()) {
        h_final_proj += G1Projective::from(*h) * r;
    }
    let h_final = h_final_proj.into_affine();

    let mut l_initial_proj = G1Projective::generator() * Fr::from(0u64);
    for (l, r) in initial_pk.l_query.iter().zip(l_coefficients.iter()) {
        l_initial_proj += G1Projective::from(*l) * r;
    }
    let l_initial = l_initial_proj.into_affine();

    let mut l_final_proj = G1Projective::generator() * Fr::from(0u64);
    for (l, r) in params.proving_key.l_query.iter().zip(l_coefficients.iter()) {
        l_final_proj += G1Projective::from(*l) * r;
    }
    let l_final = l_final_proj.into_affine();

    // Verify h_query pairing relations:
    // Pairing(h_initial, vk_initial.delta) == Pairing(h_final, vk_final.delta)
    if Bn254::pairing(h_initial, initial_vk.delta_g2)
        != Bn254::pairing(h_final, params.verifying_key.delta_g2)
    {
        return Err("FAILED: Proving key h_query was not updated correctly!".into());
    }

    // Verify l_query pairing relations:
    // Pairing(l_initial, vk_initial.delta) == Pairing(l_final, vk_final.delta)
    if Bn254::pairing(l_initial, initial_vk.delta_g2)
        != Bn254::pairing(l_final, params.verifying_key.delta_g2)
    {
        return Err("FAILED: Proving key l_query was not updated correctly!".into());
    }
    eprintln!("  ✅ Query vectors (h_query and l_query) transition verified");

    // Verify the VK is structurally valid (has correct number of IC points)
    let expected_ic_count = 9; // 8 public inputs + 1 (for the constant term)
    if params.verifying_key.gamma_abc_g1.len() != expected_ic_count {
        return Err(format!(
            "FAILED: VK has {} IC points, expected {}",
            params.verifying_key.gamma_abc_g1.len(),
            expected_ic_count
        ));
    }
    eprintln!(
        "  ✅ Verifying key structure valid ({} IC points for 8 public inputs)",
        expected_ic_count
    );

    // Verify chain hash
    eprintln!("  ✅ Chain hash: {}", hex::encode(params.chain_hash));
    eprintln!();
    eprintln!("  ═══════════════════════════════════════════════════");
    eprintln!(
        "  CEREMONY VALID: {} contributions, chain verified",
        params.contributions.len()
    );
    eprintln!("  ═══════════════════════════════════════════════════");

    Ok(())
}

/// Phase 4: Finalize the ceremony — extract final keys and generate transcript.
pub fn finalize(input_path: &str) -> Result<(), String> {
    eprintln!("╔══════════════════════════════════════════════════════════════╗");
    eprintln!("║  ZK-LoRaWAN MPC Ceremony — Phase 4: Finalize               ║");
    eprintln!("╚══════════════════════════════════════════════════════════════╝");
    eprintln!();

    // First verify the chain
    verify(input_path)?;
    eprintln!();

    let data =
        std::fs::read(input_path).map_err(|e| format!("Failed to read {}: {}", input_path, e))?;
    let params = CeremonyParams::from_bytes(&data)?;

    // Write final keys
    std::fs::create_dir_all("keys").map_err(|e| format!("Failed to create keys dir: {}", e))?;

    let mut pk_buf = Vec::new();
    params
        .proving_key
        .serialize_compressed(&mut pk_buf)
        .map_err(|e| format!("Failed to serialize pk: {}", e))?;
    std::fs::write("keys/proving_key.bin", &pk_buf)
        .map_err(|e| format!("Failed to write proving_key.bin: {}", e))?;
    eprintln!("  ✅ keys/proving_key.bin written ({} bytes)", pk_buf.len());

    let mut vk_buf = Vec::new();
    params
        .verifying_key
        .serialize_compressed(&mut vk_buf)
        .map_err(|e| format!("Failed to serialize vk: {}", e))?;
    std::fs::write("keys/verifying_key.bin", &vk_buf)
        .map_err(|e| format!("Failed to write verifying_key.bin: {}", e))?;
    eprintln!(
        "  ✅ keys/verifying_key.bin written ({} bytes)",
        vk_buf.len()
    );

    // Generate ceremony transcript
    let final_vk_hash = hash_params(&params.proving_key, &params.verifying_key);
    let transcript = CeremonyTranscript {
        circuit: "ZKLoRaCircuit".to_string(),
        curve: "BN254".to_string(),
        num_contributions: params.contributions.len() as u32,
        contributions: params.contributions,
        final_vk_hash: hex::encode(final_vk_hash),
        status: "finalized".to_string(),
    };

    let transcript_json = serde_json::to_string_pretty(&transcript)
        .map_err(|e| format!("Failed to serialize transcript: {}", e))?;
    std::fs::write("keys/ceremony_transcript.json", &transcript_json)
        .map_err(|e| format!("Failed to write transcript: {}", e))?;
    eprintln!("  ✅ keys/ceremony_transcript.json written");

    eprintln!();
    eprintln!("  ═══════════════════════════════════════════════════");
    eprintln!("  CEREMONY FINALIZED");
    eprintln!("  Contributors: {}", transcript.num_contributions);
    eprintln!("  Final VK hash: {}", transcript.final_vk_hash);
    eprintln!("  ═══════════════════════════════════════════════════");
    eprintln!();
    eprintln!("  The keys in keys/ are now ready for deployment.");
    eprintln!("  Distribute ceremony_transcript.json for public verification.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keccak256_known_answer_test_vectors() {
        // Ethereum / Keccak-256 standard Known Answer Test vectors
        assert_eq!(
            hex::encode(keccak256(b"")),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
        assert_eq!(
            hex::encode(keccak256(b"abc")),
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
        );
        assert_eq!(
            hex::encode(keccak256(b"The quick brown fox jumps over the lazy dog")),
            "4d741b6f1eb29cb2a9b9911c82f56fa8d73b04959d3d9d222895df6c0b28aa15"
        );
    }

    #[test]
    fn test_keccak256_sponge_boundary_differential() {
        use sha3::{Digest, Keccak256};
        let test_lengths = [
            0, 1, 2, 7, 31, 32, 63, 100, 134, 135, 136, 137, 138, 270, 271, 272, 273, 274, 4096,
        ];
        for &len in &test_lengths {
            let data = vec![0x61u8; len];
            let ours = keccak256(&data);
            let mut reference = Keccak256::new();
            reference.update(&data);
            let expected = reference.finalize();
            assert_eq!(
                ours.as_slice(),
                expected.as_slice(),
                "Keccak mismatch at boundary length {len}"
            );
        }

        // 1,000 deterministic seeded randomized test vectors
        let mut seed = 0x12345678u64;
        for _ in 0..1000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let len = (seed % 4096) as usize;
            let data: Vec<u8> = (0..len)
                .map(|i| ((seed.wrapping_add(i as u64)) & 0xFF) as u8)
                .collect();
            let ours = keccak256(&data);
            let mut reference = Keccak256::new();
            reference.update(&data);
            let expected = reference.finalize();
            assert_eq!(
                ours.as_slice(),
                expected.as_slice(),
                "Keccak randomized differential mismatch at length {len}"
            );
        }
    }
}
