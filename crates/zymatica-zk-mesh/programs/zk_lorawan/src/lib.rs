// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.
//
// ZK-LoRaWAN Solana Anchor Program — Dual Mode (Single + Batch)
// Upgraded to Advanced Shielded Pool (ZK-Compression), Micro-TEE Attestation & zk-VDE
// ==============================================================================

use anchor_lang::prelude::*;
use anchor_lang::solana_program::{alt_bn128::prelude::*, keccak, pubkey, system_instruction};

#[cfg(feature = "integration-test")]
declare_id!("7wDzutwwr37nfxeMRydy5UEyREKho3Vjm8SxJgR4fzFy");
#[cfg(not(feature = "integration-test"))]
declare_id!("4HRP2eV8qtYW54ozQmnGDjF7emwb8MvqFcF89UgSM6iC");

// ============================================================================
// Protocol Constants
// ============================================================================
pub const PROTOCOL_FEE_LAMPORTS: u64 = 50_000; // treasury per chirp
pub const GATEWAY_REWARD_LAMPORTS: u64 = 100_000; // gateway per chirp
pub const TOTAL_FEE_PER_CHIRP: u64 = PROTOCOL_FEE_LAMPORTS + GATEWAY_REWARD_LAMPORTS;
pub const MAX_BATCH_SIZE: usize = 100;
pub const MERKLE_DEPTH: usize = 10; // 2^16 = 65,536 leaves; fits in Solana 1232-byte tx thanks to chunked flow
pub const BATCH_SEED: &[u8] = b"zk-lorawan-batch";
pub const REGISTRY_SEED: &[u8] = b"zk-lorawan-registry";
#[cfg(feature = "integration-test")]
pub const ADMIN_AUTHORITY: Pubkey = pubkey!("FXdqcsZZnirF1sLYm4huXZeFTLwsqJ3ueNZJdPWUMkRc");
#[cfg(not(feature = "integration-test"))]
pub const ADMIN_AUTHORITY: Pubkey = pubkey!("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS");

#[program]
pub mod zk_lorawan {
    use super::*;

