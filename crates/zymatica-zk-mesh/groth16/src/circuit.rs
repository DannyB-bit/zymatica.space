// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

// ============================================================================
// ZK-LoRaWAN Groth16 Circuit — Hardened Production Circuit
// ============================================================================
// Public inputs (8):
//   1. identity_hash        = MiMC(private_key)
//   2. nullifier_hash       = MiMC(private_key + nonce)
//   3. attestation_hash     = MiMC(private_key + firmware_hash)
//   4. ciphertext_hash      = MiMC(decryption_key + coordinate_val)
//   5. gateway_part1        = lower 16 bytes of gateway Pubkey as Fr
//   6. gateway_part2        = upper 16 bytes of gateway Pubkey as Fr
//   7. deposit_commitment   = MiMC(identity_hash + deposit_value) [pool drain prevention]
//   8. firmware_hash_public = firmware hash (on-chain whitelist check)
// ============================================================================

use ark_bn254::{Bn254, Fr};
use ark_ff::{Field, PrimeField};
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError, Variable};
use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
use ark_std::rand::{CryptoRng, Rng};

/// ZKLoRaCircuit defines the hardened ZK-SNARK circuit for the ZK-LoRaWAN protocol.
/// It proves:
///   1. Balance Owner Identity: knowledge of private key matching public identity.
///   2. Nullifier Generation: unique nullifier to prevent double spending.
///   3. Micro-TEE Attestation: firmware hash binds to private key identity.
///   4. Private coordinate commitment: knowledge of decryption_key + coordinate_val matching
///      the public ciphertext/coordinate commitment. This is not described as a VDE unless a
///      verifiable-delay primitive is separately implemented and constrained.
///   5. Gateway Binding: proof is bound to a specific gateway address.
///   6. Deposit Commitment: depositor's identity is bound to a value (pool drain prevention).
///   7. Firmware Whitelist: firmware hash is exposed for on-chain whitelist verification.
pub struct ZKLoRaCircuit<F: PrimeField> {
    // Secret inputs (witnesses)
    pub private_key: Option<F>,
    pub nonce: Option<F>,
    pub decryption_key: Option<F>,
    pub coordinate_val: Option<F>,
    pub firmware_hash_witness: Option<F>,
    pub deposit_value: Option<F>,

    // Public inputs
    pub identity_hash: Option<F>,
    pub nullifier_hash: Option<F>,
    pub attestation_hash: Option<F>,
    pub ciphertext_hash: Option<F>,
    pub gateway_part1: Option<F>,
    pub gateway_part2: Option<F>,
    pub deposit_commitment: Option<F>,
    pub firmware_hash_public: Option<F>,

    pub round_constants: Vec<F>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for ZKLoRaCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // Allocate core private witnesses
        let private_key_var =
            cs.new_witness_variable(|| self.private_key.ok_or(SynthesisError::AssignmentMissing))?;

        let nonce_var =
            cs.new_witness_variable(|| self.nonce.ok_or(SynthesisError::AssignmentMissing))?;

