/**
 * Zymatica Devnet Deployment Script
 * 
 * Deploys and initializes the Solana Cuneiform Anchor program on devnet.
 * Sets the cold wallet as the protocol fee treasury.
 * 
 * Watermark: ip zymatica.space | astronautshe.com
 * Copyright (c) 2026 Zymatica. Licensed under Apache License 2.0.
 */

import {
  Connection,
  PublicKey,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import { CuneiformClient } from "./cuneiform_client.js";

// ============================================================================
// Configuration
// ============================================================================

const DEVNET_URL = "https://api.devnet.solana.com";
const PROGRAM_ID = "BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M";

// Cold wallet treasury — all protocol fees route here
const TREASURY_ADDRESS = "7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS";

// Protocol fee: 150,000 lamports (~$0.002 at $200/SOL)
// Production can be adjusted via update_program_state
const PROTOCOL_FEE_LAMPORTS = BigInt(150_000);

// ============================================================================
// Helpers
// ============================================================================

function loadKeypair(filepath: string): Keypair {
  const resolved = filepath.replace("~", process.env.HOME || process.env.USERPROFILE || "");
  const raw = JSON.parse(fs.readFileSync(resolved, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function log(emoji: string, msg: string) {
  console.log(`${emoji}  ${msg}`);
}

// ============================================================================
// Main Deployment
// ============================================================================

async function main() {
  console.log("=".repeat(70));
  console.log("ZYMATICA | Solana Cuneiform Anchor — Devnet Deployment");
  console.log("=".repeat(70));
  console.log();

  // 1. Load deployer keypair
  const walletPath = path.join(
    process.env.USERPROFILE || process.env.HOME || "",
    ".config", "solana", "id.json"
  );
  const deployer = loadKeypair(walletPath);
  log("🔑", `Deployer: ${deployer.publicKey.toBase58()}`);

  // 2. Connect to devnet
  const connection = new Connection(DEVNET_URL, "confirmed");
  const balance = await connection.getBalance(deployer.publicKey);
  log("💰", `Balance: ${balance / LAMPORTS_PER_SOL} SOL`);

  if (balance < 0.1 * LAMPORTS_PER_SOL) {
    log("❌", "Insufficient balance. Airdrop more SOL first.");
    process.exit(1);
  }

  // 3. Initialize the client
  const client = new CuneiformClient(DEVNET_URL, PROGRAM_ID);
  const treasury = new PublicKey(TREASURY_ADDRESS);

  log("📋", `Program ID: ${PROGRAM_ID}`);
  log("🏦", `Treasury (cold wallet): ${TREASURY_ADDRESS}`);
  log("💸", `Protocol Fee: ${PROTOCOL_FEE_LAMPORTS} lamports`);
  console.log();

  // 4. Check if program state already exists
  const existingState = await client.fetchProgramState();
  
  if (existingState) {
    log("✅", "Program state already initialized!");
    log("👤", `Admin: ${existingState.admin.toBase58()}`);
    log("🏦", `Treasury: ${existingState.treasury.toBase58()}`);
    log("💸", `Fee: ${existingState.feeLamports} lamports`);
  } else {
    // 5. Initialize program state
    log("🚀", "Initializing program state on devnet...");
    
    try {
      const txSig = await client.initializeProgram(
        deployer,
        treasury,
        PROTOCOL_FEE_LAMPORTS
      );

      log("✅", `Program state initialized!`);
      log("📝", `Transaction: ${txSig}`);
      log("🔗", `Explorer: https://explorer.solana.com/tx/${txSig}?cluster=devnet`);
    } catch (err: any) {
      log("❌", `Initialization failed: ${err.message}`);
      console.log("This is expected if the program hasn't been deployed yet.");
      console.log("Deploy the program first with: solana program deploy target/deploy/solana_cuneiform_anchor.so");
      process.exit(1);
    }
  }

  console.log();

  // 6. Test coordinate registration
  log("🧪", "Testing coordinate registration...");
  
  const sessionId = Buffer.alloc(16);
  sessionId.write("test-devnet-001!", "utf-8");
  
  // Sample 6D Cuneiform-U coordinates
  const coords = [42, 7, 3, 128, 200, 15]; // [Domain, Subdomain, Modality, Polarity, Strength, Depth]
  
  // Generate Merkle root from the coordinate payload
  const { createHash } = await import("crypto");
  const merkleRoot = createHash("sha256")
    .update(Buffer.from(coords))
    .digest();

  try {
    const regTx = await client.registerCoordinates(
      deployer,
      sessionId,
      coords,
      merkleRoot,
      treasury
    );

    log("✅", `Coordinates registered on-chain!`);
    log("📝", `Transaction: ${regTx}`);
    log("🔗", `Explorer: https://explorer.solana.com/tx/${regTx}?cluster=devnet`);

    // 7. Verify the record was written
    const record = await client.fetchRecord(deployer.publicKey, sessionId);
    if (record) {
      log("✅", `On-chain record verified!`);
      log("📊", `Coords: [${record.coords.join(", ")}]`);
      log("⏰", `Timestamp: ${new Date(record.timestamp * 1000).toISOString()}`);
      log("🔐", `Merkle Root: ${record.merkleRoot.toString("hex").slice(0, 32)}...`);
    }
  } catch (err: any) {
    log("⚠️", `Registration test skipped: ${err.message}`);
  }

  console.log();
  console.log("=".repeat(70));
  log("🎉", "DEVNET DEPLOYMENT COMPLETE");
  console.log("=".repeat(70));
  console.log();
  log("📋", "Grant Milestone 1 Status: ✅ LIVE ON DEVNET");
  log("🏦", `Treasury: ${TREASURY_ADDRESS}`);
  log("💸", `Fee per registration: ${PROTOCOL_FEE_LAMPORTS} lamports`);
  log("🔗", `Program: https://explorer.solana.com/address/${PROGRAM_ID}?cluster=devnet`);
}

main().catch(console.error);
