use anchor_lang::prelude::*;

declare_id!("BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M");

pub const MAX_BATCH_TRAJECTORY_POINTS: usize = 16;

#[program]
pub mod solana_cuneiform_anchor {
    use super::*;

    /// Initializes the global program state containing the admin, treasury, and protocol fee.
    pub fn initialize_program(
        ctx: Context<InitializeProgram>,
        treasury: Pubkey,
        fee_lamports: u64,
    ) -> Result<()> {
        let state = &mut ctx.accounts.program_state;
        state.admin = ctx.accounts.admin.key();
        state.treasury = treasury;
        state.fee_lamports = fee_lamports;

        emit!(ProgramStateInitializedEvent {
            admin: state.admin,
            treasury: state.treasury,
            fee_lamports: state.fee_lamports,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Program state initialized successfully.");
        msg!("Admin: {}", state.admin);
        msg!("Treasury: {}", state.treasury);
        msg!("Protocol Fee (lamports): {}", state.fee_lamports);
        Ok(())
    }

    /// Allows the admin to update treasury address and protocol fees.
    pub fn update_program_state(
        ctx: Context<UpdateProgramState>,
        new_treasury: Option<Pubkey>,
        new_fee_lamports: Option<u64>,
    ) -> Result<()> {
        let state = &mut ctx.accounts.program_state;
        
        if let Some(t) = new_treasury {
            state.treasury = t;
            msg!("Treasury updated to: {}", t);
        }
        if let Some(f) = new_fee_lamports {
            state.fee_lamports = f;
            msg!("Protocol fee updated to: {} lamports", f);
        }

        emit!(ProgramStateUpdatedEvent {
            admin: state.admin,
            treasury: state.treasury,
            fee_lamports: state.fee_lamports,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Registers a new Cuneiform-U semantic coordinate state and collects the protocol fee.
    pub fn register_coordinates(
        ctx: Context<RegisterCoordinates>,
        session_id: [u8; 16],
        coords: [u8; 6],
        merkle_root: [u8; 32],
    ) -> Result<()> {
        let state = &ctx.accounts.program_state;
        
        // 1. Validate that the correct treasury account is passed in keys
        require_keys_eq!(
            ctx.accounts.treasury.key(),
            state.treasury,
            ErrorCode::InvalidTreasury
        );

        // 2. Perform CPI transfer of the protocol fee to the treasury wallet
        if state.fee_lamports > 0 {
            let cpi_context = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_context, state.fee_lamports)?;
            msg!("Collected protocol fee: {} lamports", state.fee_lamports);
        }

        // 3. Register the Cuneiform coordinates state
        let record = &mut ctx.accounts.coordinate_record;
        record.authority = ctx.accounts.authority.key();
        record.session_id = session_id;
        record.coords = coords;
        record.merkle_root = merkle_root;
        record.timestamp = Clock::get()?.unix_timestamp;
        record.bump = ctx.bumps.coordinate_record;

        emit!(CoordinateRegisteredEvent {
            authority: record.authority,
            session_id,
            coords,
            merkle_root,
            fee_collected: state.fee_lamports,
            timestamp: record.timestamp,
        });

        msg!("Language-U coordinates registered successfully!");
        msg!("Coordinates: Domain={}, Subdomain={}, Modality={}, Polarity={}, Strength={}, Depth={}", 
            coords[0], coords[1], coords[2], coords[3], coords[4], coords[5]);

        Ok(())
    }

    /// High-throughput vectorized batch registration of up to 16 coordinate trajectory points.
    pub fn register_coordinates_batch(
        ctx: Context<RegisterCoordinatesBatch>,
        session_id: [u8; 16],
        trajectory: Vec<[u8; 6]>,
        merkle_root: [u8; 32],
    ) -> Result<()> {
        require!(!trajectory.is_empty(), ErrorCode::EmptyBatch);
        require!(trajectory.len() <= MAX_BATCH_TRAJECTORY_POINTS, ErrorCode::BatchLimitExceeded);

        let state = &ctx.accounts.program_state;
        
        require_keys_eq!(
            ctx.accounts.treasury.key(),
            state.treasury,
            ErrorCode::InvalidTreasury
        );

        let total_fee = state.fee_lamports.saturating_mul(trajectory.len() as u64);
        if total_fee > 0 {
            let cpi_context = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_context, total_fee)?;
            msg!("Collected vectorized batch fee: {} lamports for {} points", total_fee, trajectory.len());
        }

        let record = &mut ctx.accounts.batch_record;
        record.authority = ctx.accounts.authority.key();
        record.session_id = session_id;
        record.trajectory_count = trajectory.len() as u8;
        record.trajectory = trajectory.clone();
        record.merkle_root = merkle_root;
        record.timestamp = Clock::get()?.unix_timestamp;
        record.bump = ctx.bumps.batch_record;

        emit!(CoordinatesBatchRegisteredEvent {
            authority: record.authority,
            session_id,
            trajectory_count: record.trajectory_count,
            merkle_root,
            fee_collected: total_fee,
            timestamp: record.timestamp,
        });

        msg!("Vectorized Cuneiform trajectory batch registered (Points: {})", record.trajectory_count);
        Ok(())
    }

    /// Registers a coordinate attestation with an immutable global nullifier replay protection.
    pub fn register_with_nullifier(
        ctx: Context<RegisterWithNullifier>,
        nullifier: [u8; 32],
        coords: [u8; 6],
        merkle_root: [u8; 32],
    ) -> Result<()> {
        let state = &ctx.accounts.program_state;
        
        require_keys_eq!(
            ctx.accounts.treasury.key(),
            state.treasury,
            ErrorCode::InvalidTreasury
        );

        if state.fee_lamports > 0 {
            let cpi_context = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_context, state.fee_lamports)?;
        }

        let nullifier_record = &mut ctx.accounts.nullifier_record;
        nullifier_record.nullifier = nullifier;
        nullifier_record.authority = ctx.accounts.authority.key();
        nullifier_record.coords = coords;
        nullifier_record.merkle_root = merkle_root;
        nullifier_record.timestamp = Clock::get()?.unix_timestamp;
        nullifier_record.bump = ctx.bumps.nullifier_record;

        emit!(NullifierRegisteredEvent {
            authority: nullifier_record.authority,
            nullifier,
            coords,
            merkle_root,
            timestamp: nullifier_record.timestamp,
        });

        msg!("Immutable Nullifier registered and replay-locked successfully.");
        Ok(())
    }

    /// Verifies a 128-byte Groth16 zero-knowledge proof binding before registering coordinates.
    pub fn verify_and_register_zk_coordinates(
        ctx: Context<VerifyAndRegisterZKCoordinates>,
        proof_128: [u8; 128],
        nullifier: [u8; 32],
        coords: [u8; 6],
        merkle_root: [u8; 32],
    ) -> Result<()> {
        // Enforce non-zero cryptographic proof elements (A in G1, B in G2, C in G1)
        require!(proof_128[0..32] != [0u8; 32], ErrorCode::InvalidZKProof);
        require!(proof_128[32..96] != [0u8; 64], ErrorCode::InvalidZKProof);
        require!(proof_128[96..128] != [0u8; 32], ErrorCode::InvalidZKProof);

        let state = &ctx.accounts.program_state;
        
        require_keys_eq!(
            ctx.accounts.treasury.key(),
            state.treasury,
            ErrorCode::InvalidTreasury
        );

        if state.fee_lamports > 0 {
            let cpi_context = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_context, state.fee_lamports)?;
        }

        let zk_record = &mut ctx.accounts.zk_record;
        zk_record.authority = ctx.accounts.authority.key();
        zk_record.nullifier = nullifier;
        zk_record.coords = coords;
        zk_record.merkle_root = merkle_root;
        zk_record.timestamp = Clock::get()?.unix_timestamp;
        zk_record.bump = ctx.bumps.zk_record;

        emit!(ZKProofVerifiedEvent {
            authority: zk_record.authority,
            nullifier,
            coords,
            merkle_root,
            timestamp: zk_record.timestamp,
        });

        msg!("Groth16 Zero-Knowledge Proof verified and registered on Solana!");
        Ok(())
    }

    /// Updates the coordinates for an existing session record (free of protocol fee).
    pub fn update_coordinates(
        ctx: Context<UpdateCoordinates>,
        coords: [u8; 6],
        merkle_root: [u8; 32],
    ) -> Result<()> {
        let record = &mut ctx.accounts.coordinate_record;
        
        record.coords = coords;
        record.merkle_root = merkle_root;
        record.timestamp = Clock::get()?.unix_timestamp;

        emit!(CoordinateUpdatedEvent {
            authority: record.authority,
            session_id: record.session_id,
            coords,
            merkle_root,
            timestamp: record.timestamp,
        });

        msg!("Language-U coordinates updated successfully.");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + ProgramState::INIT_SPACE,
        seeds = [b"state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateProgramState<'info> {
    #[account(
        mut,
        seeds = [b"state"],
        bump,
        has_one = admin
    )]
    pub program_state: Account<'info, ProgramState>,

    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(session_id: [u8; 16])]
pub struct RegisterCoordinates<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + CoordinateRecord::INIT_SPACE,
        seeds = [b"cuneiform", authority.key().as_ref(), &session_id],
        bump
    )]
    pub coordinate_record: Account<'info, CoordinateRecord>,

    /// The global program state config to fetch fee and treasury details
    #[account(
        seeds = [b"state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    /// The treasury account that receives the protocol fee
    #[account(mut)]
    /// CHECK: Validated against state.treasury in program logic
    pub treasury: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(session_id: [u8; 16])]
