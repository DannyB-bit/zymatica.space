use ark_bn254::Fr;
use ark_groth16::prepare_verifying_key;
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystem};
use rand::{rngs::StdRng, SeedableRng};
use zk_lorawan_groth16::circuit::{
    generate_mimc_constants, generate_proof, mimc_hash, setup_keys, verify_proof, ZKLoRaCircuit,
};

const MIMC_ROUNDS: usize = 220;
// 8 public inputs + 1 implicit "one" variable = 9
const EXPECTED_PUBLIC_INPUTS_WITH_ONE: usize = 9;
// 6 hash constraint sections (identity, nullifier, attestation, ciphertext, gateway binding, deposit)
// + firmware_hash equality = 2652 constraints
const EXPECTED_CONSTRAINTS: usize = 2652;

#[derive(Clone)]
struct CircuitFixture {
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
}

fn fixture() -> CircuitFixture {
    let constants = generate_mimc_constants(MIMC_ROUNDS);
    let private_key = Fr::from(1_337u64);
    let nonce = Fr::from(9_999u64);
    let decryption_key = Fr::from(7_331u64);
    let coordinate_val = Fr::from(42u64);
    let firmware_hash = Fr::from(2_026u64);
    let deposit_value = Fr::from(100_000u64);
    let gateway_part1 = Fr::from(1_111u64);
    let gateway_part2 = Fr::from(2_222u64);

    let identity_hash = mimc_hash(private_key, None, &constants);
    let deposit_commitment = mimc_hash(identity_hash, Some(deposit_value), &constants);

    CircuitFixture {
        identity_hash,
        nullifier_hash: mimc_hash(private_key, Some(nonce), &constants),
        attestation_hash: mimc_hash(private_key, Some(firmware_hash), &constants),
        ciphertext_hash: mimc_hash(decryption_key, Some(coordinate_val), &constants),
        deposit_commitment,
        firmware_hash_public: firmware_hash,
        private_key,
        nonce,
        decryption_key,
        coordinate_val,
        firmware_hash,
        deposit_value,
        gateway_part1,
        gateway_part2,
        constants,
    }
}

fn circuit_from(f: &CircuitFixture) -> ZKLoRaCircuit<Fr> {
    ZKLoRaCircuit {
        private_key: Some(f.private_key),
        nonce: Some(f.nonce),
        decryption_key: Some(f.decryption_key),
        coordinate_val: Some(f.coordinate_val),
        firmware_hash_witness: Some(f.firmware_hash),
        deposit_value: Some(f.deposit_value),
        identity_hash: Some(f.identity_hash),
        nullifier_hash: Some(f.nullifier_hash),
        attestation_hash: Some(f.attestation_hash),
        ciphertext_hash: Some(f.ciphertext_hash),
        gateway_part1: Some(f.gateway_part1),
        gateway_part2: Some(f.gateway_part2),
        deposit_commitment: Some(f.deposit_commitment),
        firmware_hash_public: Some(f.firmware_hash_public),
        round_constants: f.constants.clone(),
    }
}

fn assert_unsatisfied_after(mut mutate: impl FnMut(&mut CircuitFixture)) {
    let mut f = fixture();
    mutate(&mut f);

    let cs = ConstraintSystem::<Fr>::new_ref();
    circuit_from(&f).generate_constraints(cs.clone()).unwrap();

    assert!(
        !cs.is_satisfied().unwrap(),
        "mutated public input or witness unexpectedly satisfied the circuit"
    );
}

#[test]
fn valid_assignment_has_expected_public_inputs_and_constraints() {
    let f = fixture();
    let cs = ConstraintSystem::<Fr>::new_ref();

    circuit_from(&f).generate_constraints(cs.clone()).unwrap();

    assert!(cs.is_satisfied().unwrap());
    assert_eq!(
        cs.num_instance_variables(),
        EXPECTED_PUBLIC_INPUTS_WITH_ONE,
        "R1CS should expose exactly eight public inputs plus the implicit one variable"
    );
    assert_eq!(
        cs.num_constraints(),
        EXPECTED_CONSTRAINTS,
        "constraint count changed; review for accidental under-constraint or circuit drift"
    );
    assert!(
        cs.num_witness_variables() > cs.num_instance_variables(),
        "hash rounds should allocate private witness intermediates"
    );
}

#[test]
fn each_public_input_is_bound_by_constraints() {
    assert_unsatisfied_after(|f| f.identity_hash += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.nullifier_hash += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.attestation_hash += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.ciphertext_hash += Fr::from(1u64));
    // deposit_commitment is constrained by MiMC(identity_hash + deposit_value)
    assert_unsatisfied_after(|f| f.deposit_commitment += Fr::from(1u64));
    // firmware_hash_public must equal firmware_hash witness
    assert_unsatisfied_after(|f| f.firmware_hash_public += Fr::from(1u64));
}