    // ========================================================================
    // deposit_shielded — fund global shielded pool
    // ========================================================================
    pub fn deposit_shielded(
        ctx: Context<DepositShielded>,
        amount: u64,
        leaf_hash: [u8; 32],
    ) -> Result<()> {
        require!(
            amount == TOTAL_FEE_PER_CHIRP,
            ZKLoRaError::InvalidDepositAmount
        );

        anchor_lang::solana_program::program::invoke(
            &system_instruction::transfer(
                &ctx.accounts.sender.key(),
                &ctx.accounts.shielded_pool.key(),
                amount,
            ),
            &[
                ctx.accounts.sender.to_account_info(),
                ctx.accounts.shielded_pool.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        let shielded_pool = &mut ctx.accounts.shielded_pool;

        let next_index = shielded_pool.next_index;
        // Enforce max depth index limit (2^MERKLE_DEPTH = 1,024 leaves)
        require!(
            next_index < (1u64 << MERKLE_DEPTH as u64),
            ZKLoRaError::CalculationOverflow
        );

        let new_root =
            insert_leaf_on_chain(&mut shielded_pool.filled_subtrees, next_index, leaf_hash);

        shielded_pool.total_balance = shielded_pool.to_account_info().lamports();
        shielded_pool.merkle_root = new_root;
        shielded_pool.next_index = next_index
            .checked_add(1)
            .ok_or(ZKLoRaError::CalculationOverflow)?;
        shielded_pool.last_updated = Clock::get()?.unix_timestamp;
        shielded_pool.bump = ctx.bumps.shielded_pool;

        // Maintain the 8-root history ring buffer
        let hist_idx = shielded_pool.history_index as usize;
        shielded_pool.roots_history[hist_idx] = new_root;
        shielded_pool.history_index = (shielded_pool.history_index + 1) % 8;

        msg!(
            "Deposited {} lamports into Shielded Pool (index {}), updated Merkle Root to {:?}",
            amount,
            next_index,
            new_root
        );
        Ok(())
    }

    // ========================================================================
    // MODE A — Single Chirp Verification (Shielded, Attested, zk-VDE)
    // ========================================================================
    pub fn verify_single(
        ctx: Context<VerifySingle>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        nullifier_hash: [u8; 32],
        attestation_hash: [u8; 32],
        zk_vde_proof_hash: [u8; 32],
        ciphertext_hash: [u8; 32],
        deposit_commitment: [u8; 32],
        firmware_hash: [u8; 32],
        _timestamp: i64,
        merkle_proof: Vec<[u8; 32]>,
        leaf_index: u32,
    ) -> Result<()> {
        let shielded_pool_info = ctx.accounts.shielded_pool.to_account_info();
        let gateway_info = ctx.accounts.gateway.to_account_info();
        let treasury_info = ctx.accounts.treasury.to_account_info();

        // 0. Verify Merkle membership proof in the shielded pool (allowing historical roots)
        require!(
            merkle_proof.len() == MERKLE_DEPTH,
            ZKLoRaError::InvalidMerkleProof
        );
        require!(
            (leaf_index as u64) < ctx.accounts.shielded_pool.next_index,
            ZKLoRaError::InvalidMerkleProof
        );

        let computed_root =
            compute_merkle_root_from_proof(&zk_vde_proof_hash, &merkle_proof, leaf_index);
        let is_member = is_known_root(&ctx.accounts.shielded_pool, &computed_root);
        require!(is_member, ZKLoRaError::InvalidMerkleProof);

        // Verify firmware is whitelisted
        require!(
            verify_firmware_hash(&ctx.accounts.registry, &firmware_hash),
            ZKLoRaError::InvalidAttestation
        );

        // 1. Verify ZK Groth16 proof using alt_bn128 pairing check (with gateway address binding)
        let gateway_key = ctx.accounts.gateway.key().to_bytes();
        let mut gateway_part1 = [0u8; 32];
        gateway_part1[0..16].copy_from_slice(&gateway_key[0..16]);
        let mut gateway_part2 = [0u8; 32];
        gateway_part2[0..16].copy_from_slice(&gateway_key[16..32]);

        let public_inputs = [
            zk_vde_proof_hash,
            nullifier_hash,
            attestation_hash,
            ciphertext_hash,
            gateway_part1,
            gateway_part2,
            deposit_commitment,
            firmware_hash,
        ];
        verify_groth16(&proof_a, &proof_b, &proof_c, &public_inputs)?;

        // 2. Double-spend prevention via nullifier record initialization
        let nullifier_record = &mut ctx.accounts.nullifier_record;
        nullifier_record.nullifier_hash = nullifier_hash;
        nullifier_record.spent_at = Clock::get()?.unix_timestamp;

        // Check balance of the global shielded pool
        let rent_minimum = Rent::get()?.minimum_balance(shielded_pool_info.data_len());
        let available_balance = shielded_pool_info.lamports().saturating_sub(rent_minimum);
        require!(
            available_balance >= TOTAL_FEE_PER_CHIRP,
            ZKLoRaError::InsufficientEscrowFunding
        );

        // Perform transfers (debit ShieldedEscrowPool directly)
        **shielded_pool_info.try_borrow_mut_lamports()? = shielded_pool_info
            .lamports()
            .checked_sub(TOTAL_FEE_PER_CHIRP)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        **gateway_info.try_borrow_mut_lamports()? = gateway_info
            .lamports()
            .checked_add(GATEWAY_REWARD_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        **treasury_info.try_borrow_mut_lamports()? = treasury_info
            .lamports()
            .checked_add(PROTOCOL_FEE_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        // Update registry
        let registry = &mut ctx.accounts.registry;
        registry.total_chirps_verified += 1;
        registry.total_fees_collected += TOTAL_FEE_PER_CHIRP;
        registry.total_gateway_rewards += GATEWAY_REWARD_LAMPORTS;
        registry.total_treasury_fees += PROTOCOL_FEE_LAMPORTS;

        msg!("ZK-LoRaWAN [SINGLE] Shielded chirp verified.");
        msg!("  Nullifier:     {:?} spent", &nullifier_hash[..4]);
        msg!("  Attestation:   {:?} verified", &attestation_hash[..4]);
        msg!("  Gateway:       +{} lamports", GATEWAY_REWARD_LAMPORTS);
        msg!("  Treasury:      +{} lamports", PROTOCOL_FEE_LAMPORTS);
        Ok(())
    }

    pub fn verify_single_proof(
        ctx: Context<VerifySingleProof>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        public_inputs: [[u8; 32]; 8],
        merkle_proof: Vec<[u8; 32]>,
        leaf_index: u32,
    ) -> Result<()> {
        let pool_account_info = ctx.accounts.shielded_pool.to_account_info();
        let pool_data = pool_account_info.try_borrow_data()?;
        let mut pool_ptr = &pool_data[8..];
        let shielded_pool: Box<ShieldedEscrowPool> =
            Box::new(ShieldedEscrowPool::deserialize(&mut pool_ptr)?);
        let shielded_pool_info = ctx.accounts.shielded_pool.to_account_info();
        let gateway_info = ctx.accounts.gateway.to_account_info();
        let treasury_info = ctx.accounts.treasury.to_account_info();

        // 0. Verify Merkle membership proof in the shielded pool (allowing historical roots)
        require!(
            merkle_proof.len() == MERKLE_DEPTH,
            ZKLoRaError::InvalidMerkleProof
        );
        require!(
            (leaf_index as u64) < shielded_pool.next_index,
            ZKLoRaError::InvalidMerkleProof
        );

        let computed_root =
            compute_merkle_root_from_proof(&public_inputs[0], &merkle_proof, leaf_index);
        let is_member = is_known_root(&shielded_pool, &computed_root);
        require!(is_member, ZKLoRaError::InvalidMerkleProof);

        // Verify firmware is whitelisted
        require!(
            verify_firmware_hash(&ctx.accounts.registry, &public_inputs[7]),
            ZKLoRaError::InvalidAttestation
        );

        // Verify gateway binding: public_inputs[4] and [5] must encode gateway's public key
        let gateway_key = ctx.accounts.gateway.key().to_bytes();
        let mut gateway_part1 = [0u8; 32];
        gateway_part1[0..16].copy_from_slice(&gateway_key[0..16]);
        let mut gateway_part2 = [0u8; 32];
        gateway_part2[0..16].copy_from_slice(&gateway_key[16..32]);

        require!(
            public_inputs[4] == gateway_part1,
            ZKLoRaError::UnauthorizedGateway
        );
        require!(
            public_inputs[5] == gateway_part2,
            ZKLoRaError::UnauthorizedGateway
        );

        // 1. Verify ZK Groth16 proof using alt_bn128 pairing check
        verify_groth16(&proof_a, &proof_b, &proof_c, &public_inputs)?;

        // 2. Double-spend prevention via nullifier record initialization
        let nullifier_record = &mut ctx.accounts.nullifier_record;
        nullifier_record.nullifier_hash = public_inputs[1];
        nullifier_record.spent_at = Clock::get()?.unix_timestamp;

        // Check balance of the global shielded pool
        let rent_minimum = Rent::get()?.minimum_balance(shielded_pool_info.data_len());
        let available_balance = shielded_pool_info.lamports().saturating_sub(rent_minimum);
        require!(
            available_balance >= TOTAL_FEE_PER_CHIRP,
            ZKLoRaError::InsufficientEscrowFunding
        );

        // Perform transfers (debit ShieldedEscrowPool directly)
        **shielded_pool_info.try_borrow_mut_lamports()? = shielded_pool_info
            .lamports()
            .checked_sub(TOTAL_FEE_PER_CHIRP)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        **gateway_info.try_borrow_mut_lamports()? = gateway_info
            .lamports()
            .checked_add(GATEWAY_REWARD_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        **treasury_info.try_borrow_mut_lamports()? = treasury_info
            .lamports()
            .checked_add(PROTOCOL_FEE_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        // Update registry
        let registry = &mut ctx.accounts.registry;
        registry.total_chirps_verified += 1;
        registry.total_fees_collected += TOTAL_FEE_PER_CHIRP;
        registry.total_gateway_rewards += GATEWAY_REWARD_LAMPORTS;
        registry.total_treasury_fees += PROTOCOL_FEE_LAMPORTS;

        msg!("ZK-LoRaWAN [SINGLE PROOF] Cryptographically verified.");
        msg!("  Nullifier:     {:?} spent", &public_inputs[1][..4]);
        msg!("  Attestation:   {:?} verified", &public_inputs[2][..4]);
        msg!("  Gateway:       +{} lamports", GATEWAY_REWARD_LAMPORTS);
        msg!("  Treasury:      +{} lamports", PROTOCOL_FEE_LAMPORTS);
        Ok(())
    }

    // ========================================================================
    // MODE B — Batch Verification
    // ========================================================================

    // B.1 — Initialize a new batch
    pub fn initialize_batch(ctx: Context<InitializeBatch>) -> Result<()> {
        let batch = &mut ctx.accounts.batch;
        batch.gateway = ctx.accounts.gateway.key();
        batch.chirp_count = 0;
        batch.nullifiers = Vec::new();
        batch.merkle_root = [0u8; 32];
        batch.is_finalized = false;
        batch.created_at = Clock::get()?.unix_timestamp;
        batch.finalized_at = 0;
        batch.batch_id = ctx.accounts.registry.next_batch_id;

        let registry = &mut ctx.accounts.registry;
        registry.next_batch_id += 1;
        registry.total_batches += 1;

        msg!(
            "ZK-LoRaWAN [BATCH] #{} initialized for gateway {}",
            batch.batch_id,
            batch.gateway
        );
        Ok(())
    }

    // B.2 — Add chirps to the batch
    pub fn add_chirp(
        ctx: Context<AddChirp>,
        proof_a: [u8; 64],
        proof_b: [u8; 128],
        proof_c: [u8; 64],
        nullifier_hash: [u8; 32],
        attestation_hash: [u8; 32],
        zk_vde_proof_hash: [u8; 32],
        payload_hash: [u8; 32],
        deposit_commitment: [u8; 32],
        firmware_hash: [u8; 32],
        _timestamp: i64,
        merkle_proof: Vec<[u8; 32]>,
        leaf_index: u32,
    ) -> Result<()> {
        let batch = &mut ctx.accounts.batch;

        require!(!batch.is_finalized, ZKLoRaError::BatchAlreadyFinalized);
        require!(
            batch.chirp_count < MAX_BATCH_SIZE as u32,
            ZKLoRaError::BatchFull
        );
        require!(
            batch.gateway == ctx.accounts.gateway.key(),
            ZKLoRaError::UnauthorizedGateway
        );

        // 0. Verify Merkle membership proof in the shielded pool (allowing historical roots)
        require!(
            merkle_proof.len() == MERKLE_DEPTH,
            ZKLoRaError::InvalidMerkleProof
        );
        require!(
            (leaf_index as u64) < ctx.accounts.shielded_pool.next_index,
            ZKLoRaError::InvalidMerkleProof
        );

        let computed_root =
            compute_merkle_root_from_proof(&zk_vde_proof_hash, &merkle_proof, leaf_index);
        let is_member = is_known_root(&ctx.accounts.shielded_pool, &computed_root);
        require!(is_member, ZKLoRaError::InvalidMerkleProof);

        // Verify firmware is whitelisted
        require!(
            verify_firmware_hash(&ctx.accounts.registry, &firmware_hash),
            ZKLoRaError::InvalidAttestation
        );

        // 1. Verify Groth16 proof per chirp inside the batch on-chain (with gateway address binding)
        let gateway_key = ctx.accounts.gateway.key().to_bytes();
        let mut gateway_part1 = [0u8; 32];
        gateway_part1[0..16].copy_from_slice(&gateway_key[0..16]);
        let mut gateway_part2 = [0u8; 32];
        gateway_part2[0..16].copy_from_slice(&gateway_key[16..32]);

        let public_inputs = [
            zk_vde_proof_hash,
            nullifier_hash,
            attestation_hash,
            payload_hash,
            gateway_part1,
            gateway_part2,
            deposit_commitment,
            firmware_hash,
        ];
        verify_groth16(&proof_a, &proof_b, &proof_c, &public_inputs)?;

        batch.nullifiers.push(nullifier_hash);
        batch.chirp_count += 1;

        msg!(
            "ZK-LoRaWAN [BATCH] #{}: chirp {} added with nullifier {:?}",
            batch.batch_id,
            batch.chirp_count,
            &nullifier_hash[..4]
        );
        Ok(())
    }

    // B.3 — Submit (finalize) the batch
    pub fn submit_batch<'a, 'b, 'c, 'info>(
        ctx: Context<'a, 'b, 'c, 'info, SubmitBatch<'info>>,
    ) -> Result<()> {
        let batch = &mut ctx.accounts.batch;

        require!(!batch.is_finalized, ZKLoRaError::BatchAlreadyFinalized);
        require!(batch.chirp_count > 0, ZKLoRaError::EmptyBatch);
        require!(
            batch.gateway == ctx.accounts.gateway.key(),
            ZKLoRaError::UnauthorizedGateway
        );

        let n = batch.chirp_count as u64;

        // Total fees
        let total_gateway_reward = GATEWAY_REWARD_LAMPORTS * n;
        let total_treasury_fee = PROTOCOL_FEE_LAMPORTS * n;
        let total_fee = total_gateway_reward + total_treasury_fee;

        // Verify and initialize spent nullifiers from remaining accounts
        for &nullifier in batch.nullifiers.iter() {
            let (expected_nullifier_pda, _bump) =
                Pubkey::find_program_address(&[b"nullifier", nullifier.as_ref()], ctx.program_id);

            let nullifier_account_info = ctx
                .remaining_accounts
                .iter()
                .find(|acc| acc.key() == expected_nullifier_pda)
                .ok_or(error!(ZKLoRaError::NullifierAccountMissing))?;

            // Check if already initialized
            if nullifier_account_info.data_is_empty() {
                let rent = Rent::get()?;
                let space = 8 + 32 + 8;
                let lamports = rent.minimum_balance(space);

                let seeds = &[b"nullifier".as_ref(), nullifier.as_ref(), &[_bump]];
                let signer_seeds = &[&seeds[..]];

                // Create account programmatically
                anchor_lang::solana_program::program::invoke_signed(
                    &system_instruction::create_account(
                        &ctx.accounts.gateway.key(),
                        &expected_nullifier_pda,
                        lamports,
                        space as u64,
                        ctx.program_id,
                    ),
                    &[
                        ctx.accounts.gateway.to_account_info(),
                        nullifier_account_info.clone(),
                        ctx.accounts.system_program.to_account_info(),
                    ],
                    signer_seeds,
                )?;

                // Write state
                let mut data = nullifier_account_info.try_borrow_mut_data()?;
                data[0..8].copy_from_slice(&[56, 18, 57, 175, 69, 202, 189, 70]);
                data[8..40].copy_from_slice(&nullifier);
                data[40..48].copy_from_slice(&Clock::get()?.unix_timestamp.to_le_bytes());
            } else {
                return err!(ZKLoRaError::NullifierAlreadySpent);
            }
        }

        // Check balance of the global shielded pool
        let rent_minimum =
            Rent::get()?.minimum_balance(ctx.accounts.shielded_pool.to_account_info().data_len());
        let available_balance = ctx
            .accounts
            .shielded_pool
            .to_account_info()
            .lamports()
            .saturating_sub(rent_minimum);
        require!(
            available_balance >= total_fee,
            ZKLoRaError::InsufficientEscrowFunding
        );

        // Debit from ShieldedEscrowPool PDA
        **ctx
            .accounts
            .shielded_pool
            .to_account_info()
            .try_borrow_mut_lamports()? = ctx
            .accounts
            .shielded_pool
            .to_account_info()
            .lamports()
            .checked_sub(total_fee)
            .ok_or(error!(ZKLoRaError::CalculationOverflow))?;

        // Credit to gateway
        **ctx.accounts.gateway.try_borrow_mut_lamports()? = ctx
            .accounts
            .gateway
            .lamports()
            .checked_add(total_gateway_reward)
            .ok_or(error!(ZKLoRaError::CalculationOverflow))?;

        // Credit to treasury
        **ctx.accounts.treasury.try_borrow_mut_lamports()? = ctx
            .accounts
            .treasury
            .lamports()
            .checked_add(total_treasury_fee)
            .ok_or(error!(ZKLoRaError::CalculationOverflow))?;

        // Compute Merkle root
        let merkle_root = compute_merkle_root(&batch.nullifiers);
        batch.merkle_root = merkle_root;
        batch.is_finalized = true;
        batch.finalized_at = Clock::get()?.unix_timestamp;

        // Update registry
        let registry = &mut ctx.accounts.registry;
        registry.total_chirps_verified += n;
        registry.total_fees_collected += total_fee;
        registry.total_gateway_rewards += total_gateway_reward;
        registry.total_treasury_fees += total_treasury_fee;

        msg!("ZK-LoRaWAN [BATCH] #{} FINALIZED", batch.batch_id);
        msg!("  Chirps:          {}", n);
        msg!("  Merkle root:     {:?}...", &merkle_root[..8]);
        msg!(
            "  Gateway reward:  {} lamports ({} × {})",
            total_gateway_reward,
            GATEWAY_REWARD_LAMPORTS,
            n
        );
        msg!(
            "  Treasury fee:    {} lamports ({} × {})",
            total_treasury_fee,
            PROTOCOL_FEE_LAMPORTS,
            n
        );
        msg!("  Total charged:   {} lamports", total_fee);
        Ok(())
    }

    // ========================================================================
    // Verify a chirp's inclusion in a finalized batch (public, anyone can call)
    // ========================================================================
    pub fn verify_chirp_inclusion(
        ctx: Context<VerifyChirpInclusion>,
        proof_hash: [u8; 32],
        merkle_proof: Vec<[u8; 32]>,
        leaf_index: u32,
    ) -> Result<()> {
        let batch = &ctx.accounts.batch;

        require!(batch.is_finalized, ZKLoRaError::BatchNotFinalized);

        let is_valid =
            verify_merkle_proof(&proof_hash, &merkle_proof, leaf_index, &batch.merkle_root);

        require!(is_valid, ZKLoRaError::InvalidMerkleProof);

        msg!(
            "ZK-LoRaWAN: Chirp inclusion verified in batch #{} (leaf {})",
            batch.batch_id,
            leaf_index
        );
        Ok(())
    }

    // ========================================================================
    // Initialize protocol registry (one-time admin setup)
    // ========================================================================
    pub fn initialize_registry(ctx: Context<InitializeRegistry>) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        registry.authority = ctx.accounts.authority.key();
        registry.next_batch_id = 0;
        registry.total_batches = 0;
        registry.total_chirps_verified = 0;
        registry.total_fees_collected = 0;
        registry.total_gateway_rewards = 0;
        registry.total_treasury_fees = 0;
        registry.created_at = Clock::get()?.unix_timestamp;

        msg!(
            "ZK-LoRaWAN: Protocol registry initialized by {}",
            registry.authority
        );
        Ok(())
    }

    pub fn register_firmware_hash(ctx: Context<RegisterFirmware>, hash: [u8; 32]) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        require!(
            registry.approved_firmware_hashes.len() < 10,
            ZKLoRaError::CalculationOverflow
        );
        if !registry.approved_firmware_hashes.contains(&hash) {
            registry.approved_firmware_hashes.push(hash);
        }
        msg!("Registered firmware hash: {:?}", hash);
        Ok(())
    }

    // ========================================================================
    // Chunked Proof Flow — split proof data across multiple small transactions
    // ========================================================================

    /// Step 1: Create the ProofContext PDA (empty, ready for chunks)
    pub fn initialize_proof_context(
        ctx: Context<InitializeProofContext>,
        _nonce: u64,
    ) -> Result<()> {
        let pc = &mut ctx.accounts.proof_context;
        pc.gateway = ctx.accounts.gateway.key();
        pc.chunks_written = 0;
        pc.is_complete = false;
        pc.created_at = Clock::get()?.unix_timestamp;
        pc.bump = ctx.bumps.proof_context;
        msg!("ProofContext initialized for gateway {}", pc.gateway);
        Ok(())
    }

    /// Step 2: Write one chunk of proof data into the PDA
    /// chunk_index 0 = proof_a (64) + proof_b (128) = 192 bytes
    /// chunk_index 1 = proof_c (64) + public_inputs (256) = 320 bytes
    /// chunk_index 2 = merkle_proof (MERKLE_DEPTH*32) + leaf_index (4)
    pub fn write_proof_chunk(
        ctx: Context<WriteProofChunk>,
        _nonce: u64,
        chunk_index: u8,
        data: Vec<u8>,
    ) -> Result<()> {
        let pc = &mut ctx.accounts.proof_context;

        // Gateway binding: only the original gateway can write
        require!(
            pc.gateway == ctx.accounts.gateway.key(),
            ZKLoRaError::ProofContextUnauthorized
        );
        require!(chunk_index < 3, ZKLoRaError::InvalidChunkIndex);

        match chunk_index {
            0 => {
                // proof_a (64) + proof_b (128) = 192 bytes
                require!(data.len() == 192, ZKLoRaError::InvalidChunkDataLength);
                pc.proof_a.copy_from_slice(&data[0..64]);
                pc.proof_b.copy_from_slice(&data[64..192]);
            }
            1 => {
                // proof_c (64) + public_inputs (8 * 32 = 256) = 320 bytes
                require!(data.len() == 320, ZKLoRaError::InvalidChunkDataLength);
                pc.proof_c.copy_from_slice(&data[0..64]);
                for i in 0..8 {
                    pc.public_inputs[i].copy_from_slice(&data[64 + i * 32..64 + (i + 1) * 32]);
                }
            }
            2 => {
                // merkle_proof (MERKLE_DEPTH * 32) + leaf_index (4)
                let expected = MERKLE_DEPTH * 32 + 4;
                require!(data.len() == expected, ZKLoRaError::InvalidChunkDataLength);
                for i in 0..MERKLE_DEPTH {
                    pc.merkle_proof[i].copy_from_slice(&data[i * 32..(i + 1) * 32]);
                }
                let leaf_bytes: [u8; 4] = data[MERKLE_DEPTH * 32..MERKLE_DEPTH * 32 + 4]
                    .try_into()
                    .unwrap();
                pc.leaf_index = u32::from_le_bytes(leaf_bytes);
            }
            _ => return Err(ZKLoRaError::InvalidChunkIndex.into()),
        }

        pc.chunks_written |= 1 << chunk_index; // bitmask: bit 0, 1, 2
        if pc.chunks_written == 0b111 {
            pc.is_complete = true;
            msg!("ProofContext complete — all 3 chunks written");
        } else {
            msg!(
                "ProofContext chunk {} written (mask: {:03b})",
                chunk_index,
                pc.chunks_written
            );
        }
        Ok(())
    }

    /// Step 3: Verify the proof using data from the PDA — real Groth16 + all checks
    /// Instruction data is only 16 bytes (discriminator + nonce)
    pub fn verify_proof_context(ctx: Context<VerifyProofContext>, _nonce: u64) -> Result<()> {
        let pc = &ctx.accounts.proof_context;

        // 0. Ensure proof context is complete
        require!(pc.is_complete, ZKLoRaError::ProofContextIncomplete);
        require!(
            pc.gateway == ctx.accounts.gateway.key(),
            ZKLoRaError::ProofContextUnauthorized
        );

        let merkle_proof: Vec<[u8; 32]> = pc.merkle_proof.to_vec();
        let leaf_index = pc.leaf_index;
        let shielded_pool = &ctx.accounts.shielded_pool;

        // 1. Verify Merkle membership proof (allowing historical roots)
        require!(
            merkle_proof.len() == MERKLE_DEPTH,
            ZKLoRaError::InvalidMerkleProof
        );
        require!(
            (leaf_index as u64) < shielded_pool.next_index,
            ZKLoRaError::InvalidMerkleProof
        );

        // Compute Merkle root from leaf + proof
        let leaf_hash = pc.public_inputs[0]; // identity_hash — the leaf deposited into pool
        let computed_root = compute_merkle_root_from_proof(&leaf_hash, &merkle_proof, leaf_index);

        let is_member = is_known_root(shielded_pool, &computed_root);
        require!(is_member, ZKLoRaError::InvalidMerkleProof);

        // 2. Firmware hash whitelist check
        let firmware_hash = pc.public_inputs[7]; // firmware_hash is public_input[7]
        let registry = &ctx.accounts.registry;
        require!(
            registry.approved_firmware_hashes.contains(&firmware_hash),
            ZKLoRaError::InvalidAttestation
        );

        // 3. Gateway public input binding
        // public_inputs[4] = gateway_part1 (lower 16 bytes)
        // public_inputs[5] = gateway_part2 (upper 16 bytes)
        let gateway_key_bytes = ctx.accounts.gateway.key().to_bytes();
        let mut expected_part1 = [0u8; 32];
        expected_part1[0..16].copy_from_slice(&gateway_key_bytes[0..16]);
        let mut expected_part2 = [0u8; 32];
        expected_part2[0..16].copy_from_slice(&gateway_key_bytes[16..32]);
        require!(
            pc.public_inputs[4] == expected_part1,
            ZKLoRaError::UnauthorizedGateway
        );
        require!(
            pc.public_inputs[5] == expected_part2,
            ZKLoRaError::UnauthorizedGateway
        );

        // 4. Groth16 proof verification via alt_bn128 precompile
        verify_groth16(&pc.proof_a, &pc.proof_b, &pc.proof_c, &pc.public_inputs)?;

        // 5. Nullifier spend prevention (PDA init ensures uniqueness)
        let nullifier_record = &mut ctx.accounts.nullifier_record;
        nullifier_record.nullifier_hash = pc.public_inputs[1];
        nullifier_record.spent_at = Clock::get()?.unix_timestamp;

        // 6. Escrow balance check & payouts
        let shielded_pool_info = ctx.accounts.shielded_pool.to_account_info();
        let gateway_info = ctx.accounts.gateway.to_account_info();
        let treasury_info = ctx.accounts.treasury.to_account_info();

        let rent_minimum = Rent::get()?.minimum_balance(shielded_pool_info.data_len());
        let available_balance = shielded_pool_info.lamports().saturating_sub(rent_minimum);
        require!(
            available_balance >= TOTAL_FEE_PER_CHIRP,
            ZKLoRaError::InsufficientEscrowFunding
        );

        // Transfer gateway reward
        **shielded_pool_info.try_borrow_mut_lamports()? = shielded_pool_info
            .lamports()
            .checked_sub(GATEWAY_REWARD_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;
        **gateway_info.try_borrow_mut_lamports()? = gateway_info
            .lamports()
            .checked_add(GATEWAY_REWARD_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        // Transfer protocol fee to treasury
        **shielded_pool_info.try_borrow_mut_lamports()? = shielded_pool_info
            .lamports()
            .checked_sub(PROTOCOL_FEE_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;
        **treasury_info.try_borrow_mut_lamports()? = treasury_info
            .lamports()
            .checked_add(PROTOCOL_FEE_LAMPORTS)
            .ok_or(ZKLoRaError::CalculationOverflow)?;

        // 7. Update registry stats
        let registry = &mut ctx.accounts.registry;
        registry.total_chirps_verified += 1;
        registry.total_fees_collected += TOTAL_FEE_PER_CHIRP;
        registry.total_gateway_rewards += GATEWAY_REWARD_LAMPORTS;
        registry.total_treasury_fees += PROTOCOL_FEE_LAMPORTS;

        msg!(
            "ZK-LoRaWAN: Chunked proof verified — gateway {} | nullifier {:?}... | firmware {:?}...",
            ctx.accounts.gateway.key(),
            &pc.public_inputs[1][..4],
            &firmware_hash[..4]
        );
        Ok(())
    }

    /// Step 4: Close the ProofContext PDA and return rent to gateway
    pub fn close_proof_context(_ctx: Context<CloseProofContext>, _nonce: u64) -> Result<()> {
        msg!("ProofContext closed, rent returned to gateway");
        Ok(())
    }

    /// Batch variant: add a chirp from a completed ProofContext PDA
    pub fn add_chirp_from_context(ctx: Context<AddChirpFromContext>, _nonce: u64) -> Result<()> {
        let pc = &ctx.accounts.proof_context;
        let batch = &mut ctx.accounts.batch;

        require!(pc.is_complete, ZKLoRaError::ProofContextIncomplete);
        require!(
            pc.gateway == ctx.accounts.gateway.key(),
            ZKLoRaError::ProofContextUnauthorized
        );
        require!(!batch.is_finalized, ZKLoRaError::BatchAlreadyFinalized);
        require!(
            batch.gateway == ctx.accounts.gateway.key(),
            ZKLoRaError::UnauthorizedGateway
        );
        require!(
            (batch.chirp_count as usize) < MAX_BATCH_SIZE,
            ZKLoRaError::BatchFull
        );

        // Verify Merkle membership
        let merkle_proof: Vec<[u8; 32]> = pc.merkle_proof.to_vec();
        require!(
            merkle_proof.len() == MERKLE_DEPTH,
            ZKLoRaError::InvalidMerkleProof
        );
        require!(
            (pc.leaf_index as u64) < ctx.accounts.shielded_pool.next_index,
            ZKLoRaError::InvalidMerkleProof
        );

        let leaf_hash = pc.public_inputs[0];
        let computed_root =
            compute_merkle_root_from_proof(&leaf_hash, &merkle_proof, pc.leaf_index);
        let is_member = is_known_root(&ctx.accounts.shielded_pool, &computed_root);
        require!(is_member, ZKLoRaError::InvalidMerkleProof);

        // Firmware hash whitelist check
        let firmware_hash = pc.public_inputs[7];
        require!(
            ctx.accounts
                .registry
                .approved_firmware_hashes
                .contains(&firmware_hash),
            ZKLoRaError::InvalidAttestation
        );

        // Gateway binding
        let gateway_key_bytes = ctx.accounts.gateway.key().to_bytes();
        let mut expected_part1 = [0u8; 32];
        expected_part1[0..16].copy_from_slice(&gateway_key_bytes[0..16]);
        let mut expected_part2 = [0u8; 32];
        expected_part2[0..16].copy_from_slice(&gateway_key_bytes[16..32]);
        require!(
            pc.public_inputs[4] == expected_part1,
            ZKLoRaError::UnauthorizedGateway
        );
        require!(
            pc.public_inputs[5] == expected_part2,
            ZKLoRaError::UnauthorizedGateway
        );

        // Add nullifier to batch
        batch.nullifiers.push(pc.public_inputs[1]);
        batch.chirp_count += 1;

        msg!(
            "ZK-LoRaWAN: Chirp added to batch #{} from context (count: {})",
            batch.batch_id,
            batch.chirp_count
        );
        Ok(())
    }

    pub fn migrate_accounts(ctx: Context<MigrateAccounts>) -> Result<()> {
        use anchor_lang::Discriminator;
        let registry_info = ctx.accounts.registry.to_account_info();
        let pool_info = ctx.accounts.shielded_pool.to_account_info();
        let authority_info = ctx.accounts.authority.to_account_info();
        let rent = Rent::get()?;

        // Upgrade Registry space to 500 if smaller
        let registry_target_size = 500;
        if registry_info.data_len() < registry_target_size {
            // Read old registry data: skip Anchor discriminator (first 8 bytes)
            let mut old_data_slice = &registry_info.data.borrow()[8..];
            let old_registry = OldProtocolRegistry::deserialize(&mut old_data_slice)?;

            let needed_rent = rent.minimum_balance(registry_target_size);
            let diff = needed_rent.saturating_sub(registry_info.lamports());
            if diff > 0 {
                anchor_lang::solana_program::program::invoke(
                    &anchor_lang::solana_program::system_instruction::transfer(
                        ctx.accounts.authority.key,
                        &ctx.accounts.registry.key(),
                        diff,
                    ),
                    &[
                        authority_info.clone(),
                        registry_info.clone(),
                        ctx.accounts.system_program.to_account_info(),
                    ],
                )?;
            }
            registry_info.realloc(registry_target_size, false)?;

            // Populate the new registry structure
            let new_registry = ProtocolRegistry {
                authority: old_registry.authority,
                next_batch_id: old_registry.next_batch_id,
                total_batches: old_registry.total_batches,
                total_chirps_verified: old_registry.total_chirps_verified,
                total_fees_collected: old_registry.total_fees_collected,
                total_gateway_rewards: old_registry.total_gateway_rewards,
                total_treasury_fees: old_registry.total_treasury_fees,
                created_at: old_registry.created_at,
                approved_firmware_hashes: vec![*b"enclave-firmware-version-v1.0.2\0"],
            };

            // Write Anchor discriminator and serialized data
            let mut data_borrow = registry_info.data.borrow_mut();
            data_borrow[..8].copy_from_slice(&ProtocolRegistry::DISCRIMINATOR);
            let mut writer = &mut data_borrow[8..];
            new_registry.serialize(&mut writer)?;
            msg!("Registry migrated to 500 bytes");
        }

        // Upgrade Pool space to 900 if smaller
        let pool_target_size = 708;
        if pool_info.data_len() < pool_target_size {
            // Read old pool data: skip Anchor discriminator (first 8 bytes)
            let mut old_data_slice = &pool_info.data.borrow()[8..];
            let old_pool = OldShieldedEscrowPool::deserialize(&mut old_data_slice)?;

            let needed_rent = rent.minimum_balance(pool_target_size);
            let diff = needed_rent.saturating_sub(pool_info.lamports());
            if diff > 0 {
                anchor_lang::solana_program::program::invoke(
                    &anchor_lang::solana_program::system_instruction::transfer(
                        ctx.accounts.authority.key,
                        &ctx.accounts.shielded_pool.key(),
                        diff,
                    ),
                    &[
                        authority_info.clone(),
                        pool_info.clone(),
                        ctx.accounts.system_program.to_account_info(),
                    ],
                )?;
            }
            pool_info.realloc(pool_target_size, false)?;

            // Initialize subtrees & roots history
            let mut filled_subtrees = [[0u8; 32]; MERKLE_DEPTH];
            filled_subtrees[0] = old_pool.merkle_root;

            let mut roots_history = [[0u8; 32]; 8];
            roots_history[0] = old_pool.merkle_root;

            let new_pool = ShieldedEscrowPool {
                total_balance: old_pool.total_balance,
                merkle_root: old_pool.merkle_root,
                last_updated: old_pool.last_updated,
                next_index: 0,
                filled_subtrees,
                roots_history,
                history_index: 1,
                bump: old_pool.bump,
            };

            // Write Anchor discriminator and serialized data
            let mut data_borrow = pool_info.data.borrow_mut();
            data_borrow[..8].copy_from_slice(&ShieldedEscrowPool::DISCRIMINATOR);
            let mut writer = &mut data_borrow[8..];
            new_pool.serialize(&mut writer)?;
            msg!("Shielded pool migrated to 708 bytes");
        }

        Ok(())
    }
}

#[derive(Accounts)]
pub struct RegisterFirmware<'info> {
    #[account(
        mut,
        seeds = [REGISTRY_SEED],
        bump,
        has_one = authority,
    )]
    pub registry: Account<'info, ProtocolRegistry>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

// ============================================================================
// Merkle Tree
// ============================================================================

fn compute_merkle_root(leaves: &Vec<[u8; 32]>) -> [u8; 32] {
    if leaves.is_empty() {
        return [0u8; 32];
    }
    if leaves.len() == 1 {
        return leaves[0];
    }

    let mut current_level: Vec<[u8; 32]> = leaves.clone();

    while current_level.len() > 1 {
        let mut next_level = Vec::new();

        for i in (0..current_level.len()).step_by(2) {
            let right = if i + 1 < current_level.len() {
                current_level[i + 1]
            } else {
                current_level[i] // odd leaf duplicates
            };

            let combined = [current_level[i].as_slice(), right.as_slice()].concat();
            next_level.push(keccak::hash(&combined).to_bytes());
        }

        current_level = next_level;
    }

    current_level[0]
}

fn compute_merkle_root_from_proof(
    leaf: &[u8; 32],
    proof: &Vec<[u8; 32]>,
    leaf_index: u32,
) -> [u8; 32] {
    let mut current = *leaf;
    let mut index = leaf_index;

    for sibling in proof {
        let combined = if index % 2 == 0 {
            [current.as_slice(), sibling.as_slice()].concat()
        } else {
            [sibling.as_slice(), current.as_slice()].concat()
        };
        current = keccak::hash(&combined).to_bytes();
        index /= 2;
    }

    current
}

fn verify_merkle_proof(
    leaf: &[u8; 32],
    proof: &Vec<[u8; 32]>,
    leaf_index: u32,
    root: &[u8; 32],
) -> bool {
    let computed = compute_merkle_root_from_proof(leaf, proof, leaf_index);
    computed == *root
}

fn is_known_root(pool: &ShieldedEscrowPool, root: &[u8; 32]) -> bool {
    if pool.merkle_root == *root {
        return true;
    }
    for historical_root in pool.roots_history.iter() {
        if *historical_root == *root {
            return true;
        }
    }
    false
}

fn get_zero_hash(level: usize) -> [u8; 32] {
    let mut current = [0u8; 32];
    for _ in 0..level {
        let combined = [current.as_slice(), current.as_slice()].concat();
        current = keccak::hash(&combined).to_bytes();
    }
    current
}

fn insert_leaf_on_chain(
    filled_subtrees: &mut [[u8; 32]; MERKLE_DEPTH],
    next_index: u64,
    leaf: [u8; 32],
) -> [u8; 32] {
    let mut current_level_hash = leaf;
    let mut index = next_index;

    for level in 0..MERKLE_DEPTH {
        if index % 2 == 0 {
            filled_subtrees[level] = current_level_hash;
            let zero_hash = get_zero_hash(level);
            let combined = [current_level_hash.as_slice(), zero_hash.as_slice()].concat();
            current_level_hash = keccak::hash(&combined).to_bytes();
        } else {
            let left_hash = filled_subtrees[level];
            let combined = [left_hash.as_slice(), current_level_hash.as_slice()].concat();
            current_level_hash = keccak::hash(&combined).to_bytes();
        }
        index /= 2;
    }
    current_level_hash
}

fn verify_firmware_hash(registry: &ProtocolRegistry, hash: &[u8; 32]) -> bool {
    if registry.approved_firmware_hashes.is_empty() {
        // Fallback default approved firmware hash (for initial setup/demo compatibility)
        let mut default_hash = [0u8; 32];
        let default_bytes = b"enclave-firmware-version-v1.0.2";
        default_hash[..default_bytes.len()].copy_from_slice(default_bytes);
        return hash == &default_hash;
    }
    registry.approved_firmware_hashes.contains(hash)
}

// ============================================================================
// Zero Knowledge Proof Verification via alt_bn128 Precompile Syscall
// ============================================================================

fn negate_fp2(y: &[u8; 64]) -> Result<[u8; 64]> {
    let p = [
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58,
        0x5d, 0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c,
        0xfd, 0x47,
    ];
    let mut negated = [0u8; 64];
    negate_fp(&y[0..32], &p, &mut negated[0..32])?;
    negate_fp(&y[32..64], &p, &mut negated[32..64])?;
    Ok(negated)
}

fn negate_fp(val: &[u8], p: &[u8; 32], out: &mut [u8]) -> Result<()> {
    let mut is_zero = true;
    for &b in val {
        if b != 0 {
            is_zero = false;
            break;
        }
    }
    if is_zero {
        out.copy_from_slice(&[0u8; 32]);
        return Ok(());
    }

    let mut borrow = 0i16;
    for i in (0..32).rev() {
        let p_digit = p[i] as i16;
        let val_digit = val[i] as i16;
        let diff = p_digit - val_digit - borrow;
        if diff < 0 {
            out[i] = (diff + 256) as u8;
            borrow = 1;
        } else {
            out[i] = diff as u8;
            borrow = 0;
        }
    }
    if borrow > 0 {
        return err!(ZKLoRaError::CalculationOverflow);
    }
    Ok(())
}

pub fn verify_groth16(
    proof_a: &[u8; 64],
    proof_b: &[u8; 128],
    proof_c: &[u8; 64],
    public_inputs: &[[u8; 32]; 8],
) -> Result<()> {
    // Verifying Key (VK) coordinates from secure random single-party setup for demo/devnet (8 public inputs)
    let vk_alpha: [u8; 64] = [
        0x27, 0xdd, 0x7c, 0xd6, 0xaf, 0xf9, 0x1a, 0xeb, 0xd1, 0x9c, 0xb6, 0x7f, 0xd5, 0xe6, 0x86,
        0x3b, 0x28, 0x9b, 0xb0, 0x28, 0xd8, 0x1c, 0x45, 0xbc, 0x57, 0x14, 0xd7, 0x97, 0x47, 0xbb,
        0x2b, 0x98, 0x05, 0x39, 0x6e, 0x80, 0xa0, 0x4a, 0x3b, 0xac, 0xd2, 0xa3, 0x9b, 0xcf, 0xdc,
        0x58, 0x80, 0x2d, 0xc0, 0xc7, 0x61, 0x75, 0x6c, 0x18, 0x56, 0x18, 0xe7, 0xcf, 0x78, 0xe7,
        0x9d, 0x7b, 0x62, 0x0c,
    ];

    let vk_beta: [u8; 128] = [
        0x06, 0x6c, 0x1c, 0x42, 0x61, 0x77, 0x13, 0xeb, 0x59, 0x5f, 0xe7, 0x71, 0xc4, 0xf9, 0x64,
        0x3e, 0x96, 0x0d, 0x67, 0x5f, 0xb9, 0x83, 0x15, 0xf9, 0xb3, 0x33, 0x38, 0x45, 0xa3, 0xde,
        0xf0, 0x53, 0x1e, 0x4e, 0x45, 0xfb, 0x08, 0x36, 0x49, 0xde, 0x27, 0x0a, 0xe6, 0x83, 0xbd,
        0x7d, 0xfa, 0x45, 0x6c, 0xaa, 0xe6, 0x5a, 0x6c, 0x61, 0xed, 0x1e, 0x1e, 0x9a, 0x27, 0x2d,
        0xc5, 0x7e, 0xa0, 0x0b, 0x12, 0xc8, 0x6d, 0xe3, 0xd0, 0x8e, 0x08, 0xf9, 0x9c, 0xfd, 0x27,
        0xf1, 0x8f, 0x21, 0x6a, 0x80, 0x94, 0x64, 0xca, 0xbe, 0x74, 0xa0, 0xa0, 0xc5, 0xaf, 0xbd,
        0x37, 0xe7, 0x7a, 0x49, 0x94, 0x94, 0x2c, 0x63, 0xc8, 0x1e, 0x53, 0x43, 0x9e, 0x73, 0xf0,
        0xed, 0x59, 0x5b, 0xf5, 0xd7, 0xee, 0x83, 0x72, 0x05, 0x83, 0x81, 0xe0, 0x2e, 0x58, 0xe4,
        0x32, 0x43, 0xdb, 0x47, 0x34, 0x7a, 0x1b, 0xee,
    ];

    let vk_gamma: [u8; 128] = [
        0x13, 0xde, 0x90, 0x24, 0x6f, 0x72, 0xe0, 0x36, 0x81, 0xc3, 0xa0, 0x02, 0x65, 0xde, 0xbb,
        0x94, 0xa2, 0xe7, 0xfe, 0x0f, 0x6a, 0x5d, 0xe1, 0x5f, 0xc6, 0xff, 0xf5, 0x47, 0x49, 0x36,
        0x08, 0x68, 0x04, 0x42, 0x57, 0x0a, 0x2c, 0xb4, 0x91, 0x89, 0x26, 0xd0, 0x2c, 0x4f, 0xc2,
        0x0a, 0x15, 0x36, 0xb9, 0x35, 0x7f, 0x6e, 0x47, 0x1b, 0x42, 0xc5, 0xe9, 0xa0, 0x61, 0x80,
        0xe2, 0xc1, 0xb4, 0x08, 0x0e, 0xd4, 0xe3, 0xb1, 0xa2, 0x11, 0xe2, 0xab, 0x3b, 0xfe, 0xde,
        0x9b, 0xd4, 0x49, 0x06, 0xab, 0xc8, 0xdd, 0xfa, 0xf4, 0x5d, 0x51, 0xd1, 0x4c, 0x75, 0x26,
        0x99, 0x81, 0x45, 0x91, 0x69, 0x11, 0x11, 0xfb, 0x4d, 0x8d, 0x0d, 0xc1, 0x54, 0x8b, 0xd9,
        0x7c, 0x43, 0xab, 0xb5, 0x00, 0x72, 0x63, 0x9e, 0x20, 0x98, 0xda, 0xfe, 0xef, 0xe2, 0x68,
        0xea, 0xbb, 0xc9, 0xb2, 0x07, 0x0b, 0x57, 0x3d,
    ];

    let vk_delta: [u8; 128] = [
        0x03, 0x00, 0xe8, 0xa1, 0xf8, 0x29, 0xd6, 0x88, 0x00, 0x70, 0x18, 0x04, 0x61, 0x51, 0xae,
        0x76, 0x87, 0xb4, 0x0b, 0x4c, 0x91, 0x69, 0xd0, 0x5f, 0xbb, 0x74, 0xc5, 0x6d, 0xb8, 0x63,
        0x2f, 0x29, 0x1d, 0x9e, 0xfd, 0x3d, 0x71, 0x26, 0x37, 0x54, 0x64, 0x75, 0xa5, 0x07, 0xc2,
        0xb0, 0xde, 0x18, 0x32, 0x60, 0xbe, 0xfa, 0x64, 0xf7, 0xec, 0x35, 0xbc, 0xc0, 0xb4, 0xa6,
        0x0f, 0xf0, 0x6c, 0xb1, 0x2e, 0xbc, 0xaf, 0x8c, 0x28, 0x2a, 0xe7, 0xd5, 0x41, 0x49, 0x46,
        0xbc, 0xab, 0x9a, 0x67, 0x35, 0x34, 0x0b, 0xd1, 0x20, 0x57, 0x32, 0x39, 0x02, 0x71, 0x4c,
        0x67, 0xd0, 0x6d, 0xa3, 0x94, 0x77, 0x04, 0x86, 0xe6, 0xcd, 0x0f, 0xa3, 0x1a, 0x47, 0x5c,
        0x56, 0xff, 0x6d, 0xfd, 0xdc, 0x3c, 0xe1, 0x2e, 0x13, 0xce, 0x26, 0x11, 0x62, 0x6f, 0x92,
        0x74, 0x23, 0xad, 0x34, 0x16, 0xcb, 0x4e, 0x56,
    ];

    // IC points for 8 public inputs (generated with OsRng)
    let vk_ic: [[u8; 64]; 9] = [
        // vk_ic[0]
        [
            0x26, 0x45, 0x20, 0x6a, 0x92, 0xda, 0x64, 0x2d, 0x5e, 0x53, 0x0e, 0x21, 0x6c, 0x52,
            0x9a, 0x6e, 0x46, 0x1e, 0xcb, 0xc0, 0x20, 0xbc, 0x36, 0x19, 0x79, 0x2b, 0x2f, 0x14,
            0xa0, 0x78, 0x79, 0xc2, 0x02, 0xfe, 0xc8, 0x36, 0x4f, 0x40, 0x78, 0x10, 0xc0, 0xca,
            0x24, 0xdd, 0x9d, 0xa6, 0x41, 0xe2, 0x81, 0x33, 0x62, 0xb7, 0xc3, 0x54, 0x82, 0xb8,
            0x70, 0x5a, 0xd6, 0x12, 0xc5, 0x27, 0x54, 0x7e,
        ],
        // vk_ic[1] — identity_hash
        [
            0x24, 0x47, 0xe6, 0x75, 0xf9, 0x82, 0x54, 0x29, 0x48, 0x16, 0x70, 0x60, 0xe4, 0xbc,
            0xea, 0x6b, 0xe5, 0x7a, 0x30, 0xe1, 0xfa, 0x92, 0xa1, 0xbb, 0x08, 0xf8, 0xe6, 0x84,
            0xe4, 0x3e, 0xd4, 0x68, 0x2c, 0xb9, 0x69, 0xac, 0xce, 0xbe, 0xe2, 0x53, 0x29, 0x82,
            0xa2, 0x3f, 0xa4, 0x3f, 0xd1, 0xec, 0xaa, 0x7e, 0x9f, 0x6a, 0xfd, 0x16, 0x0e, 0x00,
            0xb1, 0xbb, 0x45, 0xc5, 0x46, 0xa9, 0xd2, 0xa6,
        ],
        // vk_ic[2] — nullifier_hash
        [
            0x04, 0x32, 0x98, 0xa3, 0x33, 0x5e, 0x67, 0xa4, 0x13, 0x0e, 0x3f, 0xbc, 0x1a, 0x50,
            0x2c, 0x9e, 0xd9, 0x8b, 0xfa, 0x6c, 0xaa, 0x45, 0xb5, 0xb2, 0xc2, 0xe8, 0x47, 0x05,
            0xc4, 0x65, 0x9d, 0x56, 0x1f, 0xe1, 0xb9, 0xa2, 0xeb, 0xbf, 0x56, 0x18, 0x25, 0xa6,
            0x74, 0x46, 0x7d, 0x4c, 0xf8, 0x1b, 0x86, 0x23, 0x54, 0x64, 0xeb, 0x6a, 0x5a, 0x79,
            0xc7, 0x6a, 0x1f, 0x4f, 0xb2, 0x69, 0x10, 0xa9,
        ],
        // vk_ic[3] — attestation_hash
        [
            0x05, 0x3c, 0x99, 0xba, 0x63, 0x73, 0xc1, 0x1c, 0x1d, 0x18, 0x65, 0xae, 0x29, 0x12,
            0xfe, 0x79, 0x08, 0x42, 0xba, 0x42, 0x49, 0x5d, 0xac, 0x93, 0x25, 0xd5, 0xa7, 0x61,
            0x80, 0xdd, 0x4e, 0x69, 0x04, 0x49, 0xbc, 0xff, 0x94, 0x89, 0x0a, 0x24, 0xd0, 0x5c,
            0x78, 0x18, 0xa8, 0x45, 0x41, 0x4a, 0x5c, 0x94, 0xde, 0x9e, 0xe7, 0x2d, 0xde, 0xdd,
            0xcc, 0xad, 0x59, 0xf5, 0x28, 0x3d, 0xec, 0x1a,
        ],
        // vk_ic[4] — ciphertext_hash
        [
            0x08, 0x53, 0x22, 0xf4, 0x9e, 0x29, 0x52, 0x9f, 0x8f, 0x11, 0x71, 0x1f, 0xcd, 0xf8,
            0xac, 0x81, 0x91, 0x8c, 0x0d, 0x92, 0x9f, 0x04, 0x38, 0x0c, 0xe7, 0x83, 0x39, 0x6c,
            0x8f, 0xe2, 0x1e, 0x78, 0x20, 0x03, 0x0b, 0xb4, 0x05, 0x93, 0x2b, 0xb6, 0xbd, 0xed,
            0x50, 0x16, 0x12, 0x1e, 0x54, 0x4f, 0xda, 0x4d, 0xfa, 0x6a, 0xd2, 0x3e, 0x95, 0x85,
            0xa4, 0x39, 0x60, 0x31, 0xff, 0x2c, 0xc3, 0x9e,
        ],
        // vk_ic[5] — gateway_part1
        [
            0x07, 0xe5, 0xbb, 0x30, 0xa2, 0x06, 0x3a, 0x45, 0xb5, 0xd0, 0x9b, 0x4d, 0x28, 0x01,
            0x1b, 0xc3, 0xbb, 0xb2, 0x35, 0x0f, 0x85, 0xe4, 0xa4, 0x0b, 0xab, 0xac, 0x27, 0xb3,
            0x5c, 0x67, 0xde, 0x73, 0x1e, 0xbc, 0x8c, 0x24, 0xbc, 0x9f, 0x25, 0x68, 0x57, 0x09,
            0xf3, 0x53, 0xe5, 0x7b, 0x54, 0xd9, 0xcb, 0xdd, 0xd7, 0xf6, 0x43, 0x14, 0x24, 0x17,
            0x6b, 0xd2, 0x36, 0x5b, 0xee, 0x05, 0xa9, 0xa3,
        ],
        // vk_ic[6] — gateway_part2
        [
            0x1b, 0x79, 0xc8, 0x7e, 0xf7, 0x8f, 0xd3, 0x5e, 0x95, 0xa9, 0x34, 0x11, 0x4a, 0x19,
            0xb4, 0xdb, 0x6d, 0xf4, 0x37, 0x22, 0x4d, 0xf8, 0x48, 0xf1, 0xec, 0x35, 0x80, 0xcf,
            0x63, 0x13, 0x82, 0xd8, 0x0b, 0xed, 0x4d, 0xd0, 0x14, 0xe8, 0x17, 0x40, 0x16, 0x82,
            0x14, 0xb8, 0x26, 0x2a, 0xd2, 0xd6, 0xe0, 0x89, 0x57, 0x04, 0xb4, 0xa6, 0x3f, 0xbf,
            0xbe, 0x0a, 0x9b, 0xe0, 0xe4, 0xdc, 0x17, 0x80,
        ],
        // vk_ic[7] — deposit_commitment
        [
            0x03, 0x41, 0x13, 0xd3, 0xe8, 0x2b, 0x4b, 0xc4, 0x71, 0x29, 0x38, 0xf4, 0x9f, 0xd9,
            0x99, 0xb7, 0x64, 0x26, 0x13, 0x1b, 0xb5, 0x3b, 0xbe, 0xe6, 0xdd, 0x97, 0x34, 0x71,
            0xee, 0xc9, 0x0a, 0x1b, 0x1e, 0xfb, 0xdf, 0x52, 0xa8, 0x39, 0x42, 0x35, 0x77, 0x3b,
            0x97, 0xad, 0xab, 0xad, 0xa7, 0x07, 0x6c, 0x1b, 0xba, 0xdb, 0x21, 0x45, 0x9b, 0x44,
            0xd8, 0x4c, 0x56, 0x1f, 0x11, 0xe9, 0x16, 0x7b,
        ],
        // vk_ic[8] — firmware_hash_public
        [
            0x26, 0xe3, 0x45, 0xfa, 0xc1, 0x23, 0xbd, 0x48, 0xef, 0x94, 0x34, 0x37, 0x4b, 0xf9,
            0xae, 0x3f, 0x93, 0xea, 0xf7, 0x30, 0xbb, 0xba, 0xf6, 0x9a, 0x80, 0x57, 0xd5, 0xa2,
            0xeb, 0xe8, 0x30, 0x0a, 0x16, 0xb1, 0xb5, 0xb3, 0xea, 0xd7, 0xa0, 0x94, 0x39, 0x3f,
            0xd3, 0xd2, 0x00, 0xa2, 0xd0, 0x70, 0x30, 0x90, 0x18, 0x99, 0x76, 0x69, 0x1f, 0xd0,
            0xe7, 0xd8, 0x0c, 0x53, 0x57, 0x59, 0x36, 0xad,
        ],
    ];

    // Compute public input G1 point:
    // IC_total = vk_ic[0] + \sum_{i=0}^{7} public_inputs[i] * vk_ic[i+1]
    let mut current_ic = vk_ic[0];

    for i in 0..8 {
        let mut mul_input = [0u8; 96];
        mul_input[0..64].copy_from_slice(&vk_ic[i + 1]);
        for (dst, src) in mul_input[64..96]
            .iter_mut()
            .zip(public_inputs[i].iter().rev())
        {
            *dst = *src;
        }

        let scaled_point_vec = alt_bn128_multiplication(&mul_input)
            .map_err(|_| error!(ZKLoRaError::InvalidZeroKnowledgeProof))?;
        let scaled_point: [u8; 64] = scaled_point_vec
            .try_into()
            .map_err(|_| error!(ZKLoRaError::InvalidZeroKnowledgeProof))?;

        let mut add_input = [0u8; 128];
        add_input[0..64].copy_from_slice(&current_ic);
        add_input[64..128].copy_from_slice(&scaled_point);

        let added_point_vec = alt_bn128_addition(&add_input)
            .map_err(|_| error!(ZKLoRaError::InvalidZeroKnowledgeProof))?;
        current_ic = added_point_vec
            .try_into()
            .map_err(|_| error!(ZKLoRaError::InvalidZeroKnowledgeProof))?;
    }

    let mut pairing_input = [0u8; 192 * 4];

    // 1. proof_a (G1) and negated proof_b (G2)
    pairing_input[0..64].copy_from_slice(proof_a);

    // X coordinate of proof_b
    pairing_input[64..128].copy_from_slice(&proof_b[0..64]);
    // Negated Y coordinate of proof_b
    let negated_y = negate_fp2(proof_b[64..128].try_into().unwrap())?;
    pairing_input[128..192].copy_from_slice(&negated_y);

    // 2. vk_alpha (G1) and vk_beta (G2)
    pairing_input[192..256].copy_from_slice(&vk_alpha);
    pairing_input[256..384].copy_from_slice(&vk_beta);

    // 3. current_ic (G1) and vk_gamma (G2)
    pairing_input[384..448].copy_from_slice(&current_ic);
    pairing_input[448..576].copy_from_slice(&vk_gamma);

    // 4. proof_c (G1) and vk_delta (G2)
    pairing_input[576..640].copy_from_slice(proof_c);
    pairing_input[640..768].copy_from_slice(&vk_delta);

    // Execute AltBn128 pairing check on Solana runtime
    let result = alt_bn128_pairing(&pairing_input)
        .map_err(|_| error!(ZKLoRaError::InvalidZeroKnowledgeProof))?;

    if result
        != [
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 1,
        ]
    {
        return err!(ZKLoRaError::InvalidZeroKnowledgeProof);
    }

    Ok(())
}

// ============================================================================
// Accounts
// ============================================================================

#[account]
pub struct ProtocolRegistry {
    pub authority: Pubkey,
    pub next_batch_id: u64,
    pub total_batches: u64,
    pub total_chirps_verified: u64,
    pub total_fees_collected: u64,
    pub total_gateway_rewards: u64,
    pub total_treasury_fees: u64,
    pub created_at: i64,
    pub approved_firmware_hashes: Vec<[u8; 32]>,
}

#[account]
pub struct BatchAccumulator {
    pub gateway: Pubkey,
    pub batch_id: u64,
    pub chirp_count: u32,
    pub is_finalized: bool,
    pub created_at: i64,
    pub finalized_at: i64,
    pub merkle_root: [u8; 32],
    pub nullifiers: Vec<[u8; 32]>,
}

#[account]
pub struct ShieldedEscrowPool {
    pub total_balance: u64,
    pub merkle_root: [u8; 32],
    pub last_updated: i64,
    pub next_index: u64,
    pub filled_subtrees: [[u8; 32]; MERKLE_DEPTH],
    pub roots_history: [[u8; 32]; 8],
    pub history_index: u64,
    pub bump: u8,
}

#[account]
pub struct NullifierRecord {
    pub nullifier_hash: [u8; 32],
    pub spent_at: i64,
}

/// Staged proof context PDA — accumulates proof data across multiple small
/// transactions so the final verify instruction carries only a 16-byte payload.
#[account]
pub struct ProofContext {
    pub gateway: Pubkey,                        // 32
    pub proof_a: [u8; 64],                      // 64
    pub proof_b: [u8; 128],                     // 128
    pub proof_c: [u8; 64],                      // 64
    pub public_inputs: [[u8; 32]; 8],           // 256
    pub merkle_proof: [[u8; 32]; MERKLE_DEPTH], // 320
    pub leaf_index: u32,                        // 4
    pub chunks_written: u8,                     // 1
    pub is_complete: bool,                      // 1
    pub created_at: i64,                        // 8
    pub bump: u8,                               // 1
}
pub const PROOF_CONTEXT_SEED: &[u8] = b"proof-ctx";

// ============================================================================
// Instruction Contexts
// ============================================================================

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = 500, // 8 + 32 + 8 + 8 + 8 + 8 + 8 + 8 + 8 + (4 + 10 * 32)
        seeds = [REGISTRY_SEED],
        bump,
    )]
    pub registry: Account<'info, ProtocolRegistry>,
    #[account(mut, address = ADMIN_AUTHORITY)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct DepositShielded<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    #[account(
        init_if_needed,
        payer = sender,
        space = 708, // 8 + 8 + 32 + 8 + 8 + 16 * 32 + 8 * 32 + 8 + 1
        seeds = [b"shielded-pool"],
        bump
    )]
    pub shielded_pool: Account<'info, ShieldedEscrowPool>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    proof_a: [u8; 64],
    proof_b: [u8; 128],
    proof_c: [u8; 64],
    nullifier_hash: [u8; 32]
)]
pub struct VerifySingle<'info> {
    #[account(mut, seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    #[account(
        mut,
        seeds = [b"shielded-pool"],
        bump,
    )]
    pub shielded_pool: Account<'info, ShieldedEscrowPool>,
    #[account(
        init,
        payer = gateway,
        space = 8 + 32 + 8,
        seeds = [b"nullifier", nullifier_hash.as_ref()],
        bump,
    )]
    pub nullifier_record: Account<'info, NullifierRecord>,
    #[account(mut)]
    pub gateway: Signer<'info>,
    /// CHECK: Protocol treasury wallet verified via hardcoded address
    #[account(mut, address = pubkey!("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS"))]
    pub treasury: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(public_inputs: [[u8; 32]; 8])]
