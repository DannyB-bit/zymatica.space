pub mod circuit;

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_ff::PrimeField;
use std::sync::Mutex;

// Global state for our custom PRNG (Xorshift128)
static PRNG_STATE: Mutex<[u32; 4]> = Mutex::new([123456789, 362436069, 521288629, 88675123]);

fn xorshift128(state: &mut [u32; 4]) -> u32 {
    let mut t = state[3];
    let s = state[0];
    state[3] = state[2];
    state[2] = state[1];
    state[1] = s;
    t ^= t << 11;
    t ^= t >> 8;
    state[0] = t ^ s ^ (s >> 19);
    state[0]
}

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    if let Ok(mut state) = PRNG_STATE.lock() {
        for chunk in buf.chunks_mut(4) {
            let rand_val = xorshift128(&mut *state);
            let bytes = rand_val.to_le_bytes();
            for (i, b) in chunk.iter_mut().enumerate() {
                *b = bytes[i];
            }
        }
    }
    Ok(())
}

// Register the custom getrandom function
getrandom::register_custom_getrandom!(custom_getrandom);

/// Seeds the custom PRNG with cryptographically secure random bytes from JS/Node.
#[no_mangle]
pub extern "C" fn seed_prng(seed_ptr: *const u8, seed_len: usize) {
    let seed_bytes = unsafe { std::slice::from_raw_parts(seed_ptr, seed_len) };
    let mut state = [0u32; 4];
    for i in 0..4 {
        let start = (i * 4) % seed_bytes.len();
        let mut val = 0u32;
        for j in 0..4 {
            val = (val << 8) | seed_bytes[(start + j) % seed_bytes.len()] as u32;
        }
        state[i] = val;
    }
    if let Ok(mut global_state) = PRNG_STATE.lock() {
        *global_state = state;
    }
}

/// Allocates memory on the WASM heap for passing strings/bytes from JS.
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Deallocates memory on the WASM heap.
#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

/// WebAssembly-compatible proof generation function.
/// Takes private key and public hash hex strings, generates the Groth16 proof,
/// and writes the serialized proof points (A, B, C, and proof_hash) as a JSON string to `out_json_ptr`.
#[no_mangle]
pub extern "C" fn generate_zk_proof_wasm(
    private_key_hex_ptr: *const u8, private_key_hex_len: usize,
    public_hash_hex_ptr: *const u8, public_hash_hex_len: usize,
    out_json_ptr: *mut u8,
    out_json_max_len: usize,
) -> usize {
    let priv_bytes_slice = unsafe { std::slice::from_raw_parts(private_key_hex_ptr, private_key_hex_len) };
    let pub_bytes_slice = unsafe { std::slice::from_raw_parts(public_hash_hex_ptr, public_hash_hex_len) };

    let priv_str = match std::str::from_utf8(priv_bytes_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let pub_str = match std::str::from_utf8(pub_bytes_slice) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let priv_bytes = match hex::decode(priv_str) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    let pub_bytes = match hex::decode(pub_str) {
        Ok(b) => b,
        Err(_) => return 0,
    };

    let private_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&priv_bytes);
    let public_key_fr = ark_bn254::Fr::from_le_bytes_mod_order(&pub_bytes);

    // Use a deterministic seedable RNG for the trusted setup
    use rand::SeedableRng;
    let mut setup_rng = rand::rngs::StdRng::seed_from_u64(12345678);
    let constants = circuit::generate_mimc_constants(10);
    let (pk, _vk) = match circuit::setup_keys(&mut setup_rng, constants.clone()) {
        Ok(k) => k,
        Err(_) => return 0,
    };

    let mut proof_rng = rand::thread_rng();
    let proof = match circuit::generate_proof(&pk, private_key_fr, public_key_fr, constants, &mut proof_rng) {
        Ok(p) => p,
        Err(_) => return 0,
    };

    let mut proof_a_bytes = Vec::new();
    let mut proof_b_bytes = Vec::new();
    let mut proof_c_bytes = Vec::new();
    if proof.a.serialize_compressed(&mut proof_a_bytes).is_err() { return 0; }
    if proof.b.serialize_compressed(&mut proof_b_bytes).is_err() { return 0; }
    if proof.c.serialize_compressed(&mut proof_c_bytes).is_err() { return 0; }

    let proof_a_hex = format!("0x{}", hex::encode(proof_a_bytes));
    let proof_b_hex = format!("0x{}", hex::encode(proof_b_bytes));
    let proof_c_hex = format!("0x{}", hex::encode(proof_c_bytes));

    let proof_bytes = format!("{}{}{}", proof_a_hex, proof_b_hex, proof_c_hex);
    
    // Simple DJB2 hash for the proof_hash to match main.rs
    let mut hash: u64 = 5381;
    for c in proof_bytes.chars() {
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(c as u64);
    }
    let proof_hash = format!("{:016x}{:016x}", hash, hash.wrapping_mul(31))[..32].to_string();

    let json_str = format!(
        r#"{{"proof_a":"{}","proof_b":"{}","proof_c":"{}","proof_hash":"{}"}}"#,
        proof_a_hex, proof_b_hex, proof_c_hex, proof_hash
    );

    let json_bytes = json_str.as_bytes();
    if json_bytes.len() > out_json_max_len {
        return 0;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(json_bytes.as_ptr(), out_json_ptr, json_bytes.len());
    }

    json_bytes.len()
}

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
    use rand::SeedableRng;
    let mut setup_rng = rand::rngs::StdRng::seed_from_u64(12345678);
    let constants = circuit::generate_mimc_constants(10);
    let (_pk, vk) = match circuit::setup_keys(&mut setup_rng, constants.clone()) {
        Ok(k) => k,
        Err(_) => return 0,
    };
    let prepared_vk = ark_groth16::prepare_verifying_key(&vk);

    match circuit::verify_proof(&prepared_vk, &ark_proof, public_key_fr) {
        Ok(true) => 1,
        _ => 0,
    }
}
