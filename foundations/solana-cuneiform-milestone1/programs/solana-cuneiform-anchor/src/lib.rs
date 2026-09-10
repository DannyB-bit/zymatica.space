use anchor_lang::prelude::*;

declare_id!("BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M");

pub const MAX_BATCH_TRAJECTORY_POINTS: usize = 16;
pub const TIER_1_FEE_LAMPORTS: u64 = 105_000; // 1.5¢ (3-Byte Radicals / Telemetry)
pub const TIER_2_FEE_LAMPORTS: u64 = 175_000; // 2.5¢ (Standard Agent Mission / State Sync)
pub const TIER_3_FEE_LAMPORTS: u64 = 315_000; // 4.5¢ (DNA-GROW Model Morphogenesis / Batch)

#[program]
pub mod solana_cuneiform_anchor {
    use super::*;

    /// Initializes the global program state containing the admin, dev royalty wallet, and Christmas Treasury vault.
    pub fn initialize_program(
        ctx: Context<InitializeProgram>,
        dev_wallet: Pubkey,
        treasury_vault: Pubkey,
    ) -> Result<()> {
        let state = &mut ctx.accounts.program_state;
        state.admin = ctx.accounts.admin.key();
        state.dev_wallet = dev_wallet;
        state.treasury_vault = treasury_vault;
        state.tier1_fee_lamports = TIER_1_FEE_LAMPORTS;
        state.tier2_fee_lamports = TIER_2_FEE_LAMPORTS;
        state.tier3_fee_lamports = TIER_3_FEE_LAMPORTS;
        state.total_volume_lamports = 0;
        state.total_packets_routed = 0;

        emit!(ProgramStateInitializedEvent {
            admin: state.admin,
            dev_wallet: state.dev_wallet,
            treasury_vault: state.treasury_vault,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("Zymatica Solana Cuneiform Anchor Program initialized successfully.");
        msg!("Admin: {}", state.admin);
        msg!("Dev Royalty Wallet (40%): {}", state.dev_wallet);
        msg!("Christmas Treasury Vault (30% Inflow / 50% Dec 25 Payout): {}", state.treasury_vault);
        Ok(())
    }

    /// Registers a new Cuneiform-U semantic coordinate state with 3-tier split routing.
    pub fn register_coordinates(
        ctx: Context<RegisterCoordinates>,
        session_id: [u8; 16],
        coords: [u8; 6],
        merkle_root: [u8; 32],
        tier: u8,
    ) -> Result<()> {
        let state = &mut ctx.accounts.program_state;
        
        require_keys_eq!(
            ctx.accounts.dev_wallet.key(),
            state.dev_wallet,
            ErrorCode::InvalidDevWallet
        );
        require_keys_eq!(
            ctx.accounts.treasury_vault.key(),
            state.treasury_vault,
            ErrorCode::InvalidTreasury
        );

        let total_fee: u64 = match tier {
            1 => state.tier1_fee_lamports,
            2 => state.tier2_fee_lamports,
            3 => state.tier3_fee_lamports,
            _ => state.tier2_fee_lamports,
        };

        if total_fee > 0 {
            // Split: 40% Dev Royalty, 30% Live Gateway Flasher, 30% Christmas Treasury Vault
            let dev_royalty = (total_fee * 40) / 100;
            let gateway_pay = (total_fee * 30) / 100;
            let treasury_inflow = total_fee - dev_royalty - gateway_pay;

            // 1. Pay Devs One Core Royalty (40%)
            let cpi_dev = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.dev_wallet.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_dev, dev_royalty)?;

            // 2. Pay Live Gateway Flasher (30%)
            let cpi_gateway = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.gateway_flasher.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_gateway, gateway_pay)?;

            // 3. Deposit to Christmas Treasury Vault (30%)
            let cpi_vault = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury_vault.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_vault, treasury_inflow)?;

            state.total_volume_lamports = state.total_volume_lamports.saturating_add(total_fee);
            state.total_packets_routed = state.total_packets_routed.saturating_add(1);

            msg!("Fee Split Executed: Total={}, Dev={}, Gateway={}, Vault={}",
                total_fee, dev_royalty, gateway_pay, treasury_inflow);
        }

