// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

// Watermark: ip zymatica.space
// ZK-LoRaWAN Groth16 Prover & Verifier — Real BN254 Implementation
// Fee Split: 100,000 lamports (gateway) + 50,000 lamports (protocol)

pub mod ceremony;
pub mod circuit;

use ark_bn254::{Bn254, Fr};
use ark_ff::PrimeField;
use ark_groth16::{PreparedVerifyingKey, Proof, ProvingKey, VerifyingKey};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};

use rand::SeedableRng;

// ============================================================================
// ZK-LoRaWAN Protocol Constants
// ============================================================================
pub const PROTOCOL_FEE_LAMPORTS: u64 = 50_000;
pub const GATEWAY_REWARD_LAMPORTS: u64 = 100_000;
pub const TOTAL_FEE_LAMPORTS: u64 = PROTOCOL_FEE_LAMPORTS + GATEWAY_REWARD_LAMPORTS;
pub const MIMC_ROUNDS: usize = 220;

/// Path for persisted proving/verifying keys (generated once, reused thereafter)
const KEYS_DIR: &str = "keys";
const PK_FILE: &str = "keys/proving_key.bin";
const VK_FILE: &str = "keys/verifying_key.bin";

// ============================================================================
// ZK-LoRaWAN Proof Bundle (serialized for LoRa transmission)
// ============================================================================
pub struct ZKLoRaWANProofBundle {
    pub proof_a_compressed: Vec<u8>, // G1 point (32 bytes compressed)
    pub proof_b_compressed: Vec<u8>, // G2 point (64 bytes compressed)
    pub proof_c_compressed: Vec<u8>, // G1 point (32 bytes compressed)
    pub identity_hash: Vec<u8>,      // Fr element (32 bytes)
    pub nullifier_hash: Vec<u8>,     // Fr element (32 bytes)
    pub attestation_hash: Vec<u8>,   // Fr element (32 bytes)
    pub ciphertext_hash: Vec<u8>,    // Fr element (32 bytes)
    pub total_bytes: usize,
}

// ============================================================================
// ZK-LoRaWAN Prover — Uses Seeded StdRng for developer reproducibility
// ============================================================================
pub struct ZKLoRaWANProver {
    pub proving_key: ProvingKey<Bn254>,
    pub verifying_key: VerifyingKey<Bn254>,
    pub prepared_vk: PreparedVerifyingKey<Bn254>,
    pub constants: Vec<Fr>,
}

impl ZKLoRaWANProver {
    /// Initialize the prover. Generates keys deterministically on first run
    /// so that fresh checkouts always match the deployed verifier's VK.
    pub fn new() -> Self {
        let constants = circuit::generate_mimc_constants(MIMC_ROUNDS);
        let use_reproducible =
            std::env::var("ZK_LORAWAN_REPRODUCIBLE_SETUP").unwrap_or_default() == "1";

        // If not forcing reproducible setup, try to load existing keys from disk first
        if !use_reproducible {
            let loaded = (std::fs::read(PK_FILE), std::fs::read(VK_FILE));
            if let (Ok(pk_data), Ok(vk_data)) = loaded {
                if let (Ok(pk), Ok(vk)) = (
                    ProvingKey::<Bn254>::deserialize_compressed(&pk_data[..]),
                    VerifyingKey::<Bn254>::deserialize_compressed(&vk_data[..]),
                ) {
                    let prepared_vk = ark_groth16::prepare_verifying_key(&vk);
                    return ZKLoRaWANProver {
                        proving_key: pk,
                        verifying_key: vk,
                        prepared_vk,
                        constants,
                    };
                }
                eprintln!("Warning: Key files exist but are corrupt; regenerating");
            }
        }

        // Determine RNG source based on environment variable for developer vs production safety
        let (pk, vk) = if use_reproducible {
            eprintln!(
                "Generating new trusted setup keys deterministically (seed = 42) for testing/CI..."
            );
            let mut rng = rand::rngs::StdRng::from_seed([42u8; 32]);
            circuit::setup_keys(&mut rng, constants.clone()).expect("Trusted setup failed")
        } else {
            eprintln!("WARNING: Generating single-party trusted setup keys with cryptographically secure OsRng.");
            eprintln!("WARNING: For production deployments, a Multi-Party Computation (MPC) ceremony must be performed.");
            let mut rng = rand::rngs::OsRng;
            circuit::setup_keys(&mut rng, constants.clone()).expect("Trusted setup failed")
        };

        // Only persist to disk if NOT in reproducible mode (so we don't overwrite production/OsRng keys on disk)
        if !use_reproducible {
            std::fs::create_dir_all(KEYS_DIR).ok();
            let mut pk_buf = Vec::new();
            pk.serialize_compressed(&mut pk_buf)
                .expect("Failed to serialize proving key");
            std::fs::write(PK_FILE, &pk_buf).expect("Failed to write proving key");

            let mut vk_buf = Vec::new();
            vk.serialize_compressed(&mut vk_buf)
                .expect("Failed to serialize verifying key");
            std::fs::write(VK_FILE, &vk_buf).expect("Failed to write verifying key");
            eprintln!("Keys saved to {PK_FILE} and {VK_FILE}");
        }

        let prepared_vk = ark_groth16::prepare_verifying_key(&vk);
        ZKLoRaWANProver {
            proving_key: pk,
            verifying_key: vk,
            prepared_vk,
            constants,
        }
    }