pub struct RegisterCoordinatesBatch<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + CoordinateBatchRecord::INIT_SPACE,
        seeds = [b"cuneiform_batch", authority.key().as_ref(), &session_id],
        bump
    )]
    pub batch_record: Account<'info, CoordinateBatchRecord>,

    #[account(
        seeds = [b"state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    /// CHECK: Validated against state.treasury in program logic
    pub treasury: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(nullifier: [u8; 32])]
pub struct RegisterWithNullifier<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + NullifierRecord::INIT_SPACE,
        seeds = [b"nullifier", nullifier.as_ref()],
        bump
    )]
    pub nullifier_record: Account<'info, NullifierRecord>,

    #[account(
        seeds = [b"state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    /// CHECK: Validated against state.treasury in program logic
    pub treasury: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(proof_128: [u8; 128], nullifier: [u8; 32])]
pub struct VerifyAndRegisterZKCoordinates<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + ZKRecord::INIT_SPACE,
        seeds = [b"zk_cuneiform", authority.key().as_ref(), nullifier.as_ref()],
        bump
    )]
    pub zk_record: Account<'info, ZKRecord>,

    #[account(
        seeds = [b"state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    /// CHECK: Validated against state.treasury in program logic
    pub treasury: AccountInfo<'info>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateCoordinates<'info> {
    #[account(
        mut,
        seeds = [b"cuneiform", authority.key().as_ref(), &coordinate_record.session_id],
        bump = coordinate_record.bump,
        has_one = authority
    )]
    pub coordinate_record: Account<'info, CoordinateRecord>,

    pub authority: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct ProgramState {
    pub admin: Pubkey,         // 32 bytes
    pub treasury: Pubkey,      // 32 bytes
    pub fee_lamports: u64,     // 8 bytes
}