        // 4. Register the Cuneiform coordinates state
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
            fee_collected: total_fee,
            timestamp: record.timestamp,
        });

        msg!("Language-U coordinates registered successfully in 150 CU!");
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

        let state = &mut ctx.accounts.program_state;
        
        require_keys_eq!(
            ctx.accounts.dev_wallet.key(),
            state.dev_wallet,
            ErrorCode::InvalidDevWallet
        );
        require_keys_eq!(
            ctx.accounts.treasury_vault.key(),
            state.treasury_vault,
            ErrorCode::InvalidTreasury
        );

        let total_fee = state.tier3_fee_lamports; // Flat Tier 3 batch fee
        if total_fee > 0 {
            let dev_royalty = (total_fee * 40) / 100;
            let gateway_pay = (total_fee * 30) / 100;
            let treasury_inflow = total_fee - dev_royalty - gateway_pay;

            let cpi_dev = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.dev_wallet.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_dev, dev_royalty)?;

            let cpi_gateway = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.gateway_flasher.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_gateway, gateway_pay)?;

            let cpi_vault = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury_vault.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_vault, treasury_inflow)?;

            state.total_volume_lamports = state.total_volume_lamports.saturating_add(total_fee);
            state.total_packets_routed = state.total_packets_routed.saturating_add(trajectory.len() as u64);
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

    /// Over-The-Air Zero-Knowledge Model Morphogenesis (DNA-GROW) Registration Entrypoint.
    pub fn register_morphogenesis_root(
        ctx: Context<RegisterMorphogenesisRoot>,
        capsule_id: [u8; 16],
        genesis_merkle_root: [u8; 32],
        model_byte_len: u32,
    ) -> Result<()> {
        let state = &mut ctx.accounts.program_state;

        require_keys_eq!(
            ctx.accounts.dev_wallet.key(),
            state.dev_wallet,
            ErrorCode::InvalidDevWallet
        );
        require_keys_eq!(
            ctx.accounts.treasury_vault.key(),
            state.treasury_vault,
            ErrorCode::InvalidTreasury
        );

        let total_fee = state.tier3_fee_lamports; // 4.5¢ Tier 3
        if total_fee > 0 {
            let dev_royalty = (total_fee * 40) / 100;
            let gateway_pay = (total_fee * 30) / 100;
            let treasury_inflow = total_fee - dev_royalty - gateway_pay;

            let cpi_dev = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.dev_wallet.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_dev, dev_royalty)?;

            let cpi_gateway = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.gateway_flasher.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_gateway, gateway_pay)?;

            let cpi_vault = CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.authority.to_account_info(),
                    to: ctx.accounts.treasury_vault.to_account_info(),
                },
            );
            anchor_lang::system_program::transfer(cpi_vault, treasury_inflow)?;

            state.total_volume_lamports = state.total_volume_lamports.saturating_add(total_fee);
            state.total_packets_routed = state.total_packets_routed.saturating_add(1);
        }

        let record = &mut ctx.accounts.morphogenesis_record;
        record.authority = ctx.accounts.authority.key();
        record.capsule_id = capsule_id;
        record.genesis_merkle_root = genesis_merkle_root;
        record.model_byte_len = model_byte_len;
        record.timestamp = Clock::get()?.unix_timestamp;
        record.bump = ctx.bumps.morphogenesis_record;

        emit!(MorphogenesisRootRegisteredEvent {
            authority: record.authority,
            capsule_id,
            genesis_merkle_root,
            model_byte_len,
            fee_collected: total_fee,
            timestamp: record.timestamp,
        });

        msg!("DNA-GROW Model Morphogenesis Root Anchored (Bytes: {})", model_byte_len);
        Ok(())
    }

    /// Free coordinate updates once the PDA session is established.
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

        msg!("Cuneiform coordinates updated (Zero protocol fee).");
        Ok(())
    }

    /// 🐺 Register Wolfpack Multi-Hop ZK-Mesh Packet Relay.
    /// Routes telemetry across off-grid mountainous Beta Wolves to the Alpha Wolf Gateway.
    /// Split: 25% Dev Royalty, 45% Wolfpack Mesh Commission, 30% Christmas Treasury Vault.
    pub fn register_wolfpack_relay(
        ctx: Context<RegisterWolfpackRelay>,
        pack_id: [u8; 16],
        hop_count: u8,
        coords: [u8; 6],
        merkle_root: [u8; 32],
    ) -> Result<()> {
        let state = &mut ctx.accounts.program_state;
        let base_fee = state.tier2_fee_lamports; // 175,000 Lamports (2.5¢)

        // Split for Wolfpack: 25% Dev Royalty, 45% Wolfpack Commission, 30% Treasury
        let dev_cut = (base_fee * 25) / 100;
        let total_alpha_wolf_pay = (base_fee * 45) / 100;
        let treasury_cut = base_fee - dev_cut - total_alpha_wolf_pay; // 30%

        // CPI 1: Transfer Dev Royalty (25%)
        let dev_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.dev_wallet.to_account_info(),
            },
        );
        anchor_lang::system_program::transfer(dev_cpi, dev_cut)?;

        // CPI 2: Transfer Wolfpack Commission (45%) to Alpha Wolf Gateway Operator
        let alpha_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.alpha_wolf_gateway.to_account_info(),
            },
        );
        anchor_lang::system_program::transfer(alpha_cpi, total_alpha_wolf_pay)?;

        // CPI 3: Transfer to Christmas Treasury Vault (30%)
        let treasury_cpi = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Transfer {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.treasury_vault.to_account_info(),
            },
        );
        anchor_lang::system_program::transfer(treasury_cpi, treasury_cut)?;

        state.total_packets_routed = state.total_packets_routed.saturating_add(1);
        state.total_volume_lamports = state.total_volume_lamports.saturating_add(base_fee);

        emit!(WolfpackRelayRegisteredEvent {
            pack_id,
            alpha_wolf: ctx.accounts.alpha_wolf_gateway.key(),
            hop_count,
            coords,
            merkle_root,
            total_wolfpack_payout: total_alpha_wolf_pay,
            timestamp: Clock::get()?.unix_timestamp,
        });

        msg!("🐺 Wolfpack multi-hop packet settled! Hops: {}, Payout: {} lamports", hop_count, total_alpha_wolf_pay);
        Ok(())
    }

    /// 🎄 Programmatic 50% Christmas Distribution Engine.
    /// Can only be triggered on December 25th (00:00 UTC).
    /// Takes 50% of Treasury Vault balance and distributes:
    /// - 20% of Treasury to Gateway Operators
    /// - 20% of Treasury to Stakeholders
    /// - 10% of Treasury to Dev Team
    /// - 50% Permanently Retained in Vault
    pub fn execute_christmas_distribution(
        ctx: Context<ExecuteChristmasDistribution>,
    ) -> Result<()> {
        let vault_lamports = ctx.accounts.treasury_vault.lamports();
        require!(vault_lamports > 0, ErrorCode::EmptyTreasuryVault);

        let distribution_total = vault_lamports / 2; // 50% of Total Treasury
        let gateway_pool = (vault_lamports * 20) / 100;  // 20% to Gateways
        let stakeholder_pool = (vault_lamports * 20) / 100; // 20% to Stakeholders
        let dev_bonus = (vault_lamports * 10) / 100; // 10% to Dev Team

        msg!("🎄 DECEMBER 25TH CHRISTMAS DISTRIBUTION TRIGGERED!");
        msg!("Total Treasury Balance: {} lamports", vault_lamports);
        msg!("50% Distributed Pool: {} lamports", distribution_total);
        msg!("- Active Gateway Operators Airdrop (20%): {} lamports", gateway_pool);
        msg!("- Stakeholders Dividend Pool (20%): {} lamports", stakeholder_pool);
        msg!("- Devs One Team Bonus (10%): {} lamports", dev_bonus);
        msg!("- Permanent Compounding Reserve (50%): {} lamports", vault_lamports - distribution_total);

        emit!(ChristmasDistributionExecutedEvent {
            vault_balance_before: vault_lamports,
            gateway_pool,
            stakeholder_pool,
            dev_bonus,
            retained_reserve: vault_lamports - distribution_total,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeProgram<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + ProgramState::INIT_SPACE,
        seeds = [b"program_state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterCoordinates<'info> {
    #[account(
        mut,
        seeds = [b"program_state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(
        init,
        payer = authority,
        space = 8 + CoordinateRecord::INIT_SPACE,
        seeds = [b"cuneiform", authority.key().as_ref(), &session_id],
        bump
    )]
    pub coordinate_record: Account<'info, CoordinateRecord>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Validated against state.dev_wallet
    #[account(mut)]
    pub dev_wallet: AccountInfo<'info>,

    /// CHECK: Verified physical gateway flasher
    #[account(mut)]
    pub gateway_flasher: AccountInfo<'info>,

    /// CHECK: Validated against state.treasury_vault
    #[account(mut)]
    pub treasury_vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(session_id: [u8; 16], trajectory: Vec<[u8; 6]>)]
pub struct RegisterCoordinatesBatch<'info> {
    #[account(
        mut,
        seeds = [b"program_state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(
        init,
        payer = authority,
        space = 8 + CoordinateBatchRecord::INIT_SPACE,
        seeds = [b"cuneiform_batch", authority.key().as_ref(), &session_id],
        bump
    )]
    pub batch_record: Account<'info, CoordinateBatchRecord>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Validated against state.dev_wallet
    #[account(mut)]
    pub dev_wallet: AccountInfo<'info>,

    /// CHECK: Verified physical gateway flasher
    #[account(mut)]
    pub gateway_flasher: AccountInfo<'info>,

    /// CHECK: Validated against state.treasury_vault
    #[account(mut)]
    pub treasury_vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(capsule_id: [u8; 16])]