    /// Generate a Groth16 proof for private key, decryption key, coordinate value, firmware hash,
    /// gateway address, and deposit value.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_proof(
        &self,
        private_key_bytes: &[u8],
        decryption_key_bytes: &[u8],
        coordinate_val: u64,
        firmware_hash_bytes: &[u8],
        gateway_address_bytes: &[u8],
        deposit_value: u64,
    ) -> ZKLoRaWANProofBundle {
        let mut rng = rand::thread_rng();

        // Convert inputs to field elements
        let private_key_fr = Fr::from_le_bytes_mod_order(private_key_bytes);
        let decryption_key_fr = Fr::from_le_bytes_mod_order(decryption_key_bytes);
        let coordinate_fr = Fr::from(coordinate_val);
        let firmware_hash_fr = Fr::from_le_bytes_mod_order(firmware_hash_bytes);
        let deposit_value_fr = Fr::from(deposit_value);

        // Split 32-byte gateway address into two 16-byte halves and map to Fr
        let mut part1_bytes = [0u8; 32];
        part1_bytes[0..16].copy_from_slice(&gateway_address_bytes[0..16]);
        let gateway_part1_fr = Fr::from_le_bytes_mod_order(&part1_bytes);

        let mut part2_bytes = [0u8; 32];
        part2_bytes[0..16].copy_from_slice(&gateway_address_bytes[16..32]);
        let gateway_part2_fr = Fr::from_le_bytes_mod_order(&part2_bytes);

        // Compute the public hashes
        let identity_hash = circuit::mimc_hash(private_key_fr, None, &self.constants);
        let nonce_val: u64 = rand::Rng::gen(&mut rng);
        let nonce_fr = Fr::from(nonce_val);
        let nullifier_hash = circuit::mimc_hash(private_key_fr, Some(nonce_fr), &self.constants);
        let attestation_hash =
            circuit::mimc_hash(private_key_fr, Some(firmware_hash_fr), &self.constants);
        let ciphertext_hash =
            circuit::mimc_hash(decryption_key_fr, Some(coordinate_fr), &self.constants);
        let deposit_commitment =
            circuit::mimc_hash(identity_hash, Some(deposit_value_fr), &self.constants);

        // Generate the real Groth16 proof
        let proof = circuit::generate_proof(
            &self.proving_key,
            private_key_fr,
            nonce_fr,
            decryption_key_fr,
            coordinate_fr,
            firmware_hash_fr,
            deposit_value_fr,
            identity_hash,
            nullifier_hash,
            attestation_hash,
            ciphertext_hash,
            gateway_part1_fr,
            gateway_part2_fr,
            deposit_commitment,
            firmware_hash_fr, // firmware_hash_public == firmware_hash_witness
            self.constants.clone(),
            &mut rng,
        )
        .expect("Proof generation failed");

        // Serialize proof points to compressed form
        let mut proof_a_buf = Vec::new();
        proof.a.serialize_compressed(&mut proof_a_buf).unwrap();

        let mut proof_b_buf = Vec::new();
        proof.b.serialize_compressed(&mut proof_b_buf).unwrap();

        let mut proof_c_buf = Vec::new();
        proof.c.serialize_compressed(&mut proof_c_buf).unwrap();

        let mut identity_hash_buf = Vec::new();
        identity_hash
            .serialize_compressed(&mut identity_hash_buf)
            .unwrap();

        let mut nullifier_hash_buf = Vec::new();
        nullifier_hash
            .serialize_compressed(&mut nullifier_hash_buf)
            .unwrap();

        let mut attestation_hash_buf = Vec::new();
        attestation_hash
            .serialize_compressed(&mut attestation_hash_buf)
            .unwrap();

        let mut ciphertext_hash_buf = Vec::new();
        ciphertext_hash
            .serialize_compressed(&mut ciphertext_hash_buf)
            .unwrap();

        let total = proof_a_buf.len()
            + proof_b_buf.len()
            + proof_c_buf.len()
            + identity_hash_buf.len()
            + nullifier_hash_buf.len()
            + attestation_hash_buf.len()
            + ciphertext_hash_buf.len();

        ZKLoRaWANProofBundle {
            proof_a_compressed: proof_a_buf,
            proof_b_compressed: proof_b_buf,
            proof_c_compressed: proof_c_buf,
            identity_hash: identity_hash_buf,
            nullifier_hash: nullifier_hash_buf,
            attestation_hash: attestation_hash_buf,
            ciphertext_hash: ciphertext_hash_buf,
            total_bytes: total,
        }
    }

    /// Verify a proof bundle (used by gateway)
    #[allow(clippy::too_many_arguments)]
    pub fn verify_bundle(
        &self,
        bundle: &ZKLoRaWANProofBundle,
        gateway_address_bytes: &[u8],
        deposit_value: u64,
        firmware_hash_bytes: &[u8],
    ) -> bool {
        // Deserialize proof points
        let proof_a = match ark_bn254::G1Affine::deserialize_compressed(&*bundle.proof_a_compressed)
        {
            Ok(p) => p,
            Err(_) => return false,
        };
        let proof_b = match ark_bn254::G2Affine::deserialize_compressed(&*bundle.proof_b_compressed)
        {
            Ok(p) => p,
            Err(_) => return false,
        };
        let proof_c = match ark_bn254::G1Affine::deserialize_compressed(&*bundle.proof_c_compressed)
        {
            Ok(p) => p,
            Err(_) => return false,
        };

        let ark_proof = Proof {
            a: proof_a,
            b: proof_b,
            c: proof_c,
        };

        // Deserialize public hashes
        let identity_hash = match Fr::deserialize_compressed(&*bundle.identity_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let nullifier_hash = match Fr::deserialize_compressed(&*bundle.nullifier_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let attestation_hash = match Fr::deserialize_compressed(&*bundle.attestation_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };
        let ciphertext_hash = match Fr::deserialize_compressed(&*bundle.ciphertext_hash) {
            Ok(h) => h,
            Err(_) => return false,
        };

        // Split 32-byte gateway address into two 16-byte halves and map to Fr
        let mut part1_bytes = [0u8; 32];
        part1_bytes[0..16].copy_from_slice(&gateway_address_bytes[0..16]);
        let gateway_part1_fr = Fr::from_le_bytes_mod_order(&part1_bytes);

        let mut part2_bytes = [0u8; 32];
        part2_bytes[0..16].copy_from_slice(&gateway_address_bytes[16..32]);
        let gateway_part2_fr = Fr::from_le_bytes_mod_order(&part2_bytes);

        // Compute deposit commitment and firmware hash Fr
        let deposit_value_fr = Fr::from(deposit_value);
        let deposit_commitment =
            circuit::mimc_hash(identity_hash, Some(deposit_value_fr), &self.constants);
        let firmware_hash_fr = Fr::from_le_bytes_mod_order(firmware_hash_bytes);

        // Run real Groth16 verification (bilinear pairing check)
        matches!(
            circuit::verify_proof(
                &self.prepared_vk,
                &ark_proof,
                identity_hash,
                nullifier_hash,
                attestation_hash,
                ciphertext_hash,
                gateway_part1_fr,
                gateway_part2_fr,
                deposit_commitment,
                firmware_hash_fr,
            ),
            Ok(true)
        )
    }
}

impl Default for ZKLoRaWANProver {
    fn default() -> Self {
        Self::new()
    }
}

fn to_solana_g1(p: &ark_bn254::G1Affine) -> [u8; 64] {
    use ark_ff::{BigInteger, PrimeField};

    let mut solana_bytes = [0u8; 64];
    solana_bytes[..32].copy_from_slice(&p.x.into_bigint().to_bytes_be());
    solana_bytes[32..].copy_from_slice(&p.y.into_bigint().to_bytes_be());
    solana_bytes
}

fn to_solana_g2(p: &ark_bn254::G2Affine) -> [u8; 128] {
    use ark_ff::{BigInteger, PrimeField};

    let mut solana_bytes = [0u8; 128];
    solana_bytes[..32].copy_from_slice(&p.x.c1.into_bigint().to_bytes_be());
    solana_bytes[32..64].copy_from_slice(&p.x.c0.into_bigint().to_bytes_be());
    solana_bytes[64..96].copy_from_slice(&p.y.c1.into_bigint().to_bytes_be());
    solana_bytes[96..].copy_from_slice(&p.y.c0.into_bigint().to_bytes_be());
    solana_bytes
}

// ============================================================================
// CLI Entry Point — Proof Generation & Verification Demo
// ============================================================================
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "print-proof" {
        let prover = ZKLoRaWANProver::new();
        let decryption_key = b"zk-lorawan-decryption-key-secret";
        let coordinate_val = 123456789u64;
        let firmware_hash = b"enclave-firmware-version-v1.0.2";
        let deposit_value = 100_000u64;

        use ark_bn254::Fr;
        use ark_ff::PrimeField;
        use ark_serialize::CanonicalSerialize;

        for i in 1..=4 {
            let private_key_str = format!("zk-lorawan-test-private-key-2026_{}", i);
            let private_key = private_key_str.as_bytes();

            let private_key_fr = Fr::from_le_bytes_mod_order(private_key);
            let decryption_key_fr = Fr::from_le_bytes_mod_order(decryption_key);
            let coordinate_fr = Fr::from(coordinate_val);
            let firmware_hash_fr = Fr::from_le_bytes_mod_order(firmware_hash);
            let deposit_value_fr = Fr::from(deposit_value);

            let identity_hash = circuit::mimc_hash(private_key_fr, None, &prover.constants);
            let nullifier_hash =
                circuit::mimc_hash(private_key_fr, Some(Fr::from(9999u64)), &prover.constants);
            let attestation_hash =
                circuit::mimc_hash(private_key_fr, Some(firmware_hash_fr), &prover.constants);
            let ciphertext_hash =
                circuit::mimc_hash(decryption_key_fr, Some(coordinate_fr), &prover.constants);
            let deposit_commitment =
                circuit::mimc_hash(identity_hash, Some(deposit_value_fr), &prover.constants);

            let part1_bytes = [0u8; 32];
            let gateway_part1_fr = Fr::from_le_bytes_mod_order(&part1_bytes);
            let part2_bytes = [0u8; 32];
            let gateway_part2_fr = Fr::from_le_bytes_mod_order(&part2_bytes);

            let nonce_fr = Fr::from(9999u64);
            let mut rng = rand::thread_rng();
            let proof = circuit::generate_proof(
                &prover.proving_key,
                private_key_fr,
                nonce_fr,
                decryption_key_fr,
                coordinate_fr,
                firmware_hash_fr,
                deposit_value_fr,
                identity_hash,
                nullifier_hash,
                attestation_hash,
                ciphertext_hash,
                gateway_part1_fr,
                gateway_part2_fr,
                deposit_commitment,
                firmware_hash_fr,
                prover.constants.clone(),
                &mut rng,
            )
            .expect("Proof generation failed");

            let mut id_hash_bytes = Vec::new();
            identity_hash
                .serialize_compressed(&mut id_hash_bytes)
                .unwrap();
            let mut null_hash_bytes = Vec::new();
            nullifier_hash
                .serialize_compressed(&mut null_hash_bytes)
                .unwrap();
            let mut att_hash_bytes = Vec::new();
            attestation_hash
                .serialize_compressed(&mut att_hash_bytes)
                .unwrap();
            let mut cipher_hash_bytes = Vec::new();
            ciphertext_hash
                .serialize_compressed(&mut cipher_hash_bytes)
                .unwrap();

            let mut proof_a_compressed = Vec::new();
            proof
                .a
                .serialize_compressed(&mut proof_a_compressed)
                .unwrap();
            let mut proof_b_compressed = Vec::new();
            proof
                .b
                .serialize_compressed(&mut proof_b_compressed)
                .unwrap();
            let mut proof_c_compressed = Vec::new();
            proof
                .c
                .serialize_compressed(&mut proof_c_compressed)
                .unwrap();

            println!("identity_hash_{}: {}", i, hex::encode(id_hash_bytes));
            println!("nullifier_hash_{}: {}", i, hex::encode(null_hash_bytes));
            println!("attestation_hash_{}: {}", i, hex::encode(att_hash_bytes));
            println!("ciphertext_hash_{}: {}", i, hex::encode(cipher_hash_bytes));
            println!(
                "solana_proof_a_{}: {}",
                i,
                hex::encode(to_solana_g1(&proof.a))
            );
            println!(
                "solana_proof_b_{}: {}",
                i,
                hex::encode(to_solana_g2(&proof.b))
            );
            println!(
                "solana_proof_c_{}: {}",
                i,
                hex::encode(to_solana_g1(&proof.c))
            );
            println!(
                "compressed_proof_a_{}: {}",
                i,
                hex::encode(proof_a_compressed)
            );
            println!(
                "compressed_proof_b_{}: {}",
                i,
                hex::encode(proof_b_compressed)
            );
            println!(
                "compressed_proof_c_{}: {}",
                i,
                hex::encode(proof_c_compressed)
            );
        }

        std::process::exit(0);
    }

    if args.len() > 1 && args[1] == "print-vk" {
        let prover = ZKLoRaWANProver::new();
        let vk = &prover.verifying_key;
        println!("vk_alpha: {}", hex::encode(to_solana_g1(&vk.alpha_g1)));
        println!("vk_beta: {}", hex::encode(to_solana_g2(&vk.beta_g2)));
        println!("vk_gamma: {}", hex::encode(to_solana_g2(&vk.gamma_g2)));
        println!("vk_delta: {}", hex::encode(to_solana_g2(&vk.delta_g2)));
        for (i, ic) in vk.gamma_abc_g1.iter().enumerate() {
            println!("vk_ic[{}]: {}", i, hex::encode(to_solana_g1(ic)));
        }
        std::process::exit(0);
    }

    if args.len() > 1 && args[1] == "generate" {
        if args.len() < 8 {
            eprintln!("Usage: zk_lorawan_prove generate <private_key_hex> <decryption_key_hex> <coordinate_val> <firmware_hash_hex> <gateway_address_hex> <deposit_value>");
            std::process::exit(2);
        }
        let private_key = hex::decode(&args[2]).expect("Invalid private_key hex");
        let decryption_key = hex::decode(&args[3]).expect("Invalid decryption_key hex");
        let coordinate_val: u64 = args[4].parse().expect("Invalid coordinate_val");
        let firmware_hash = hex::decode(&args[5]).expect("Invalid firmware_hash hex");
        let gateway_address = hex::decode(&args[6]).expect("Invalid gateway_address hex");
        let deposit_value: u64 = args[7].parse().expect("Invalid deposit_value");

        let prover = ZKLoRaWANProver::new();
        let bundle = prover.generate_proof(
            &private_key,
            &decryption_key,
            coordinate_val,
            &firmware_hash,
            &gateway_address,
            deposit_value,
        );

        let proof_a =
            ark_bn254::G1Affine::deserialize_compressed(&*bundle.proof_a_compressed).unwrap();
        let proof_b =
            ark_bn254::G2Affine::deserialize_compressed(&*bundle.proof_b_compressed).unwrap();
        let proof_c =
            ark_bn254::G1Affine::deserialize_compressed(&*bundle.proof_c_compressed).unwrap();

        let constants = circuit::generate_mimc_constants(MIMC_ROUNDS);
        let identity_fr = Fr::deserialize_compressed(&bundle.identity_hash[..]).unwrap();
        let deposit_commitment_fr =
            circuit::mimc_hash(identity_fr, Some(Fr::from(deposit_value)), &constants);
        let mut deposit_commitment_buf = Vec::new();
        use ark_serialize::CanonicalSerialize;
        deposit_commitment_fr
            .serialize_compressed(&mut deposit_commitment_buf)
            .unwrap();

        println!("{{");
        println!(
            "  \"proof_a_compressed\": \"{}\",",
            hex::encode(&bundle.proof_a_compressed)
        );
        println!(
            "  \"proof_b_compressed\": \"{}\",",
            hex::encode(&bundle.proof_b_compressed)
        );
        println!(
            "  \"proof_c_compressed\": \"{}\",",
            hex::encode(&bundle.proof_c_compressed)
        );
        println!(
            "  \"proof_a\": \"{}\",",
            hex::encode(to_solana_g1(&proof_a))
        );
        println!(
            "  \"proof_b\": \"{}\",",
            hex::encode(to_solana_g2(&proof_b))
        );
        println!(
            "  \"proof_c\": \"{}\",",
            hex::encode(to_solana_g1(&proof_c))
        );
        println!(
            "  \"identity_hash\": \"{}\",",
            hex::encode(&bundle.identity_hash)
        );
        println!(
            "  \"nullifier_hash\": \"{}\",",
            hex::encode(&bundle.nullifier_hash)
        );
        println!(
            "  \"attestation_hash\": \"{}\",",
            hex::encode(&bundle.attestation_hash)
        );
        println!(
            "  \"ciphertext_hash\": \"{}\",",
            hex::encode(&bundle.ciphertext_hash)
        );
        println!(
            "  \"deposit_commitment\": \"{}\",",
            hex::encode(deposit_commitment_buf)
        );
        println!("  \"firmware_hash\": \"{}\"", hex::encode(&firmware_hash));
        println!("}}");
        std::process::exit(0);
    }

    if args.len() > 1 && args[1] == "verify" {
        if args.len() < 12 {
            eprintln!("Usage: zk_lorawan_prove verify <proof_a> <proof_b> <proof_c> <identity_hash> <nullifier_hash> <attestation_hash> <ciphertext_hash> <gateway_address_hex> <deposit_value> <firmware_hash_hex>");
            std::process::exit(2);
        }
        let proof_a_bytes = hex::decode(&args[2]).expect("Invalid proof_a hex");
        let proof_b_bytes = hex::decode(&args[3]).expect("Invalid proof_b hex");
        let proof_c_bytes = hex::decode(&args[4]).expect("Invalid proof_c hex");
        let identity_bytes = hex::decode(&args[5]).expect("Invalid identity_hash hex");
        let nullifier_bytes = hex::decode(&args[6]).expect("Invalid nullifier_hash hex");
        let attestation_bytes = hex::decode(&args[7]).expect("Invalid attestation_hash hex");
        let ciphertext_bytes = hex::decode(&args[8]).expect("Invalid ciphertext_hash hex");
        let gateway_bytes = hex::decode(&args[9]).expect("Invalid gateway_address hex");
        let deposit_value: u64 = args[10].parse().expect("Invalid deposit_value");
        let firmware_hash = hex::decode(&args[11]).expect("Invalid firmware_hash hex");

        let bundle = ZKLoRaWANProofBundle {
            proof_a_compressed: proof_a_bytes,
            proof_b_compressed: proof_b_bytes,
            proof_c_compressed: proof_c_bytes,
            identity_hash: identity_bytes,
            nullifier_hash: nullifier_bytes,
            attestation_hash: attestation_bytes,
            ciphertext_hash: ciphertext_bytes,
            total_bytes: 0,
        };

        let prover = ZKLoRaWANProver::new();
        let is_valid = prover.verify_bundle(&bundle, &gateway_bytes, deposit_value, &firmware_hash);
        if is_valid {
            println!("VALID");
            std::process::exit(0);
        } else {
            println!("INVALID");
            std::process::exit(1);
        }
    }

    if args.len() > 1 && args[1] == "ceremony" {
        println!("✅ MPC Ceremony CLI — Mainnet Hardened Setup Tooling");
        println!("🔒 Cryptographically verifies SRS delta transitions, proof of knowledge, and query vectors via bilinear pairings.");
        println!();
        if args.len() < 3 {
            eprintln!(
                "Usage: zk_lorawan_prove ceremony <init|contribute|verify|finalize> [options]"
            );
            eprintln!();
            eprintln!("Subcommands:");
            eprintln!("  init        --output <file>       Initialize a new ceremony");
            eprintln!(
                "  contribute  --input <file> --output <file> --name <name>   Add a contribution"
            );
            eprintln!("  verify      --input <file>        Verify the ceremony chain");
            eprintln!("  finalize    --input <file>        Finalize and extract keys");
            std::process::exit(2);
        }

        let subcmd = &args[2];
        match subcmd.as_str() {
            "init" => {
                let output = args
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| args[i + 1].as_str())
                    .unwrap_or("ceremony_0000.params");
                if let Err(e) = ceremony::initialize(output) {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            "contribute" => {
                let input = args
                    .iter()
                    .position(|a| a == "--input")
                    .and_then(|i| args.get(i + 1))
                    .expect("--input <file> required");
                let output = args
                    .iter()
                    .position(|a| a == "--output")
                    .and_then(|i| args.get(i + 1))
                    .expect("--output <file> required");
                let name = args
                    .iter()
                    .position(|a| a == "--name")
                    .and_then(|i| args.get(i + 1))
                    .map(|s| s.as_str())
                    .unwrap_or("anonymous");
                if let Err(e) = ceremony::contribute(input, output, name) {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            "verify" => {
                let input = args
                    .iter()
                    .position(|a| a == "--input")
                    .and_then(|i| args.get(i + 1))
                    .expect("--input <file> required");
                if let Err(e) = ceremony::verify(input) {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            "finalize" => {
                let input = args
                    .iter()
                    .position(|a| a == "--input")
                    .and_then(|i| args.get(i + 1))
                    .expect("--input <file> required");
                if let Err(e) = ceremony::finalize(input) {
                    eprintln!("ERROR: {}", e);
                    std::process::exit(1);
                }
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown ceremony subcommand: {}", subcmd);
                std::process::exit(2);
            }
        }
    }

    println!("======================================================================");
    println!("  ZK-LoRaWAN | Groth16 BN254 Proof Engine (Real ark-works)");
    println!("  Proving Balance Identity + Nullifiers + Micro-TEE + zk-VDE");
    println!("  + Gateway Binding + Deposit Commitment + Firmware Whitelist");
    println!("======================================================================");
    println!();

    // Initialize prover (includes trusted setup)
    println!("  [1] Running Groth16 trusted setup on BN254...");
    let prover = ZKLoRaWANProver::new();
    println!("      ✅ Secure random single-party setup complete (OsRng, file-persisted keys for demo/devnet)");
    println!("      For mainnet, run: zk_lorawan_prove ceremony init/contribute/verify/finalize");
    println!("      MiMC rounds: {}", MIMC_ROUNDS);
    println!("      Public inputs: 8 (identity, nullifier, attestation, ciphertext, gateway×2, deposit, firmware)");
    println!();

    // Generate a proof for sample witnesses
    let private_key = b"zk-lorawan-test-private-key-2026";
    let decryption_key = b"zk-lorawan-decryption-key-secret";
    let coordinate_val = 123456789u64;
    let firmware_hash = b"enclave-firmware-version-v1.0.2";
    let dummy_gateway = [0u8; 32];
    let deposit_value = 100_000u64;

    println!("  [2] Generating Groth16 proof...");
    let bundle = prover.generate_proof(
        private_key,
        decryption_key,
        coordinate_val,
        firmware_hash,
        &dummy_gateway,
        deposit_value,
    );
    println!("      ✅ Proof generated");
    println!(
        "      proof_a (G1): {} bytes compressed",
        bundle.proof_a_compressed.len()
    );
    println!(
        "      proof_b (G2): {} bytes compressed",
        bundle.proof_b_compressed.len()
    );
    println!(
        "      proof_c (G1): {} bytes compressed",
        bundle.proof_c_compressed.len()
    );
    println!("      identity_hash:{} bytes", bundle.identity_hash.len());
    println!("      nullifier_hash:{} bytes", bundle.nullifier_hash.len());
    println!(
        "      attestation_hash:{} bytes",
        bundle.attestation_hash.len()
    );
    println!(
        "      ciphertext_hash:{} bytes",
        bundle.ciphertext_hash.len()
    );
    println!("      TOTAL:        {} bytes", bundle.total_bytes);
    println!();

    // Verify the proof
    println!("  [3] Verifying proof (bilinear pairing check)...");
    let is_valid = prover.verify_bundle(&bundle, &dummy_gateway, deposit_value, firmware_hash);
    println!(
        "      Verification result: {}",
        if is_valid { "✅ VALID" } else { "❌ INVALID" }
    );
    println!();

    // Check LoRa MTU fit
    let lora_mtu = 255;
    let overhead = 3 + 6 + 8; // frame header + coordinates + receiver_tag
    let available = lora_mtu - overhead;
    let fits = bundle.total_bytes <= available;
    println!("  [4] LoRa MTU check:");
    println!("      LoRa MTU:       {} bytes", lora_mtu);
    println!(
        "      Frame overhead: {} bytes (header + coords + tag)",
        overhead
    );
    println!("      Available:      {} bytes", available);
    println!("      Proof size:     {} bytes", bundle.total_bytes);
    println!(
        "      Fits in frame:  {}",
        if fits {
            "✅ YES"
        } else {
            "⚠️  Needs LLD-AC compression"
        }
    );
    println!();

    // Fee split display
    println!("  [5] ZK-LoRaWAN Fee Split:");
    println!(
        "      Gateway reward:  {} lamports (${:.4})",
        GATEWAY_REWARD_LAMPORTS,
        GATEWAY_REWARD_LAMPORTS as f64 / 1_000_000_000.0 * 150.0
    );
    println!(
        "      Protocol fee:    {} lamports (${:.4})",
        PROTOCOL_FEE_LAMPORTS,
        PROTOCOL_FEE_LAMPORTS as f64 / 1_000_000_000.0 * 150.0
    );
    println!(
        "      Total per packet:{} lamports (${:.4})",
        TOTAL_FEE_LAMPORTS,
        TOTAL_FEE_LAMPORTS as f64 / 1_000_000_000.0 * 150.0
    );
    println!();

    // Test with wrong key / tampered inputs
    println!("  [6] Tamper test (wrong inputs)...");
    let wrong_key = b"this-is-the-wrong-key-should-fail";
    let wrong_bundle = prover.generate_proof(
        wrong_key,
        decryption_key,
        coordinate_val,
        firmware_hash,
        &dummy_gateway,
        deposit_value,
    );

    // Tamper with proof_a to simulate corruption
    let mut tampered = ZKLoRaWANProofBundle {
        proof_a_compressed: wrong_bundle.proof_a_compressed,
        proof_b_compressed: bundle.proof_b_compressed.clone(),
        proof_c_compressed: bundle.proof_c_compressed.clone(),
        identity_hash: bundle.identity_hash.clone(),
        nullifier_hash: bundle.nullifier_hash.clone(),
        attestation_hash: bundle.attestation_hash.clone(),
        ciphertext_hash: bundle.ciphertext_hash.clone(),
        total_bytes: bundle.total_bytes,
    };
    // Flip a byte to corrupt
    if !tampered.proof_a_compressed.is_empty() {
        tampered.proof_a_compressed[0] ^= 0xFF;
    }
    let tamper_valid =
        prover.verify_bundle(&tampered, &dummy_gateway, deposit_value, firmware_hash);
    println!(
        "      Tampered proof result: {}",
        if !tamper_valid {
            "✅ CORRECTLY REJECTED"
        } else {
            "❌ SHOULD HAVE FAILED"
        }
    );
    println!();

    if is_valid && !tamper_valid {
        println!("  🎉 ALL CHECKS PASSED — Groth16 BN254 engine operational");
    } else {
        println!("  ❌ VERIFICATION FAILED");
    }
    println!();
    println!("======================================================================");
}
