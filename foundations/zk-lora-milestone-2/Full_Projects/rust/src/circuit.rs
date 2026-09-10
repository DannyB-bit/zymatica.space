use ark_bn254::{Bn254, Fr};
use ark_ff::{PrimeField, Field};
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError, Variable};
use ark_snark::{SNARK, CircuitSpecificSetupSNARK};
use ark_std::rand::{Rng, CryptoRng};

/// ZKLoRaCircuit defines the ZK-SNARK circuit for ZK-LoRa.
/// It proves knowledge of a private key (preimage) that hashes to a public identity (hash).
pub struct ZKLoRaCircuit<F: PrimeField> {
    pub private_key: Option<F>,
    pub public_hash: Option<F>,
    pub round_constants: Vec<F>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for ZKLoRaCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // Allocate the private key as a witness (secret input)
        let mut current_var = cs.new_witness_variable(|| {
            self.private_key.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // Track the value of the variable during witness generation
        let mut current_val = self.private_key;

        // Perform MiMC rounds: next = (current + c)^3
        for &c in &self.round_constants {
            // 1. Calculate the values for the witness
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

            // 2. Allocate the variables in the constraint system
            let sq_var = cs.new_witness_variable(|| {
                sq_val.ok_or(SynthesisError::AssignmentMissing)
            })?;

            let next_var = cs.new_witness_variable(|| {
                next_val.ok_or(SynthesisError::AssignmentMissing)
            })?;

            // 3. Add the constraints
            // Constraint 1: (current_var + c) * (current_var + c) = sq_var
            let mut lc1 = ark_relations::r1cs::LinearCombination::from(current_var);
            lc1 += (c, Variable::One);
            
            cs.enforce_constraint(
                lc1.clone(),
                lc1.clone(),
                sq_var.into()
            )?;

            // Constraint 2: sq_var * (current_var + c) = next_var
            cs.enforce_constraint(
                sq_var.into(),
                lc1,
                next_var.into()
            )?;

            // Move to the next round
            current_var = next_var;
            current_val = next_val;
        }

        // Allocate the public input (hash)
        let public_hash_var = cs.new_input_variable(|| {
            self.public_hash.ok_or(SynthesisError::AssignmentMissing)
        })?;

        // Enforce that the output of the MiMC rounds equals the public hash
        cs.enforce_constraint(
            current_var.into(),
            Variable::One.into(),
            public_hash_var.into()
        )?;

        Ok(())
    }
}

/// Generates deterministic round constants for the MiMC hash function.
pub fn generate_mimc_constants(num_rounds: usize) -> Vec<Fr> {
    let mut constants = Vec::with_capacity(num_rounds);
    for i in 0..num_rounds {
        // Generate constants deterministically using a simple formula
        constants.push(Fr::from((i + 123456789) as u64));
    }
    constants
}

/// Computes the MiMC hash of a preimage in the BN254 scalar field.
pub fn mimc_hash(preimage: Fr, constants: &[Fr]) -> Fr {
    let mut current = preimage;
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
        public_hash: None,
        round_constants: constants,
    };
    
    Groth16::<Bn254>::setup(empty_circuit, rng)
}

/// Generates a Groth16 proof for the ZK-LoRa circuit.
pub fn generate_proof<R: Rng + CryptoRng>(
    pk: &ProvingKey<Bn254>,
    private_key: Fr,
    public_hash: Fr,
    constants: Vec<Fr>,
    rng: &mut R,
) -> Result<Proof<Bn254>, SynthesisError> {
    let circuit = ZKLoRaCircuit {
        private_key: Some(private_key),
        public_hash: Some(public_hash),
        round_constants: constants,
    };
    
    Groth16::<Bn254>::prove(pk, circuit, rng)
}

/// Verifies a Groth16 proof against a public hash.
pub fn verify_proof(
    pvk: &PreparedVerifyingKey<Bn254>,
    proof: &Proof<Bn254>,
    public_hash: Fr,
) -> Result<bool, SynthesisError> {
    let public_inputs = vec![public_hash];
    Groth16::<Bn254>::verify_with_processed_vk(pvk, &public_inputs, proof)
}