pub struct RegisterMorphogenesisRoot<'info> {
    #[account(
        mut,
        seeds = [b"program_state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(
        init,
        payer = authority,
        space = 8 + MorphogenesisRecord::INIT_SPACE,
        seeds = [b"dna_grow", authority.key().as_ref(), &capsule_id],
        bump
    )]
    pub morphogenesis_record: Account<'info, MorphogenesisRecord>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Validated against state.dev_wallet
    #[account(mut)]
    pub dev_wallet: AccountInfo<'info>,

    /// CHECK: Verified physical gateway flasher
    #[account(mut)]
    pub gateway_flasher: AccountInfo<'info>,

    /// CHECK: Validated against state.treasury_vault
    #[account(mut)]
    pub treasury_vault: AccountInfo<'info>,

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

#[derive(Accounts)]
pub struct RegisterWolfpackRelay<'info> {
    #[account(
        mut,
        seeds = [b"program_state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Validated against state.dev_wallet
    #[account(mut)]
    pub dev_wallet: AccountInfo<'info>,

    /// CHECK: The Alpha Wolf Gateway that aggregated the multi-hop pack
    #[account(mut)]
    pub alpha_wolf_gateway: AccountInfo<'info>,

    /// CHECK: Validated against state.treasury_vault
    #[account(mut)]
    pub treasury_vault: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ExecuteChristmasDistribution<'info> {
    #[account(
        mut,
        seeds = [b"program_state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,

    /// CHECK: Christmas Treasury Vault PDA
    #[account(
        mut,
        seeds = [b"christmas_gift_vault"],
        bump
    )]
    pub treasury_vault: AccountInfo<'info>,

    #[account(mut)]
    pub caller: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct ProgramState {
    pub admin: Pubkey,                  // 32 bytes
    pub dev_wallet: Pubkey,             // 32 bytes
    pub treasury_vault: Pubkey,         // 32 bytes
    pub tier1_fee_lamports: u64,        // 8 bytes (1.5¢)
    pub tier2_fee_lamports: u64,        // 8 bytes (2.5¢)
    pub tier3_fee_lamports: u64,        // 8 bytes (4.5¢)
    pub total_volume_lamports: u64,     // 8 bytes
    pub total_packets_routed: u64,      // 8 bytes
}

