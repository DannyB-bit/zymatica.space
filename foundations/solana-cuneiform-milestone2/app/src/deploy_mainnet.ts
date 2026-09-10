/**
 * Zymatica Solana Cuneiform Anchor — Mainnet Beta Deployment & Initialization
 * 
 * Milestone 3 Rollout Orchestrator:
 * 1. Establishes a connection to Solana Mainnet Beta.
 * 2. Validates that the deployer has the required rent balance (~1.6 SOL).
 * 3. Initializes the global program state with the cold wallet treasury and protocol fees.
 */

import {
  Connection,
  PublicKey,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { CuneiformClient } from "./cuneiform_client.js";
import * as fs from "fs";
import * as path from "path";

const MAINNET_RPC_URL = "https://api.mainnet-beta.solana.com";

// Solana Program ID on Mainnet Beta (same as keypair)
const PROGRAM_ID = new PublicKey("BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M");

// Cold Wallet Treasury Address
const TREASURY_ADDRESS = new PublicKey("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS");

// Protocol fee: 100,000 lamports (~$0.015 USD)
const PROTOCOL_FEE_LAMPORTS = 100000;

function loadKeypair(filepath: string): Keypair {
  const resolved = filepath.replace("~", process.env.HOME || process.env.USERPROFILE || "");
  const raw = JSON.parse(fs.readFileSync(resolved, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function log(emoji: string, msg: string) {
  console.log(`${emoji}  ${msg}`);
}

async function main() {
  console.log("=".repeat(70));
  console.log("ZYMATICA | Solana Cuneiform — Mainnet Beta Rollout");
  console.log("=".repeat(70));

  // 1. Load Deployer keypair
  const walletPath = path.join(
    process.env.USERPROFILE || process.env.HOME || "",
    ".config", "solana", "id.json"
  );
  if (!fs.existsSync(walletPath)) {
    throw new Error(`Deployer wallet not found at: ${walletPath}`);
  }
  
  const deployer = loadKeypair(walletPath);
  log("🔑", `Deployer Address: ${deployer.publicKey.toBase58()}`);

  const connection = new Connection(MAINNET_RPC_URL, "confirmed");
  const client = new CuneiformClient(MAINNET_RPC_URL, PROGRAM_ID.toBase58());

  // 2. Validate balance
  const balance = await connection.getBalance(deployer.publicKey);
  log("💰", `Deployer Balance: ${balance / LAMPORTS_PER_SOL} SOL`);

  const minRequired = 1.6; // Rent for program deployment + state PDA initialization
  if (balance < minRequired * LAMPORTS_PER_SOL) {
    log("⚠️", `WARNING: Deployer balance is low. Need at least ${minRequired} SOL for mainnet deployment and account rent.`);
  }

  log("📡", `Treasury Wallet Address: ${TREASURY_ADDRESS.toBase58()}`);
  log("💵", `Protocol Fee: ${PROTOCOL_FEE_LAMPORTS} lamports`);

  // Derive global state PDA
  const [statePda] = client.deriveStateAddress();
  log("🔗", `Derived Program State PDA: ${statePda.toBase58()}`);

  // 3. Confirm mainnet deployment action
  console.log();
  console.log("----------------- MAINNET TRANSACTION NOTICE -----------------");
  console.log("To deploy and initialize the program state on Solana Mainnet:");
  console.log("1. Ensure your program binary is built: cargo build-sbf");
  console.log("2. Deploy the binary: solana program deploy target/deploy/solana_cuneiform_anchor.so --url mainnet-beta");
  console.log("3. Run this script to initialize state PDA.");
  console.log("--------------------------------------------------------------");
  console.log();

  log("⏳", "Checking if program is already initialized on Mainnet...");
  try {
    const state = await client.fetchProgramState();
    if (state) {
      log("✅", "Program already initialized!");
      log("👑", `Admin: ${state.admin.toBase58()}`);
      log("🏦", `Treasury: ${state.treasury.toBase58()}`);
      log("💸", `Fee: ${state.feeLamports} lamports`);
      return;
    }
  } catch (err) {
    log("⚠️", "State PDA not found. Proceeding with initialization...");
  }

  log("🚀", "Initializing Program State PDA on Mainnet Beta...");
  try {
    const txSig = await client.initializeProgram(
      deployer,
      TREASURY_ADDRESS,
      PROTOCOL_FEE_LAMPORTS
    );
    log("✅", `Initialization succeeded!`);
    log("📝", `Tx Signature: ${txSig}`);
    log("🔗", `Explorer: https://explorer.solana.com/tx/${txSig}`);
  } catch (err: any) {
    console.error("  ❌ FAIL: State initialization failed:", err.message);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