#[test]
fn witness_mutations_are_detected_by_public_bindings() {
    assert_unsatisfied_after(|f| f.private_key += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.nonce += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.decryption_key += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.coordinate_val += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.firmware_hash += Fr::from(1u64));
    assert_unsatisfied_after(|f| f.deposit_value += Fr::from(1u64));
}

#[test]
fn groth16_verifier_rejects_mutated_public_inputs() {
    let f = fixture();
    let mut rng = StdRng::from_seed([7u8; 32]);
    let (pk, vk) = setup_keys(&mut rng, f.constants.clone()).unwrap();
    let pvk = prepare_verifying_key(&vk);

    let proof = generate_proof(
        &pk,
        f.private_key,
        f.nonce,
        f.decryption_key,
        f.coordinate_val,
        f.firmware_hash,
        f.deposit_value,
        f.identity_hash,
        f.nullifier_hash,
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1,
        f.gateway_part2,
        f.deposit_commitment,
        f.firmware_hash_public,
        f.constants.clone(),
        &mut rng,
    )
    .unwrap();

    // Valid proof should verify
    assert!(verify_proof(
        &pvk,
        &proof,
        f.identity_hash,
        f.nullifier_hash,
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1,
        f.gateway_part2,
        f.deposit_commitment,
        f.firmware_hash_public,
    )
    .unwrap());

    // Mutated identity_hash should fail
    assert!(!verify_proof(
        &pvk,
        &proof,
        f.identity_hash + Fr::from(1u64),
        f.nullifier_hash,
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1,
        f.gateway_part2,
        f.deposit_commitment,
        f.firmware_hash_public,
    )
    .unwrap());

    // Mutated nullifier_hash should fail
    assert!(!verify_proof(
        &pvk,
        &proof,
        f.identity_hash,
        f.nullifier_hash + Fr::from(1u64),
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1,
        f.gateway_part2,
        f.deposit_commitment,
        f.firmware_hash_public,
    )
    .unwrap());

    // Mutated deposit_commitment should fail
    assert!(!verify_proof(
        &pvk,
        &proof,
        f.identity_hash,
        f.nullifier_hash,
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1,
        f.gateway_part2,
        f.deposit_commitment + Fr::from(1u64),
        f.firmware_hash_public,
    )
    .unwrap());

    // Mutated firmware_hash_public should fail
    assert!(!verify_proof(
        &pvk,
        &proof,
        f.identity_hash,
        f.nullifier_hash,
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1,
        f.gateway_part2,
        f.deposit_commitment,
        f.firmware_hash_public + Fr::from(1u64),
    )
    .unwrap());

    // Mutated gateway should fail
    assert!(!verify_proof(
        &pvk,
        &proof,
        f.identity_hash,
        f.nullifier_hash,
        f.attestation_hash,
        f.ciphertext_hash,
        f.gateway_part1 + Fr::from(1u64),
        f.gateway_part2,
        f.deposit_commitment,
        f.firmware_hash_public,
    )
    .unwrap());
}

#[test]
fn unsatisfied_assignment_cannot_verify_against_its_bad_public_inputs() {
    let mut f = fixture();
    let mut rng = StdRng::from_seed([8u8; 32]);
    let (pk, vk) = setup_keys(&mut rng, f.constants.clone()).unwrap();
    let pvk = prepare_verifying_key(&vk);

    f.identity_hash += Fr::from(1u64);
    let prove_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generate_proof(
            &pk,
            f.private_key,
            f.nonce,
            f.decryption_key,
            f.coordinate_val,
            f.firmware_hash,
            f.deposit_value,
            f.identity_hash,
            f.nullifier_hash,
            f.attestation_hash,
            f.ciphertext_hash,
            f.gateway_part1,
            f.gateway_part2,
            f.deposit_commitment,
            f.firmware_hash_public,
            f.constants.clone(),
            &mut rng,
        )
    }));

    match prove_result {
        Err(_) | Ok(Err(_)) => {}
        Ok(Ok(proof)) => {
            assert!(
                !verify_proof(
                    &pvk,
                    &proof,
                    f.identity_hash,
                    f.nullifier_hash,
                    f.attestation_hash,
                    f.ciphertext_hash,
                    f.gateway_part1,
                    f.gateway_part2,
                    f.deposit_commitment,
                    f.firmware_hash_public,
                )
                .unwrap(),
                "proof generated from an unsatisfied assignment must not verify"
            );
        }
    }
}
