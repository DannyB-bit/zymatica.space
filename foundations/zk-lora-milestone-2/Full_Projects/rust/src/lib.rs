pub mod circuit;

use ark_serialize::CanonicalDeserialize;
use ark_ff::PrimeField;

/// WebAssembly-compatible verification function.
/// Takes pointers to UTF-8 hex strings for proof points and the public hash,
/// decodes them, and performs the real Groth16 verification on the BN254 curve.
#[no_mangle]
pub extern "C" fn verify_zk_proof_raw(
    proof_a_hex_ptr: *const u8, proof_a_hex_len: usize,
    proof_b_hex_ptr: *const u8, proof_b_hex_len: usize,
    proof_c_hex_ptr: *const u8, proof_c_hex_len: usize,
    public_hash_hex_ptr: *const u8, public_hash_hex_len: usize,
) -> u32 {
    let proof_a_bytes_slice = unsafe { std::slice::from_raw_parts(proof_a_hex_ptr, proof_a_hex_len) };
    let proof_b_bytes_slice = unsafe { std::slice::from_raw_parts(proof_b_hex_ptr, proof_b_hex_len) };
    let proof_c_bytes_slice = unsafe { std::slice::from_raw_parts(proof_c_hex_ptr, proof_c_hex_len) };
    let public_hash_bytes_slice = unsafe { std::slice::from_raw_parts(public_hash_hex_ptr, public_hash_hex_len) };

    let proof_a_str = match std::str::from_utf8(proof_a_bytes_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let proof_b_str = match std::str::from_utf8(proof_b_bytes_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let proof_c_str = match std::str::from_utf8(proof_c_bytes_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let public_hash_str = match std::str::from_utf8(public_hash_bytes_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    // Strip "0x" prefix if present
    let a_hex = if proof_a_str.starts_with("0x") { &proof_a_str[2..] } else { proof_a_str };
    let b_hex = if proof_b_str.starts_with("0x") { &proof_b_str[2..] } else { proof_b_str };
    let c_hex = if proof_c_str.starts_with("0x") { &proof_c_str[2..] } else { proof_c_str };

    let proof_a_bytes = match hex::decode(a_hex) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let proof_b_bytes = match hex::decode(b_hex) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let proof_c_bytes = match hex::decode(c_hex) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let pub_bytes = match hex::decode(public_hash_str) {
        Ok(b) => b,
        Err(_) => return 0,
    };

    let proof_a = match ark_bn254::G1Affine::deserialize_compressed(&*proof_a_bytes) {
        Ok(p) => p,
        Err(_) => return 0,
    };
    let proof_b = match ark_bn254::G2Affine::deserialize_compressed(&*proof_b_bytes) {
        Ok(p) => p,
        Err(_) => return 0,
    };
    let proof_c = match ark_bn254::G1Affine::deserialize_compressed(&*proof_c_bytes) {
        Ok(p) => p,
        Err(_) => return 0,
    };

    let ark_proof = ark_groth16::Proof {
        a: proof_a,
        b: proof_b,
        c: proof_c,
    };

    let public_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&pub_bytes);

    // Setup keys deterministically (same as ZKProver::new())
    let mut rng = rand::thread_rng();
    let constants = circuit::generate_mimc_constants(10);
    let (_pk, vk) = match circuit::setup_keys(&mut rng, constants.clone()) {
        Ok(k) => k,
        Err(_) => return 0,
    };
    let prepared_vk = ark_groth16::prepare_verifying_key(&vk);

    match circuit::verify_proof(&prepared_vk, &ark_proof, public_key_fr) {
        Ok(true) => 1,
        _ => 0,
    }
}