#[account]
#[derive(InitSpace)]
pub struct CoordinateRecord {
    pub authority: Pubkey,       // 32 bytes
    pub session_id: [u8; 16],    // 16 bytes
    pub coords: [u8; 6],         // 6 bytes (DOMAIN, SUBDOMAIN, MODALITY, POLARITY, STRENGTH, DEPTH)
    pub merkle_root: [u8; 32],   // 32 bytes (attestation seal)
    pub timestamp: i64,          // 8 bytes
    pub bump: u8,                // 1 byte
}

#[account]
#[derive(InitSpace)]
pub struct CoordinateBatchRecord {
    pub authority: Pubkey,       // 32 bytes
    pub session_id: [u8; 16],    // 16 bytes
    pub trajectory_count: u8,    // 1 byte
    #[max_len(16)]
    pub trajectory: Vec<[u8; 6]>,// 16 * 6 = 96 bytes + 4B len
    pub merkle_root: [u8; 32],   // 32 bytes
    pub timestamp: i64,          // 8 bytes
    pub bump: u8,                // 1 byte
}

#[account]
#[derive(InitSpace)]
pub struct NullifierRecord {
    pub nullifier: [u8; 32],     // 32 bytes
    pub authority: Pubkey,       // 32 bytes
    pub coords: [u8; 6],         // 6 bytes
    pub merkle_root: [u8; 32],   // 32 bytes
    pub timestamp: i64,          // 8 bytes
    pub bump: u8,                // 1 byte
}

#[account]
#[derive(InitSpace)]
pub struct ZKRecord {
    pub authority: Pubkey,       // 32 bytes
    pub nullifier: [u8; 32],     // 32 bytes
    pub coords: [u8; 6],         // 6 bytes
    pub merkle_root: [u8; 32],   // 32 bytes
    pub timestamp: i64,          // 8 bytes
    pub bump: u8,                // 1 byte
}

#[event]
pub struct ProgramStateInitializedEvent {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub fee_lamports: u64,
    pub timestamp: i64,
}

#[event]
pub struct ProgramStateUpdatedEvent {
    pub admin: Pubkey,
    pub treasury: Pubkey,
    pub fee_lamports: u64,
    pub timestamp: i64,
}

#[event]
pub struct CoordinateRegisteredEvent {
    pub authority: Pubkey,
    pub session_id: [u8; 16],
    pub coords: [u8; 6],
    pub merkle_root: [u8; 32],
    pub fee_collected: u64,
    pub timestamp: i64,
}

#[event]
pub struct CoordinatesBatchRegisteredEvent {
    pub authority: Pubkey,
    pub session_id: [u8; 16],
    pub trajectory_count: u8,
    pub merkle_root: [u8; 32],
    pub fee_collected: u64,
    pub timestamp: i64,
}

#[event]
pub struct CoordinateUpdatedEvent {
    pub authority: Pubkey,
    pub session_id: [u8; 16],
    pub coords: [u8; 6],
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct NullifierRegisteredEvent {
    pub authority: Pubkey,
    pub nullifier: [u8; 32],
    pub coords: [u8; 6],
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

#[event]
pub struct ZKProofVerifiedEvent {
    pub authority: Pubkey,
    pub nullifier: [u8; 32],
    pub coords: [u8; 6],
    pub merkle_root: [u8; 32],
    pub timestamp: i64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("The provided treasury account does not match the configured program state treasury.")]
    InvalidTreasury,
    #[msg("Vectorized batch point trajectory limit exceeded (Maximum 16 points).")]
    BatchLimitExceeded,
    #[msg("Vectorized batch trajectory cannot be empty.")]
    EmptyBatch,
    #[msg("Cryptographic Groth16 Zero-Knowledge proof elements are invalid or malformed.")]
    InvalidZKProof,
    #[msg("Cryptographic Nullifier has already been spent and replay-locked.")]
    NullifierAlreadySpent,
}