pub struct VerifySingleProof<'info> {
    #[account(mut, seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    /// CHECK: Seeds checked on-chain, deserialized on the heap in handler
    #[account(
        mut,
        seeds = [b"shielded-pool"],
        bump,
    )]
    pub shielded_pool: UncheckedAccount<'info>,
    #[account(
        init,
        payer = gateway,
        space = 8 + 32 + 8,
        seeds = [b"nullifier", public_inputs[1].as_ref()],
        bump,
    )]
    pub nullifier_record: Account<'info, NullifierRecord>,
    #[account(mut)]
    pub gateway: Signer<'info>,
    /// CHECK: Protocol treasury wallet verified via hardcoded address
    #[account(mut, address = pubkey!("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS"))]
    pub treasury: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeBatch<'info> {
    #[account(
        init,
        payer = gateway,
        space = 8 + 32 + 8 + 4 + 1 + 8 + 8 + 32
              + (4 + 32 * MAX_BATCH_SIZE),
        seeds = [BATCH_SEED, gateway.key().as_ref(), &registry.next_batch_id.to_le_bytes()],
        bump,
    )]
    pub batch: Account<'info, BatchAccumulator>,
    #[account(mut, seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    #[account(mut)]
    pub gateway: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AddChirp<'info> {
    #[account(mut)]
    pub batch: Account<'info, BatchAccumulator>,
    #[account(
        seeds = [b"shielded-pool"],
        bump,
    )]
    pub shielded_pool: Account<'info, ShieldedEscrowPool>,
    #[account(seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    pub gateway: Signer<'info>,
}

