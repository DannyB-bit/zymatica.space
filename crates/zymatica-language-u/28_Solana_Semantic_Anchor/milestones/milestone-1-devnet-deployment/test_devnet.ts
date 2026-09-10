/**
 * Zymatica Devnet Test Script
 * 
 * Full end-to-end test of the Cuneiform protocol on devnet:
 * 1. Verify program state (admin, treasury, fee)
 * 2. Register a coordinate record (with protocol fee)
 * 3. Fetch and verify the on-chain record
 * 4. Update the coordinates
 * 5. Verify the update
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
import { CuneiformClient } from "./cuneiform_client.js";

const DEVNET_URL = "https://api.devnet.solana.com";
const PROGRAM_ID = "BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M";
const TREASURY = "7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS";

function loadKeypair(filepath: string): Keypair {
  const resolved = filepath.replace("~", process.env.HOME || process.env.USERPROFILE || "");
  const raw = JSON.parse(fs.readFileSync(resolved, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function log(emoji: string, msg: string) {
  console.log(`${emoji}  ${msg}`);
}

function pass(name: string) { console.log(`  ✅ PASS: ${name}`); }
function fail(name: string, err: string) { console.log(`  ❌ FAIL: ${name}: ${err}`); }

async function main() {
  console.log("=".repeat(70));
  console.log("ZYMATICA | Solana Cuneiform — Devnet Integration Tests");
  console.log("=".repeat(70));
  console.log();

  // Load wallet
  const walletPath = path.join(
    process.env.USERPROFILE || process.env.HOME || "",
    ".config", "solana", "id.json"
  );
  const deployer = loadKeypair(walletPath);
  const connection = new Connection(DEVNET_URL, "confirmed");
  const client = new CuneiformClient(DEVNET_URL, PROGRAM_ID);
  const treasury = new PublicKey(TREASURY);

  log("🔑", `Wallet: ${deployer.publicKey.toBase58()}`);
  const balance = await connection.getBalance(deployer.publicKey);
  log("💰", `Balance: ${balance / LAMPORTS_PER_SOL} SOL`);
  console.log();

  let passed = 0;
  let failed = 0;

  // ── TEST 1: Fetch Program State ──────────────────────────────────────
  console.log("── Test 1: Fetch Program State ──");
  try {
    const state = await client.fetchProgramState();
    if (!state) throw new Error("Program state not found");

    if (state.admin.toBase58() === deployer.publicKey.toBase58()) {
      pass("Admin matches deployer");
      passed++;
    } else {
      fail("Admin mismatch", `${state.admin.toBase58()} != ${deployer.publicKey.toBase58()}`);
      failed++;
    }

    if (state.treasury.toBase58() === TREASURY) {
      pass("Treasury matches cold wallet");
      passed++;
    } else {
      fail("Treasury mismatch", `${state.treasury.toBase58()}`);
      failed++;
    }

    if (state.feeLamports === BigInt(150_000)) {
      pass("Protocol fee = 150,000 lamports");
      passed++;
    } else {
      fail("Fee mismatch", `${state.feeLamports}`);
      failed++;
    }
  } catch (err: any) {
    fail("Fetch program state", err.message);
    failed += 3;
  }
  console.log();

  // ── TEST 2: Register Coordinates ─────────────────────────────────────
  console.log("── Test 2: Register Cuneiform-U Coordinates ──");
  
  // Use a unique session ID per run to avoid PDA collision
  const sessionId = Buffer.alloc(16);
  const runId = Date.now().toString(36).slice(-8).padEnd(16, "0");
  sessionId.write(runId, "utf-8");
  
  const coords = [42, 7, 3, 128, 200, 15]; // Domain, Subdomain, Modality, Polarity, Strength, Depth
  const merkleRoot = crypto.createHash("sha256").update(Buffer.from(coords)).digest();

  log("📋", `Session ID: ${runId}`);
  log("📊", `Coords: [${coords.join(", ")}]`);
  log("🔐", `Merkle Root: ${merkleRoot.toString("hex").slice(0, 32)}...`);

  try {
    // Get treasury balance before
    const treasuryBefore = await connection.getBalance(treasury);
    
    const regTx = await client.registerCoordinates(
      deployer,
      sessionId,
      coords,
      merkleRoot,
      treasury
    );

    pass("Coordinate registration transaction succeeded");
    log("📝", `TX: ${regTx}`);
    log("🔗", `https://explorer.solana.com/tx/${regTx}?cluster=devnet`);
    passed++;

    // Verify protocol fee was collected
    const treasuryAfter = await connection.getBalance(treasury);
    const feePaid = treasuryAfter - treasuryBefore;
    if (feePaid >= 150_000) {
      pass(`Protocol fee collected: ${feePaid} lamports sent to treasury`);
      passed++;
    } else {
      fail("Protocol fee not collected", `Delta: ${feePaid}`);
      failed++;
    }
  } catch (err: any) {
    fail("Register coordinates", err.message);
    failed += 2;
    // If registration fails, skip remaining tests
    printResults(passed, failed);
    return;
  }
  console.log();

  // ── TEST 3: Fetch On-Chain Record ────────────────────────────────────
  console.log("── Test 3: Fetch and Verify On-Chain Record ──");
  try {
    const record = await client.fetchRecord(deployer.publicKey, sessionId);
    if (!record) throw new Error("Record not found on-chain");

    if (record.authority.toBase58() === deployer.publicKey.toBase58()) {
      pass("Authority matches");
      passed++;
    } else {
      fail("Authority mismatch", record.authority.toBase58());
      failed++;
    }

    const coordsMatch = JSON.stringify(record.coords) === JSON.stringify(coords);
    if (coordsMatch) {
      pass(`Coordinates match: [${record.coords.join(", ")}]`);
      passed++;
    } else {
      fail("Coords mismatch", `[${record.coords.join(", ")}]`);
      failed++;
    }

    if (record.timestamp > 0) {
      pass(`Timestamp recorded: ${new Date(record.timestamp * 1000).toISOString()}`);
      passed++;
    } else {
      fail("Timestamp missing", `${record.timestamp}`);
      failed++;
    }

    const rootMatch = merkleRoot.equals(record.merkleRoot);
    if (rootMatch) {
      pass("Merkle root matches");
      passed++;
    } else {
      fail("Merkle root mismatch", record.merkleRoot.toString("hex").slice(0, 16));
      failed++;
    }
  } catch (err: any) {
    fail("Fetch on-chain record", err.message);
    failed += 4;
  }
  console.log();

  // ── TEST 4: Update Coordinates ───────────────────────────────────────
  console.log("── Test 4: Update Coordinates ──");
  const newCoords = [99, 14, 6, 64, 255, 30]; // Updated coords
  const newMerkleRoot = crypto.createHash("sha256").update(Buffer.from(newCoords)).digest();

  try {
    const updateTx = await client.updateCoordinates(
      deployer,
      sessionId,
      newCoords,
      newMerkleRoot
    );
    pass("Update transaction succeeded");
    log("📝", `TX: ${updateTx}`);
    passed++;

    // Verify update
    const updated = await client.fetchRecord(deployer.publicKey, sessionId);
    if (updated && JSON.stringify(updated.coords) === JSON.stringify(newCoords)) {
      pass(`Coords updated to: [${updated.coords.join(", ")}]`);
      passed++;
    } else {
      fail("Update verification", "Coords not updated");
      failed++;
    }
  } catch (err: any) {
    fail("Update coordinates", err.message);
    failed += 2;
  }
  console.log();

  printResults(passed, failed);
}

function printResults(passed: number, failed: number) {
  console.log("=".repeat(70));
  console.log(`RESULTS: ${passed} passed, ${failed} failed, ${passed + failed} total`);
  console.log("=".repeat(70));
  
  if (failed === 0) {
    console.log();
    console.log("🎉  ALL TESTS PASSED — Protocol is fully operational on devnet!");
    console.log("🚀  Grant Milestone 1: VERIFIED");
  }
}

main().catch(console.error);