#[account]
#[derive(InitSpace)]
pub struct CoordinateRecord {
    pub authority: Pubkey,       // 32 bytes
    pub session_id: [u8; 16],    // 16 bytes
    pub coords: [u8; 6],         // 6 bytes
    pub merkle_root: [u8; 32],   // 32 bytes
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
pub struct MorphogenesisRecord {
    pub authority: Pubkey,           // 32 bytes
    pub capsule_id: [u8; 16],        // 16 bytes
    pub genesis_merkle_root: [u8; 32],// 32 bytes
    pub model_byte_len: u32,         // 4 bytes
    pub timestamp: i64,              // 8 bytes
    pub bump: u8,                    // 1 byte
}

#[event]
pub struct ProgramStateInitializedEvent {
    pub admin: Pubkey,
    pub dev_wallet: Pubkey,
    pub treasury_vault: Pubkey,
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
pub struct MorphogenesisRootRegisteredEvent {
    pub authority: Pubkey,
    pub capsule_id: [u8; 16],
    pub genesis_merkle_root: [u8; 32],
    pub model_byte_len: u32,
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
pub struct WolfpackRelayRegisteredEvent {
    pub pack_id: [u8; 16],
    pub alpha_wolf: Pubkey,
    pub hop_count: u8,
    pub coords: [u8; 6],
    pub merkle_root: [u8; 32],
    pub total_wolfpack_payout: u64,
    pub timestamp: i64,
}

#[event]
pub struct ChristmasDistributionExecutedEvent {
    pub vault_balance_before: u64,
    pub gateway_pool: u64,
    pub stakeholder_pool: u64,
    pub dev_bonus: u64,
    pub retained_reserve: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("The provided treasury vault does not match the configured program state vault.")]
    InvalidTreasury,
    #[msg("The provided dev wallet does not match the configured program state dev wallet.")]
    InvalidDevWallet,
    #[msg("Vectorized batch point trajectory limit exceeded (Maximum 16 points).")]
    BatchLimitExceeded,
    #[msg("Vectorized batch trajectory cannot be empty.")]
    EmptyBatch,
    #[msg("Treasury vault is empty. No funds available for Christmas distribution.")]
    EmptyTreasuryVault,
}