#[derive(Accounts)]
pub struct SubmitBatch<'info> {
    #[account(mut)]
    pub batch: Account<'info, BatchAccumulator>,
    #[account(mut, seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    #[account(
        mut,
        seeds = [b"shielded-pool"],
        bump,
    )]
    pub shielded_pool: Account<'info, ShieldedEscrowPool>,
    #[account(mut)]
    pub gateway: Signer<'info>,
    /// CHECK: Protocol treasury wallet verified via hardcoded address
    #[account(mut, address = pubkey!("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS"))]
    pub treasury: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyChirpInclusion<'info> {
    pub batch: Account<'info, BatchAccumulator>,
}

// ============================================================================
// Chunked Proof Flow — Instruction Contexts
// ============================================================================

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct InitializeProofContext<'info> {
    #[account(
        init,
        payer = gateway,
        space = 887, // 8 + 32 + 64 + 128 + 64 + 256 + 512 + 4 + 1 + 1 + 8 + 1
        seeds = [PROOF_CONTEXT_SEED, gateway.key().as_ref(), &nonce.to_le_bytes()],
        bump,
    )]
    pub proof_context: Account<'info, ProofContext>,
    #[account(mut)]
    pub gateway: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct WriteProofChunk<'info> {
    #[account(
        mut,
        seeds = [PROOF_CONTEXT_SEED, gateway.key().as_ref(), &nonce.to_le_bytes()],
        bump = proof_context.bump,
    )]
    pub proof_context: Account<'info, ProofContext>,
    pub gateway: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct VerifyProofContext<'info> {
    #[account(
        mut,
        seeds = [PROOF_CONTEXT_SEED, gateway.key().as_ref(), &nonce.to_le_bytes()],
        bump = proof_context.bump,
    )]
    pub proof_context: Account<'info, ProofContext>,
    #[account(mut, seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    #[account(
        mut,
        seeds = [b"shielded-pool"],
        bump,
    )]
    pub shielded_pool: Account<'info, ShieldedEscrowPool>,
    #[account(
        init,
        payer = gateway,
        space = 8 + 32 + 8,
        seeds = [b"nullifier", proof_context.public_inputs[1].as_ref()],
        bump,
    )]
    pub nullifier_record: Account<'info, NullifierRecord>,
    #[account(mut)]
    pub gateway: Signer<'info>,
    /// CHECK: Protocol treasury wallet verified via hardcoded address
    #[account(mut, address = pubkey!("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS"))]
    pub treasury: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct CloseProofContext<'info> {
    #[account(
        mut,
        close = gateway,
        seeds = [PROOF_CONTEXT_SEED, gateway.key().as_ref(), &nonce.to_le_bytes()],
        bump = proof_context.bump,
    )]
    pub proof_context: Account<'info, ProofContext>,
    #[account(mut)]
    pub gateway: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct AddChirpFromContext<'info> {
    #[account(
        mut,
        seeds = [PROOF_CONTEXT_SEED, gateway.key().as_ref(), &nonce.to_le_bytes()],
        bump = proof_context.bump,
    )]
    pub proof_context: Account<'info, ProofContext>,
    #[account(mut)]
    pub batch: Account<'info, BatchAccumulator>,
    #[account(
        seeds = [b"shielded-pool"],
        bump,
    )]
    pub shielded_pool: Account<'info, ShieldedEscrowPool>,
    #[account(seeds = [REGISTRY_SEED], bump)]
    pub registry: Account<'info, ProtocolRegistry>,
    pub gateway: Signer<'info>,
}

