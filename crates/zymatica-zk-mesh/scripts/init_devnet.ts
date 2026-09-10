#!/usr/bin/env ts-node

/**
 * ZK-LoRaWAN Devnet Initialization Script
 *
 * Initializes the protocol registry and shielded pool accounts on Solana devnet.
 * Run once after deploying the program to devnet.
 *
 * Usage:
 *   npx ts-node scripts/init_devnet.ts
 *
 * Prerequisites:
 *   - Solana CLI configured for devnet: `solana config set --url https://api.devnet.solana.com`
 *   - Admin keypair at ~/.config/solana/id.json (or set ANCHOR_WALLET env var)
 *   - Program deployed to devnet
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";

const REGISTRY_SEED = Buffer.from("zk-lorawan-registry");
const SHIELDED_POOL_SEED = Buffer.from("shielded-pool");

// Match the program ID from lib.rs (non-integration-test)
const PROGRAM_ID = new PublicKey("4HRP2eV8qtYW54ozQmnGDjF7emwb8MvqFcF89UgSM6iC");
// Admin authority from lib.rs
const ADMIN_AUTHORITY = new PublicKey("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS");

async function main() {
  console.log("╔══════════════════════════════════════════════════╗");
  console.log("║  ZK-LoRaWAN Devnet Initialization               ║");
  console.log("╚══════════════════════════════════════════════════╝");
  console.log();

  // Setup provider
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const wallet = provider.wallet;
  console.log(`  Wallet:    ${wallet.publicKey.toBase58()}`);
  console.log(`  Cluster:   ${provider.connection.rpcEndpoint}`);

  // Check balance
  const balance = await provider.connection.getBalance(wallet.publicKey);
  console.log(`  Balance:   ${balance / LAMPORTS_PER_SOL} SOL`);

  if (balance < 0.1 * LAMPORTS_PER_SOL) {
    console.log("\n  ⚠️  Low balance. Requesting airdrop...");
    const sig = await provider.connection.requestAirdrop(
      wallet.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig);
    console.log(`  ✅ Airdrop received: ${sig}`);
  }

  // Load IDL
  const idl = require("../target/idl/zk_lorawan.json");
  const program = new Program(idl, PROGRAM_ID, provider);

  // Derive PDAs
  const [registryPda, registryBump] = PublicKey.findProgramAddressSync(
    [REGISTRY_SEED],
    PROGRAM_ID
  );
  const [shieldedPoolPda, poolBump] = PublicKey.findProgramAddressSync(
    [SHIELDED_POOL_SEED],
    PROGRAM_ID
  );

  console.log(`\n  Registry PDA:      ${registryPda.toBase58()}`);
  console.log(`  Shielded Pool PDA: ${shieldedPoolPda.toBase58()}`);

  // Step 1: Initialize Registry
  console.log("\n  [1/3] Initializing Protocol Registry...");
  try {
    const registryAccount = await provider.connection.getAccountInfo(registryPda);
    if (registryAccount) {
      console.log("      ℹ️  Registry already initialized, skipping.");
    } else {
      const tx = await program.methods
        .initializeRegistry()
        .accounts({
          registry: registryPda,
          authority: wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      console.log(`      ✅ Registry initialized: ${tx}`);
    }
  } catch (e: any) {
    if (e.message?.includes("already in use")) {
      console.log("      ℹ️  Registry already exists.");
    } else {
      console.error(`      ❌ Failed: ${e.message}`);
      throw e;
    }
  }

  // Step 2: Initialize Shielded Pool with initial deposit
  console.log("\n  [2/3] Initializing Shielded Pool...");
  try {
    const poolAccount = await provider.connection.getAccountInfo(shieldedPoolPda);
    if (poolAccount) {
      console.log("      ℹ️  Shielded pool already initialized, skipping.");
    } else {
      // Initial deposit of 0.5 SOL to fund the pool
      const initialDeposit = 0.5 * LAMPORTS_PER_SOL;
      const leafHash = Buffer.alloc(32); // Zero leaf for genesis

      const tx = await program.methods
        .depositShielded(new anchor.BN(initialDeposit), Array.from(leafHash) as any)
        .accounts({
          sender: wallet.publicKey,
          shieldedPool: shieldedPoolPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      console.log(`      ✅ Shielded pool initialized with ${initialDeposit / LAMPORTS_PER_SOL} SOL: ${tx}`);
    }
  } catch (e: any) {
    if (e.message?.includes("already in use")) {
      console.log("      ℹ️  Shielded pool already exists.");
    } else {
      console.error(`      ❌ Failed: ${e.message}`);
      throw e;
    }
  }

  // Step 3: Verify state
  console.log("\n  [3/3] Verifying on-chain state...");
  try {
    const registry: any = await program.account.protocolRegistry.fetch(registryPda);
    console.log(`      Authority:        ${registry.authority.toBase58()}`);
    console.log(`      Next Batch ID:    ${registry.nextBatchId.toString()}`);
    console.log(`      Total Chirps:     ${registry.totalChirpsVerified.toString()}`);
    console.log(`      Total Fees:       ${registry.totalFeesCollected.toString()} lamports`);

    const pool: any = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda);
    console.log(`      Pool Balance:     ${pool.totalBalance.toString()} lamports`);
    console.log(`      Merkle Root:      ${Buffer.from(pool.merkleRoot).toString("hex").slice(0, 16)}...`);
    console.log(`      Last Updated:     ${new Date(pool.lastUpdated.toNumber() * 1000).toISOString()}`);
  } catch (e: any) {
    console.error(`      ❌ Verification failed: ${e.message}`);
  }

  console.log("\n  ✅ Devnet initialization complete!");
  console.log("══════════════════════════════════════════════════");
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
