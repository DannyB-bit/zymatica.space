/**
 * Zymatica Solana Pay Mesh Gateway Live Tests
 * 
 * Demonstrates and verifies the Milestone 2 integration:
 * 1. Generates a Solana Pay payment request for rewarding a packet relay.
 * 2. Simulates an edge transmitter node paying the relay node via Solana Pay.
 * 3. Scans the live Solana blockchain to confirm the payment reference and memo.
 */

import {
  Connection,
  PublicKey,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as crypto from "crypto";
import { SolanaPayMeshGateway } from "./solana_pay_mock.js";

const DEVNET_URL = "https://api.devnet.solana.com";

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
  console.log("ZYMATICA | Solana Pay Mesh Gateway — Live Devnet Tests");
  console.log("=".repeat(70));
  console.log();

  // 1. Load wallet
  const walletPath = path.join(
    process.env.USERPROFILE || process.env.HOME || "",
    ".config", "solana", "id.json"
  );
  const payer = loadKeypair(walletPath);
  const connection = new Connection(DEVNET_URL, "confirmed");
  const gateway = new SolanaPayMeshGateway(DEVNET_URL);

  log("🔑", `Payer Node: ${payer.publicKey.toBase58()}`);
  const balance = await connection.getBalance(payer.publicKey);
  log("💰", `Balance: ${balance / LAMPORTS_PER_SOL} SOL`);
  
  // Create a mock recipient node (representing the LoRa mesh gateway)
  const relayNode = Keypair.generate();
  log("📡", `Relay Node: ${relayNode.publicKey.toBase58()}`);
  console.log();

  // 2. Generate Solana Pay Request
  console.log("── Test 1: Generate Solana Pay Request ──");
  const sessionId = crypto.randomBytes(16);
  const amount = 0.0001; // Small SOL amount for mock transfer demo
  
  const paymentDetails = gateway.generatePaymentRequest(
    relayNode.publicKey,
    amount,
    sessionId
  );

  log("🔗", `Reference key generated: ${paymentDetails.reference.toBase58()}`);
  log("📝", `Memo: ${paymentDetails.memo}`);
  log("💬", `Message: ${paymentDetails.message}`);
  console.log("  ✅ PASS: Solana Pay request successfully built");
  console.log();

  // 3. Execute simulated payment
  console.log("── Test 2: Simulating Payment via Solana Pay ──");
  log("🚀", "Sending transaction with reference key to devnet...");
  
  try {
    const signature = await gateway.mockPayRelayReward(payer, paymentDetails);
    log("✅", `Transaction broadcasted successfully!`);
    log("📝", `Tx Signature: ${signature}`);
    log("🔗", `Explorer: https://explorer.solana.com/tx/${signature}?cluster=devnet`);
  } catch (err: any) {
    console.error("  ❌ FAIL: Payment simulation failed:", err.message);
    process.exit(1);
  }
  console.log();

  // 4. Verify payment on-chain (Scanning reference key)
  console.log("── Test 3: Scanning and Verifying Payment On-Chain ──");
  log("🔍", "Searching Solana ledger for reference key...");
  
  // Wait a moment to ensure indexing catches up
  await new Promise(resolve => setTimeout(resolve, 3000));

  let verified = false;
  for (let attempt = 1; attempt <= 3; attempt++) {
    log("⏳", `Scan attempt ${attempt}/3...`);
    verified = await gateway.verifyRelayReward(paymentDetails.reference);
    if (verified) break;
    await new Promise(resolve => setTimeout(resolve, 3000));
  }

  if (verified) {
    console.log();
    console.log("=".repeat(70));
    log("🎉", "ALL SOLANA PAY INTEGRATION TESTS PASSED (devnet mock — native SOL transfers)");
    console.log("=".repeat(70));
    log("🚀", "Milestone 2 Verified: Solana Pay reference-key pattern operational. USDC SPL integration pending mainnet.");
  } else {
    console.log();
    console.log("=".repeat(70));
    log("❌", "VERIFICATION FAILED");
    console.log("=".repeat(70));
  }
}

main().catch(console.error);