// ============================================================================
// Errors
// ============================================================================
#[error_code]
pub enum ZKLoRaError {
    #[msg("Batch is already finalized")]
    BatchAlreadyFinalized,
    #[msg("Batch is full (max 100 chirps)")]
    BatchFull,
    #[msg("Only the batch gateway can add chirps")]
    UnauthorizedGateway,
    #[msg("Batch has no chirps to submit")]
    EmptyBatch,
    #[msg("Batch must be finalized before verifying inclusion")]
    BatchNotFinalized,
    #[msg("Merkle proof does not match batch root")]
    InvalidMerkleProof,
    #[msg("Escrow account is not provided in remaining accounts")]
    EscrowAccountMissing,
    #[msg("Escrow account is not owned by the program")]
    InvalidEscrowOwner,
    #[msg("Numerical calculation overflow")]
    CalculationOverflow,
    #[msg("Escrow account lacks sufficient SOL balance")]
    InsufficientEscrowFunding,
    #[msg("Zero Knowledge witness verification failed")]
    InvalidZeroKnowledgeProof,
    #[msg("Nullifier has already been spent")]
    NullifierAlreadySpent,
    #[msg("Missing nullifier account in remaining accounts")]
    NullifierAccountMissing,
    #[msg("Invalid firmware attestation hash")]
    InvalidAttestation,
    #[msg("Invalid zk-VDE proof hash")]
    InvalidVdeProof,
    #[msg("Invalid ciphertext hash")]
    InvalidCiphertext,
    #[msg("Invalid deposit amount, must be exactly 150,000 lamports")]
    InvalidDepositAmount,
    #[msg("Proof context is not complete (missing chunks)")]
    ProofContextIncomplete,
    #[msg("Invalid proof chunk index")]
    InvalidChunkIndex,
    #[msg("Only the original gateway can modify this proof context")]
    ProofContextUnauthorized,
    #[msg("Proof context chunk already written")]
    ChunkAlreadyWritten,
    #[msg("Invalid chunk data length")]
    InvalidChunkDataLength,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OldProtocolRegistry {
    pub authority: Pubkey,
    pub next_batch_id: u64,
    pub total_batches: u64,
    pub total_chirps_verified: u64,
    pub total_fees_collected: u64,
    pub total_gateway_rewards: u64,
    pub total_treasury_fees: u64,
    pub created_at: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct OldShieldedEscrowPool {
    pub total_balance: u64,
    pub merkle_root: [u8; 32],
    pub last_updated: i64,
    pub bump: u8,
}

#[derive(Accounts)]
pub struct MigrateAccounts<'info> {
    /// CHECK: Checked manually inside instruction handler
    #[account(mut, seeds = [REGISTRY_SEED], bump)]
    pub registry: UncheckedAccount<'info>,
    /// CHECK: Checked manually inside instruction handler
    #[account(mut, seeds = [b"shielded-pool"], bump)]
    pub shielded_pool: UncheckedAccount<'info>,
    #[account(mut, address = ADMIN_AUTHORITY)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}