        let decryption_key_var = cs.new_witness_variable(|| {
            self.decryption_key.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let coordinate_val_var = cs.new_witness_variable(|| {
            self.coordinate_val.ok_or(SynthesisError::AssignmentMissing)
        })?;

        let firmware_hash_witness_var = cs.new_witness_variable(|| {
            self.firmware_hash_witness
                .ok_or(SynthesisError::AssignmentMissing)
        })?;

        let deposit_value_var = cs
            .new_witness_variable(|| self.deposit_value.ok_or(SynthesisError::AssignmentMissing))?;

        // Helper to enforce MiMC-based hash constraints: outputs = mimc(preimage + salt)
        let mimc_hash_constraints = |cs: &ConstraintSystemRef<F>,
                                     initial_lc: ark_relations::r1cs::LinearCombination<F>,
                                     initial_val: Option<F>|
         -> Result<Variable, SynthesisError> {
            let mut current_val = initial_val;
            let mut current_var =
                cs.new_witness_variable(|| current_val.ok_or(SynthesisError::AssignmentMissing))?;

            // Constrain current_var to match the initial preimage + salt combination
            cs.enforce_constraint(current_var.into(), Variable::One.into(), initial_lc)?;

            for &c in &self.round_constants {
                let sq_val = current_val.map(|x| {
                    let mut tmp = x;
                    tmp += &c;
                    tmp.square()
                });

                let next_val = current_val.map(|x| {
                    let mut tmp = x;
                    tmp += &c;
                    let mut cube = tmp;
                    cube.square_in_place();
                    cube *= &tmp;
                    cube
                });

                let sq_var =
                    cs.new_witness_variable(|| sq_val.ok_or(SynthesisError::AssignmentMissing))?;

                let next_var =
                    cs.new_witness_variable(|| next_val.ok_or(SynthesisError::AssignmentMissing))?;

                let mut lc1 = ark_relations::r1cs::LinearCombination::from(current_var);
                lc1 += (c, Variable::One);

                cs.enforce_constraint(lc1.clone(), lc1.clone(), sq_var.into())?;
                cs.enforce_constraint(sq_var.into(), lc1, next_var.into())?;

                current_var = next_var;
                current_val = next_val;
            }

            Ok(current_var)
        };

        // 1. Balance Owner Identity Verification: identity_hash = MiMC(private_key)
        let initial_lc_1 = ark_relations::r1cs::LinearCombination::from(private_key_var);
        let calculated_identity_var = mimc_hash_constraints(&cs, initial_lc_1, self.private_key)?;
        let identity_hash_var =
            cs.new_input_variable(|| self.identity_hash.ok_or(SynthesisError::AssignmentMissing))?;
        cs.enforce_constraint(
            calculated_identity_var.into(),
            Variable::One.into(),
            identity_hash_var.into(),
        )?;

        // 2. Nullifier Verification: nullifier_hash = MiMC(private_key + nonce)
        let mut initial_lc_2 = ark_relations::r1cs::LinearCombination::from(private_key_var);
        initial_lc_2 += (F::one(), nonce_var);
        let val_2 = match (self.private_key, self.nonce) {
            (Some(pk), Some(n)) => Some(pk + n),
            _ => None,
        };
        let calculated_nullifier_var = mimc_hash_constraints(&cs, initial_lc_2, val_2)?;
        let nullifier_hash_var =
            cs.new_input_variable(|| self.nullifier_hash.ok_or(SynthesisError::AssignmentMissing))?;
        cs.enforce_constraint(
            calculated_nullifier_var.into(),
            Variable::One.into(),
            nullifier_hash_var.into(),
        )?;

        // 3. Micro-TEE Attestation: attestation_hash = MiMC(private_key + firmware_hash)
        let mut initial_lc_3 = ark_relations::r1cs::LinearCombination::from(private_key_var);
        initial_lc_3 += (F::one(), firmware_hash_witness_var);
        let val_3 = match (self.private_key, self.firmware_hash_witness) {
            (Some(pk), Some(fh)) => Some(pk + fh),
            _ => None,
        };
        let calculated_attestation_var = mimc_hash_constraints(&cs, initial_lc_3, val_3)?;
        let attestation_hash_var = cs.new_input_variable(|| {
            self.attestation_hash
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        cs.enforce_constraint(
            calculated_attestation_var.into(),
            Variable::One.into(),
            attestation_hash_var.into(),
        )?;

        // 4. zk-VDE Decryption: ciphertext_hash = MiMC(decryption_key + coordinate_val)
        let mut initial_lc_4 = ark_relations::r1cs::LinearCombination::from(decryption_key_var);
        initial_lc_4 += (F::one(), coordinate_val_var);
        let val_4 = match (self.decryption_key, self.coordinate_val) {
            (Some(dk), Some(cv)) => Some(dk + cv),
            _ => None,
        };
        let calculated_ciphertext_var = mimc_hash_constraints(&cs, initial_lc_4, val_4)?;
        let ciphertext_hash_var = cs.new_input_variable(|| {
            self.ciphertext_hash
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        cs.enforce_constraint(
            calculated_ciphertext_var.into(),
            Variable::One.into(),
            ciphertext_hash_var.into(),
        )?;

        // 5. Gateway Binding (R1CS Gate)
        let gateway_part1_var =
            cs.new_input_variable(|| self.gateway_part1.ok_or(SynthesisError::AssignmentMissing))?;
        let gateway_part2_var =
            cs.new_input_variable(|| self.gateway_part2.ok_or(SynthesisError::AssignmentMissing))?;
        let mut gw_lc = ark_relations::r1cs::LinearCombination::from(gateway_part1_var);
        gw_lc += (F::one(), gateway_part2_var);
        gw_lc += (F::one(), private_key_var);
        let val_gw = match (self.gateway_part1, self.gateway_part2, self.private_key) {
            (Some(g1), Some(g2), Some(pk)) => Some(g1 + g2 + pk),
            _ => None,
        };
        let _calculated_gw_var = mimc_hash_constraints(&cs, gw_lc, val_gw)?;

        // 6. Deposit Commitment: deposit_commitment = MiMC(identity_hash + deposit_value)
        let mut initial_lc_5 =
            ark_relations::r1cs::LinearCombination::from(calculated_identity_var);
        initial_lc_5 += (F::one(), deposit_value_var);
        let val_5 = match (self.identity_hash, self.deposit_value) {
            (Some(ih), Some(dv)) => Some(ih + dv),
            _ => None,
        };
        let calculated_deposit_var = mimc_hash_constraints(&cs, initial_lc_5, val_5)?;
        let deposit_commitment_var = cs.new_input_variable(|| {
            self.deposit_commitment
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        cs.enforce_constraint(
            calculated_deposit_var.into(),
            Variable::One.into(),
            deposit_commitment_var.into(),
        )?;

        // 7. Firmware Hash Public
        let firmware_hash_public_var = cs.new_input_variable(|| {
            self.firmware_hash_public
                .ok_or(SynthesisError::AssignmentMissing)
        })?;
        cs.enforce_constraint(
            firmware_hash_witness_var.into(),
            Variable::One.into(),
            firmware_hash_public_var.into(),
        )?;

        Ok(())
    }
}

/// Generates deterministic round constants for the MiMC hash function.
pub fn generate_mimc_constants(num_rounds: usize) -> Vec<Fr> {
    let mut constants = Vec::with_capacity(num_rounds);
    for i in 0..num_rounds {
        constants.push(Fr::from((i + 123456789) as u64));
    }
    constants
}

/// Computes the MiMC hash of a preimage in the BN254 scalar field.
pub fn mimc_hash(preimage: Fr, salt: Option<Fr>, constants: &[Fr]) -> Fr {
    let mut current = if let Some(s) = salt {
        preimage + s
    } else {
        preimage
    };
    for &c in constants {
        let mut tmp = current;
        tmp += &c;
        let mut cube = tmp;
        cube.square_in_place();
        cube *= &tmp;
        current = cube;
    }
    current
}

/// Performs the trusted setup for the ZK-LoRa circuit.
pub fn setup_keys<R: Rng + CryptoRng>(
    rng: &mut R,
    constants: Vec<Fr>,
) -> Result<(ProvingKey<Bn254>, VerifyingKey<Bn254>), SynthesisError> {
    let empty_circuit = ZKLoRaCircuit {
        private_key: None,
        nonce: None,
        decryption_key: None,
        coordinate_val: None,
        firmware_hash_witness: None,
        deposit_value: None,
        identity_hash: None,
        nullifier_hash: None,
        attestation_hash: None,
        ciphertext_hash: None,
        gateway_part1: None,
        gateway_part2: None,
        deposit_commitment: None,
        firmware_hash_public: None,
        round_constants: constants,
    };

    Groth16::<Bn254>::setup(empty_circuit, rng)
}

/// Generates a Groth16 proof for the ZK-LoRa circuit.
#[allow(clippy::too_many_arguments)]
pub fn generate_proof<R: Rng + CryptoRng>(
    pk: &ProvingKey<Bn254>,
    private_key: Fr,
    nonce: Fr,
    decryption_key: Fr,
    coordinate_val: Fr,
    firmware_hash: Fr,
    deposit_value: Fr,
    identity_hash: Fr,
    nullifier_hash: Fr,
    attestation_hash: Fr,
    ciphertext_hash: Fr,
    gateway_part1: Fr,
    gateway_part2: Fr,
    deposit_commitment: Fr,
    firmware_hash_public: Fr,
    constants: Vec<Fr>,
    rng: &mut R,
) -> Result<Proof<Bn254>, SynthesisError> {
    let circuit = ZKLoRaCircuit {
        private_key: Some(private_key),
        nonce: Some(nonce),
        decryption_key: Some(decryption_key),
        coordinate_val: Some(coordinate_val),
        firmware_hash_witness: Some(firmware_hash),
        deposit_value: Some(deposit_value),
        identity_hash: Some(identity_hash),
        nullifier_hash: Some(nullifier_hash),
        attestation_hash: Some(attestation_hash),
        ciphertext_hash: Some(ciphertext_hash),
        gateway_part1: Some(gateway_part1),
        gateway_part2: Some(gateway_part2),
        deposit_commitment: Some(deposit_commitment),
        firmware_hash_public: Some(firmware_hash_public),
        round_constants: constants,
    };

    Groth16::<Bn254>::prove(pk, circuit, rng)
}

/// Verifies a Groth16 proof against public inputs.
#[allow(clippy::too_many_arguments)]
pub fn verify_proof(
    pvk: &PreparedVerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    identity_hash: Fr,
    nullifier_hash: Fr,
    attestation_hash: Fr,
    ciphertext_hash: Fr,
    gateway_part1: Fr,
    gateway_part2: Fr,
    deposit_commitment: Fr,
    firmware_hash_public: Fr,
) -> Result<bool, SynthesisError> {
    let public_inputs = vec![
        identity_hash,
        nullifier_hash,
        attestation_hash,
        ciphertext_hash,
        gateway_part1,
        gateway_part2,
        deposit_commitment,
        firmware_hash_public,
    ];
    Groth16::<Bn254>::verify_with_processed_vk(pvk, &public_inputs, proof)
}

#[cfg(test)]
mod binding_tests {
    use super::*;
    use ark_groth16::prepare_verifying_key;
    use ark_relations::r1cs::ConstraintSystem;
    use rand::{rngs::StdRng, SeedableRng};

    struct Fixture {
        pvk: PreparedVerifyingKey<Bn254>,
        proof: Proof<Bn254>,
        public: [Fr; 8],
    }

    fn fixture() -> Fixture {
        let constants = generate_mimc_constants(32);
        let private_key = Fr::from(11u64);
        let nonce = Fr::from(12u64);
        let decryption_key = Fr::from(13u64);
        let coordinate_val = Fr::from(14u64);
        let firmware_hash = Fr::from(15u64);
        let deposit_value = Fr::from(16u64);
        let gateway_part1 = Fr::from(17u64);
        let gateway_part2 = Fr::from(18u64);

        let identity_hash = mimc_hash(private_key, None, &constants);
        let nullifier_hash = mimc_hash(private_key, Some(nonce), &constants);
        let attestation_hash = mimc_hash(private_key, Some(firmware_hash), &constants);
        let ciphertext_hash = mimc_hash(decryption_key, Some(coordinate_val), &constants);
        let deposit_commitment = mimc_hash(identity_hash, Some(deposit_value), &constants);
        let public = [
            identity_hash,
            nullifier_hash,
            attestation_hash,
            ciphertext_hash,
            gateway_part1,
            gateway_part2,
            deposit_commitment,
            firmware_hash,
        ];

        let mut rng = StdRng::from_seed([7u8; 32]);
        let (pk, vk) = setup_keys(&mut rng, constants.clone()).expect("setup must succeed");
        let proof = generate_proof(
            &pk,
            private_key,
            nonce,
            decryption_key,
            coordinate_val,
            firmware_hash,
            deposit_value,
            identity_hash,
            nullifier_hash,
            attestation_hash,
            ciphertext_hash,
            gateway_part1,
            gateway_part2,
            deposit_commitment,
            firmware_hash,
            constants,
            &mut rng,
        )
        .expect("proof generation must succeed");

        Fixture {
            pvk: prepare_verifying_key(&vk),
            proof,
            public,
        }
    }

    fn verify_array(fixture: &Fixture, public: [Fr; 8]) -> bool {
        verify_proof(
            &fixture.pvk,
            &fixture.proof,
            public[0],
            public[1],
            public[2],
            public[3],
            public[4],
            public[5],
            public[6],
            public[7],
        )
        .expect("verification call must not error")
    }

    #[test]
    fn every_public_input_is_proof_bound() {
        let fixture = fixture();
        assert!(verify_array(&fixture, fixture.public));
        for index in 0..fixture.public.len() {
            let mut mutated = fixture.public;
            mutated[index] += Fr::from(1u64);
            assert!(
                !verify_array(&fixture, mutated),
                "mutating public input index {index} must invalidate the proof"
            );
        }
    }

    #[test]
    fn wrong_nullifier_for_nonce_makes_r1cs_unsatisfied() {
        let constants = generate_mimc_constants(32);
        let private_key = Fr::from(21u64);
        let nonce = Fr::from(22u64);
        let firmware_hash = Fr::from(23u64);
        let decryption_key = Fr::from(24u64);
        let coordinate_val = Fr::from(25u64);
        let deposit_value = Fr::from(26u64);
        let identity_hash = mimc_hash(private_key, None, &constants);
        let correct_nullifier = mimc_hash(private_key, Some(nonce), &constants);
        let attestation_hash = mimc_hash(private_key, Some(firmware_hash), &constants);
        let ciphertext_hash = mimc_hash(decryption_key, Some(coordinate_val), &constants);
        let deposit_commitment = mimc_hash(identity_hash, Some(deposit_value), &constants);

        let circuit = ZKLoRaCircuit {
            private_key: Some(private_key),
            nonce: Some(nonce),
            decryption_key: Some(decryption_key),
            coordinate_val: Some(coordinate_val),
            firmware_hash_witness: Some(firmware_hash),
            deposit_value: Some(deposit_value),
            identity_hash: Some(identity_hash),
            nullifier_hash: Some(correct_nullifier + Fr::from(1u64)),
            attestation_hash: Some(attestation_hash),
            ciphertext_hash: Some(ciphertext_hash),
            gateway_part1: Some(Fr::from(27u64)),
            gateway_part2: Some(Fr::from(28u64)),
            deposit_commitment: Some(deposit_commitment),
            firmware_hash_public: Some(firmware_hash),
            round_constants: constants,
        };

        let cs = ConstraintSystem::<Fr>::new_ref();
        circuit
            .generate_constraints(cs.clone())
            .expect("constraint generation must succeed");
        assert!(!cs.is_satisfied().expect("satisfaction check must succeed"));
    }
}
