import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { assert } from "chai";
import * as crypto from "crypto";
import * as fs from "fs";
import { keccak_256 } from "@noble/hashes/sha3";
import { execSync } from "child_process";

function getZeroHash(level: number): Buffer {
  let current = Buffer.alloc(32, 0);
  for (let i = 0; i < level; i++) {
    current = Buffer.from(keccak_256(Buffer.concat([current, current])));
  }
  return current;
}

function buildOnChainMerkleProof(leaves: Buffer[], leafIndex: number): Buffer[] {
  let proof: Buffer[] = [];
  let index = leafIndex;
  let currentLevel = [...leaves];

  for (let level = 0; level < 10; level++) {
    let nextLevel: Buffer[] = [];
    const zeroHash = getZeroHash(level);

    for (let i = 0; i < currentLevel.length; i += 2) {
      let left = currentLevel[i];
      let right = i + 1 < currentLevel.length ? currentLevel[i + 1] : zeroHash;

      if (i === index || i + 1 === index) {
        let sibling = i === index ? right : left;
        proof.push(sibling);
      }

      let combined = Buffer.concat([left, right]);
      nextLevel.push(Buffer.from(keccak_256(combined)));
    }
    index = Math.floor(index / 2);
    currentLevel = nextLevel;
  }
  return proof;
}

function buildMerkleProof(leaves: Buffer[], leafIndex: number): Buffer[] {
  let proof: Buffer[] = [];
  let currentLevel = [...leaves];
  let index = leafIndex;

  while (currentLevel.length > 1) {
    let nextLevel: Buffer[] = [];
    for (let i = 0; i < currentLevel.length; i += 2) {
      let left = currentLevel[i];
      let right = i + 1 < currentLevel.length ? currentLevel[i + 1] : left;

      if (i === index || i + 1 === index) {
        let sibling = i === index ? right : left;
        proof.push(sibling);
      }

      let combined = Buffer.concat([left, right]);
      nextLevel.push(Buffer.from(keccak_256(combined)));
    }
    currentLevel = nextLevel;
    index = Math.floor(index / 2);
  }
  return proof;
}


describe("zk_lorawan", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.ZkLorawan as Program<any>;

  // Generate keypairs for our actors
  const gateway = anchor.web3.Keypair.generate();
  const sender = anchor.web3.Keypair.generate();
  const admin = anchor.web3.Keypair.fromSecretKey(
    Uint8Array.from(
      JSON.parse(
        fs.readFileSync("tests/fixtures/integration-admin-keypair.json", "utf8")
      )
    )
  );

  // Hardcoded treasury address matching lib.rs
  const treasuryAddress = new anchor.web3.PublicKey("7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS");

  // Find PDA for registry
  const [registryPda, registryBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("zk-lorawan-registry")],
    program.programId
  );

  // Find PDA for the global shielded pool
  const [shieldedPoolPda, shieldedPoolBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("shielded-pool")],
    program.programId
  );

  let singleChirpProof: any;
  let batchChirpProofs: any[] = [];
  const accumulatedLeaves: Buffer[] = [];

  before(async () => {
    // Airdrop SOL to sender and gateway to cover transaction fees and rent
    const signature1 = await provider.connection.requestAirdrop(
      sender.publicKey,
      5 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature1);

    const signature2 = await provider.connection.requestAirdrop(
      gateway.publicKey,
      5 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature2);

    const signature3 = await provider.connection.requestAirdrop(
      treasuryAddress,
      anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature3);

    const signature4 = await provider.connection.requestAirdrop(
      admin.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(signature4);

    // Generate single chirp proof dynamically for gateway key
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";

    const generateProof = (privKey: string, decKey: string, coord: number, fwHash: string, gwHex: string, depositValue: number = 150000) => {
      const cmd = `"${proverPath}" generate ${privKey} ${decKey} ${coord} ${fwHash} ${gwHex} ${depositValue}`;
      const stdout = execSync(cmd).toString();
      return JSON.parse(stdout);
    };

    singleChirpProof = generateProof(
      "0000000000000000000000000000000000000000000000000000000000000001",
      "0000000000000000000000000000000000000000000000000000000000000002",
      123456789,
      "0000000000000000000000000000000000000000000000000000000000000003",
      gatewayHex
    );

    // Generate 3 unique proofs for batch
    for (let i = 2; i <= 4; i++) {
      batchChirpProofs.push(generateProof(
        `000000000000000000000000000000000000000000000000000000000000000${i}`,
        "0000000000000000000000000000000000000000000000000000000000000002",
        123456789,
        "0000000000000000000000000000000000000000000000000000000000000003",
        gatewayHex
      ));
    }
  });

  it("Initializes the global registry", async () => {
    await program.methods
      .initializeRegistry()
      .accounts({
        registry: registryPda,
        authority: admin.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([admin])
      .rpc();

    const registryAccount = await program.account.protocolRegistry.fetch(registryPda) as any;
    assert.ok(registryAccount.authority.equals(admin.publicKey));
    assert.equal(registryAccount.nextBatchId.toNumber(), 0);

    // Whitelist the firmware hash (0x03) used for normal/sound witness generation in tests
    const testFwHash = Buffer.alloc(32, 0);
    testFwHash[31] = 3;
    await program.methods
      .registerFirmwareHash(Array.from(testFwHash))
      .accounts({
        registry: registryPda,
        authority: admin.publicKey,
      })
      .signers([admin])
      .rpc();
  });

  it("Deposits SOL into the Shared Shielded Pool", async () => {
    const amount = new anchor.BN(150000); // must be exactly TOTAL_FEE_PER_CHIRP
    const singleZkVdeProofHash = Buffer.from(singleChirpProof.identity_hash, "hex");
    const dummyLeafHash = Array.from(singleZkVdeProofHash);

    await program.methods
      .depositShielded(amount, dummyLeafHash)
      .accounts({
        sender: sender.publicKey,
        shieldedPool: shieldedPoolPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(singleZkVdeProofHash);

    const poolAccount = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    assert.isTrue(poolAccount.totalBalance.toNumber() >= amount.toNumber());

    // Compute the expected root after index 0 insertion
    let current = singleZkVdeProofHash;
    for (let level = 0; level < 10; level++) {
      current = Buffer.from(keccak_256(Buffer.concat([current, getZeroHash(level)])));
    }
    assert.deepEqual(poolAccount.merkleRoot, Array.from(current));
  });

  it("Performs single shielded chirp verification (MODE A)", async () => {
    const nullifierHash = Buffer.from(singleChirpProof.nullifier_hash, "hex");
    const attestationHash = Buffer.from(singleChirpProof.attestation_hash, "hex");
    const zkVdeProofHash = Buffer.from(singleChirpProof.identity_hash, "hex");
    const ciphertextHash = Buffer.from(singleChirpProof.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(singleChirpProof.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(singleChirpProof.firmware_hash, "hex");
    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    // Real, mathematically verified Groth16 proof coordinates
    const proofA = Buffer.from(singleChirpProof.proof_a, "hex");
    const proofB = Buffer.from(singleChirpProof.proof_b, "hex");
    const proofC = Buffer.from(singleChirpProof.proof_c, "hex");

    // Find PDA for nullifier record
    const [nullifierRecordPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash],
      program.programId
    );

    // Get initial balances
    const initialGatewayBal = await provider.connection.getBalance(gateway.publicKey);
    const initialTreasuryBal = await provider.connection.getBalance(treasuryAddress);
    const nullifierRent = await provider.connection.getMinimumBalanceForRentExemption(8 + 32 + 8);

    // Compute Merkle proof for index 0 (all siblings are zero hashes)
    const proof0 = Array.from({ length: 10 }, (_, i) => getZeroHash(i));

    // Call verify_single
    await program.methods
      .verifySingle(
        Array.from(proofA),
        Array.from(proofB),
        Array.from(proofC),
        Array.from(nullifierHash),
        Array.from(attestationHash),
        Array.from(zkVdeProofHash),
        Array.from(ciphertextHash),
        Array.from(depositCommitment),
        Array.from(firmwareHash),
        timestamp,
        proof0.map(p => Array.from(p)),
        0
      )
      .accounts({
        registry: registryPda,
        shieldedPool: shieldedPoolPda,
        nullifierRecord: nullifierRecordPda,
        gateway: gateway.publicKey,
        treasury: treasuryAddress,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .rpc();

    // Verify balances (Gateway gets 100K lamports, Treasury gets 50K lamports)
    const finalGatewayBal = await provider.connection.getBalance(gateway.publicKey);
    const finalTreasuryBal = await provider.connection.getBalance(treasuryAddress);

    assert.equal(finalGatewayBal - initialGatewayBal, 100000 - nullifierRent);
    assert.equal(finalTreasuryBal - initialTreasuryBal, 50000);

    // Verify registry stats
    const registryAccount = await program.account.protocolRegistry.fetch(registryPda) as any;
    assert.equal(registryAccount.totalChirpsVerified.toNumber(), 1);
    assert.equal(registryAccount.totalFeesCollected.toNumber(), 150000);

    // Verify nullifier record state
    const nullifierAccount = await program.account.nullifierRecord.fetch(nullifierRecordPda) as any;
    assert.deepEqual(nullifierAccount.nullifierHash, Array.from(nullifierHash));
  });

  it("Performs batch chirp verification (MODE B)", async () => {
    // Generate a new batch ID from registry
    const registryAccountBefore = await program.account.protocolRegistry.fetch(registryPda) as any;
    const batchId = registryAccountBefore.nextBatchId;

    // Find PDA for the batch
    const batchIdBytes = batchId.toArrayLike(Buffer, "le", 8);
    const [batchPda, batchBump] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("zk-lorawan-batch"), gateway.publicKey.toBuffer(), batchIdBytes],
      program.programId
    );

    // Initialize batch
    await program.methods
      .initializeBatch()
      .accounts({
        batch: batchPda,
        registry: registryPda,
        gateway: gateway.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .rpc();

    const batchAccount = await program.account.batchAccumulator.fetch(batchPda) as any;
    assert.ok(batchAccount.gateway.equals(gateway.publicKey));
    assert.equal(batchAccount.chirpCount, 0);
    assert.isFalse(batchAccount.isFinalized);

    // Add 3 chirps
    // Define 3 real unique Groth16 proofs and hashes
    const proofA1 = Buffer.from(batchChirpProofs[0].proof_a, "hex");
    const proofB1 = Buffer.from(batchChirpProofs[0].proof_b, "hex");
    const proofC1 = Buffer.from(batchChirpProofs[0].proof_c, "hex");
    const chirp1Nullifier = Buffer.from(batchChirpProofs[0].nullifier_hash, "hex");
    const chirp1Attestation = Buffer.from(batchChirpProofs[0].attestation_hash, "hex");
    const chirp1VdeProof = Buffer.from(batchChirpProofs[0].identity_hash, "hex");
    const chirp1Payload = Buffer.from(batchChirpProofs[0].ciphertext_hash, "hex");

    const proofA2 = Buffer.from(batchChirpProofs[1].proof_a, "hex");
    const proofB2 = Buffer.from(batchChirpProofs[1].proof_b, "hex");
    const proofC2 = Buffer.from(batchChirpProofs[1].proof_c, "hex");
    const chirp2Nullifier = Buffer.from(batchChirpProofs[1].nullifier_hash, "hex");
    const chirp2Attestation = Buffer.from(batchChirpProofs[1].attestation_hash, "hex");
    const chirp2VdeProof = Buffer.from(batchChirpProofs[1].identity_hash, "hex");
    const chirp2Payload = Buffer.from(batchChirpProofs[1].ciphertext_hash, "hex");

    const proofA3 = Buffer.from(batchChirpProofs[2].proof_a, "hex");
    const proofB3 = Buffer.from(batchChirpProofs[2].proof_b, "hex");
    const proofC3 = Buffer.from(batchChirpProofs[2].proof_c, "hex");
    const chirp3Nullifier = Buffer.from(batchChirpProofs[2].nullifier_hash, "hex");
    const chirp3Attestation = Buffer.from(batchChirpProofs[2].attestation_hash, "hex");
    const chirp3VdeProof = Buffer.from(batchChirpProofs[2].identity_hash, "hex");
    const chirp3Payload = Buffer.from(batchChirpProofs[2].ciphertext_hash, "hex");

    const timestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    // All leaves currently in the tree: [single_chirp_leaf, leaf1, leaf2, leaf3]
    const allLeaves = [
      Buffer.from(singleChirpProof.identity_hash, "hex"),
      chirp1VdeProof,
      chirp2VdeProof,
      chirp3VdeProof,
    ];

    // Deposit the 3 new batch leaves to the shielded pool on-chain
    const amountPerChirp = new anchor.BN(150000);
    for (let i = 1; i <= 3; i++) {
      await program.methods
        .depositShielded(amountPerChirp, Array.from(allLeaves[i]))
        .accounts({
          sender: sender.publicKey,
          shieldedPool: shieldedPoolPda,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([sender])
        .rpc();
      accumulatedLeaves.push(allLeaves[i]);
    }

    // Compute Merkle proof for each leaf in the 10-depth pool tree
    const proof1 = buildOnChainMerkleProof(accumulatedLeaves, 1);
    const proof2 = buildOnChainMerkleProof(accumulatedLeaves, 2);
    const proof3 = buildOnChainMerkleProof(accumulatedLeaves, 3);

    const depositCommitment1 = Buffer.from(batchChirpProofs[0].deposit_commitment, "hex");
    const firmwareHash1 = Buffer.from(batchChirpProofs[0].firmware_hash, "hex");
    const depositCommitment2 = Buffer.from(batchChirpProofs[1].deposit_commitment, "hex");
    const firmwareHash2 = Buffer.from(batchChirpProofs[1].firmware_hash, "hex");
    const depositCommitment3 = Buffer.from(batchChirpProofs[2].deposit_commitment, "hex");
    const firmwareHash3 = Buffer.from(batchChirpProofs[2].firmware_hash, "hex");

    await program.methods
      .addChirp(
        Array.from(proofA1),
        Array.from(proofB1),
        Array.from(proofC1),
        Array.from(chirp1Nullifier),
        Array.from(chirp1Attestation),
        Array.from(chirp1VdeProof),
        Array.from(chirp1Payload),
        Array.from(depositCommitment1),
        Array.from(firmwareHash1),
        timestamp,
        proof1.map(p => Array.from(p)),
        1
      )
      .accounts({
        batch: batchPda,
        shieldedPool: shieldedPoolPda,
        registry: registryPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    await program.methods
      .addChirp(
        Array.from(proofA2),
        Array.from(proofB2),
        Array.from(proofC2),
        Array.from(chirp2Nullifier),
        Array.from(chirp2Attestation),
        Array.from(chirp2VdeProof),
        Array.from(chirp2Payload),
        Array.from(depositCommitment2),
        Array.from(firmwareHash2),
        timestamp,
        proof2.map(p => Array.from(p)),
        2
      )
      .accounts({
        batch: batchPda,
        shieldedPool: shieldedPoolPda,
        registry: registryPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    await program.methods
      .addChirp(
        Array.from(proofA3),
        Array.from(proofB3),
        Array.from(proofC3),
        Array.from(chirp3Nullifier),
        Array.from(chirp3Attestation),
        Array.from(chirp3VdeProof),
        Array.from(chirp3Payload),
        Array.from(depositCommitment3),
        Array.from(firmwareHash3),
        timestamp,
        proof3.map(p => Array.from(p)),
        3
      )
      .accounts({
        batch: batchPda,
        shieldedPool: shieldedPoolPda,
        registry: registryPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    const batchAccountAfterAdd = await program.account.batchAccumulator.fetch(batchPda) as any;
    assert.equal(batchAccountAfterAdd.chirpCount, 3);

    // Submit batch (Gateway gets 300K lamports, Treasury gets 150K lamports)
    const initialGatewayBal = await provider.connection.getBalance(gateway.publicKey);
    const initialTreasuryBal = await provider.connection.getBalance(treasuryAddress);
    const nullifierRent = await provider.connection.getMinimumBalanceForRentExemption(8 + 32 + 8);

    const nullifiers = [chirp1Nullifier, chirp2Nullifier, chirp3Nullifier];
    const nullifierPdas = nullifiers.map((nullifier) => {
      const [pda] = anchor.web3.PublicKey.findProgramAddressSync(
        [Buffer.from("nullifier"), nullifier],
        program.programId
      );
      return { pubkey: pda, isWritable: true, isSigner: false };
    });

    await program.methods
      .submitBatch()
      .accounts({
        batch: batchPda,
        registry: registryPda,
        shieldedPool: shieldedPoolPda,
        gateway: gateway.publicKey,
        treasury: treasuryAddress,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .remainingAccounts(nullifierPdas)
      .rpc();

    const finalGatewayBal = await provider.connection.getBalance(gateway.publicKey);
    const finalTreasuryBal = await provider.connection.getBalance(treasuryAddress);

    assert.equal(finalGatewayBal - initialGatewayBal, 300000 - 3 * nullifierRent);
    assert.equal(finalTreasuryBal - initialTreasuryBal, 150000);

    const batchAccountFinal = await program.account.batchAccumulator.fetch(batchPda) as any;
    assert.isTrue(batchAccountFinal.isFinalized);
    assert.notDeepEqual(batchAccountFinal.merkleRoot, Array(32).fill(0));

    // Verify registry stats (total verified = 1 from single + 3 from batch = 4)
    const registryAccountAfter = await program.account.protocolRegistry.fetch(registryPda) as any;
    assert.equal(registryAccountAfter.totalChirpsVerified.toNumber(), 4);
    assert.equal(registryAccountAfter.totalFeesCollected.toNumber(), 600000); // 150K + 450K

    // Test verify_chirp_inclusion
    const proof = buildMerkleProof(nullifiers, 0);
    await program.methods
      .verifyChirpInclusion(
        Array.from(chirp1Nullifier),
        proof.map(p => Array.from(p)),
        0
      )
      .accounts({
        batch: batchPda,
      })
      .rpc();
  });

  // ==========================================================================
  // Chunked Proof Flow — MODE A-CHUNKED
  // ==========================================================================

  it("Performs chunked proof verification (init → 3 writes → verify → close)", async () => {
    // First, generate a NEW proof for the chunked flow (unique private key so nullifier is unique)
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const chunkedProof = JSON.parse(
      execSync(`"${proverPath}" generate 0000000000000000000000000000000000000000000000000000000000000009 0000000000000000000000000000000000000000000000000000000000000002 123456789 0000000000000000000000000000000000000000000000000000000000000003 ${gatewayHex} 150000`).toString()
    );

    // Deposit this new leaf into the pool
    const chunkedLeafHash = Buffer.from(chunkedProof.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(chunkedLeafHash))
      .accounts({
        sender: sender.publicKey,
        shieldedPool: shieldedPoolPda,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(chunkedLeafHash);

    // Get pool state for leaf index
    const poolBefore = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolBefore.nextIndex.toNumber() - 1; // Just deposited

    const merkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);

    // Build proof data
    const proofA = Buffer.from(chunkedProof.proof_a, "hex");
    const proofB = Buffer.from(chunkedProof.proof_b, "hex");
    const proofC = Buffer.from(chunkedProof.proof_c, "hex");
    const nullifierHash = Buffer.from(chunkedProof.nullifier_hash, "hex");

    // Build public_inputs array matching the circuit order:
    // [identity, nullifier, attestation, ciphertext, gw_part1, gw_part2, deposit, firmware]
    const identityHash = Buffer.from(chunkedProof.identity_hash, "hex");
    const attestationHash = Buffer.from(chunkedProof.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(chunkedProof.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(chunkedProof.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(chunkedProof.firmware_hash, "hex");

    // Gateway binding: match how the circuit encodes it
    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16); // lower 16 bytes at offset 0
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32); // upper 16 bytes at offset 0

    const publicInputs: Buffer[] = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    // === Step 1: Initialize ProofContext ===
    const nonce = new anchor.BN(Date.now());
    const [proofCtxPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("proof-ctx"), gateway.publicKey.toBuffer(), nonce.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    const initTx = await program.methods
      .initializeProofContext(nonce)
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .transaction();
    initTx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
    initTx.feePayer = gateway.publicKey;
    console.log(`  initializeProofContext tx size: ${initTx.serialize({requireAllSignatures: false}).length} bytes`);
    assert.isBelow(initTx.serialize({requireAllSignatures: false}).length, 1232, "init tx exceeds Solana limit");

    await program.methods
      .initializeProofContext(nonce)
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .rpc();

    // === Step 2a: Write Chunk 0 — proof_a (64) + proof_b (128) = 192 bytes ===
    const chunk0 = Buffer.concat([proofA, proofB]);
    assert.equal(chunk0.length, 192);

    await program.methods
      .writeProofChunk(nonce, 0, Buffer.from(chunk0))
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    // === Step 2b: Write Chunk 1 — proof_c (64) + public_inputs (256) = 320 bytes ===
    const chunk1Parts = [proofC, ...publicInputs];
    const chunk1 = Buffer.concat(chunk1Parts);
    assert.equal(chunk1.length, 320);

    await program.methods
      .writeProofChunk(nonce, 1, Buffer.from(chunk1))
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    // === Step 2c: Write Chunk 2 — merkle_proof (10*32=320) + leaf_index (4) = 516 bytes ===
    const merkleProofBuf = Buffer.concat(merkleProof.map(p => Buffer.from(p)));
    const leafIndexBuf = Buffer.alloc(4);
    leafIndexBuf.writeUInt32LE(leafIndex, 0);
    const chunk2 = Buffer.concat([merkleProofBuf, leafIndexBuf]);
    assert.equal(chunk2.length, 324);

    const write2Tx = await program.methods
      .writeProofChunk(nonce, 2, Buffer.from(chunk2))
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .transaction();
    write2Tx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
    write2Tx.feePayer = gateway.publicKey;
    console.log(`  writeProofChunk(2) tx size: ${write2Tx.serialize({requireAllSignatures: false}).length} bytes`);
    assert.isBelow(write2Tx.serialize({requireAllSignatures: false}).length, 1232, "write chunk 2 tx exceeds Solana limit");

    await program.methods
      .writeProofChunk(nonce, 2, Buffer.from(chunk2))
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    // Verify context is complete
    const ctxAfterWrites = await program.account.proofContext.fetch(proofCtxPda) as any;
    assert.isTrue(ctxAfterWrites.isComplete);
    assert.equal(ctxAfterWrites.chunksWritten, 7); // 0b111

    // === Step 3: Verify ProofContext ===
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash],
      program.programId
    );

    const initialGatewayBal = await provider.connection.getBalance(gateway.publicKey);
    const initialTreasuryBal = await provider.connection.getBalance(treasuryAddress);
    const nullifierRent = await provider.connection.getMinimumBalanceForRentExemption(8 + 32 + 8);

    const verifyTx = await program.methods
      .verifyProofContext(nonce)
      .accounts({
        proofContext: proofCtxPda,
        registry: registryPda,
        shieldedPool: shieldedPoolPda,
        nullifierRecord: nullifierPda,
        gateway: gateway.publicKey,
        treasury: treasuryAddress,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .transaction();
    verifyTx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
    verifyTx.feePayer = gateway.publicKey;
    console.log(`  verifyProofContext tx size: ${verifyTx.serialize({requireAllSignatures: false}).length} bytes`);
    assert.isBelow(verifyTx.serialize({requireAllSignatures: false}).length, 1232, "verify tx exceeds Solana limit");

    await program.methods
      .verifyProofContext(nonce)
      .accounts({
        proofContext: proofCtxPda,
        registry: registryPda,
        shieldedPool: shieldedPoolPda,
        nullifierRecord: nullifierPda,
        gateway: gateway.publicKey,
        treasury: treasuryAddress,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .rpc();

    // Verify payouts
    const finalGatewayBal = await provider.connection.getBalance(gateway.publicKey);
    const finalTreasuryBal = await provider.connection.getBalance(treasuryAddress);
    assert.equal(finalGatewayBal - initialGatewayBal, 100000 - nullifierRent);
    assert.equal(finalTreasuryBal - initialTreasuryBal, 50000);

    // Verify registry stats (previous 4 + this 1 = 5)
    const registryAccount = await program.account.protocolRegistry.fetch(registryPda) as any;
    assert.equal(registryAccount.totalChirpsVerified.toNumber(), 5);

    // === Step 4: Close ProofContext ===
    const closeTx = await program.methods
      .closeProofContext(nonce)
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .transaction();
    closeTx.recentBlockhash = (await provider.connection.getLatestBlockhash()).blockhash;
    closeTx.feePayer = gateway.publicKey;
    console.log(`  closeProofContext tx size: ${closeTx.serialize({requireAllSignatures: false}).length} bytes`);
    assert.isBelow(closeTx.serialize({requireAllSignatures: false}).length, 1232, "close tx exceeds Solana limit");

    await program.methods
      .closeProofContext(nonce)
      .accounts({
        proofContext: proofCtxPda,
        gateway: gateway.publicKey,
      })
      .signers([gateway])
      .rpc();

    // Verify PDA is closed (should fail to fetch)
    try {
      await program.account.proofContext.fetch(proofCtxPda);
      assert.fail("ProofContext should be closed");
    } catch (e: any) {
      assert.include(e.message, "Account does not exist");
    }

    console.log("  ✅ Chunked proof flow complete: init → 3 writes → verify → close");
  });

  // ==========================================================================
  // Production Chunked Negative & Edge Case Tests
  // ==========================================================================

  // Helper to initialize and write all 3 chunks for a proof context
  async function setupProofContextChunks(
    nonce: anchor.BN,
    proofA: Buffer,
    proofB: Buffer,
    proofC: Buffer,
    publicInputs: Buffer[],
    merkleProof: Buffer[],
    leafIndex: number,
    gwKey: anchor.web3.Keypair,
    options: {
      skipInit?: boolean;
      skipChunk0?: boolean;
      skipChunk1?: boolean;
      skipChunk2?: boolean;
      corruptChunk0?: boolean;
      corruptChunk2?: boolean;
    } = {}
  ) {
    const [proofCtxPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("proof-ctx"), gwKey.publicKey.toBuffer(), nonce.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    if (!options.skipInit) {
      await program.methods
        .initializeProofContext(nonce)
        .accounts({
          proofContext: proofCtxPda,
          gateway: gwKey.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gwKey])
        .rpc();
    }

    if (!options.skipChunk0) {
      let chunk0 = Buffer.concat([proofA, proofB]);
      if (options.corruptChunk0) {
        chunk0[0] ^= 0xFF; // Flip a byte
      }
      await program.methods
        .writeProofChunk(nonce, 0, Buffer.from(chunk0))
        .accounts({
          proofContext: proofCtxPda,
          gateway: gwKey.publicKey,
        })
        .signers([gwKey])
        .rpc();
    }

    if (!options.skipChunk1) {
      const chunk1 = Buffer.concat([proofC, ...publicInputs]);
      await program.methods
        .writeProofChunk(nonce, 1, Buffer.from(chunk1))
        .accounts({
          proofContext: proofCtxPda,
          gateway: gwKey.publicKey,
        })
        .signers([gwKey])
        .rpc();
    }

    if (!options.skipChunk2) {
      let merkleProofBuf = Buffer.concat(merkleProof.map(p => Buffer.from(p)));
      if (options.corruptChunk2) {
        merkleProofBuf = Buffer.alloc(320, 0xDE); // bad siblings
      }
      const leafIndexBuf = Buffer.alloc(4);
      leafIndexBuf.writeUInt32LE(leafIndex, 0);
      const chunk2 = Buffer.concat([merkleProofBuf, leafIndexBuf]);
      await program.methods
        .writeProofChunk(nonce, 2, Buffer.from(chunk2))
        .accounts({
          proofContext: proofCtxPda,
          gateway: gwKey.publicKey,
        })
        .signers([gwKey])
        .rpc();
    }

    return proofCtxPda;
  }

  it("Rejects tampered proof", async () => {
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const proofData = JSON.parse(
      execSync(`"${proverPath}" generate 000000000000000000000000000000000000000000000000000000000000000a 0000000000000000000000000000000000000000000000000000000000000002 123456789 0000000000000000000000000000000000000000000000000000000000000003 ${gatewayHex} 150000`).toString()
    );

    // Deposit leaf
    const leafHash = Buffer.from(proofData.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(leafHash))
      .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(leafHash);

    const poolState = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolState.nextIndex.toNumber() - 1;

    const proofA = Buffer.from(proofData.proof_a, "hex");
    const proofB = Buffer.from(proofData.proof_b, "hex");
    const proofC = Buffer.from(proofData.proof_c, "hex");
    const nullifierHash = Buffer.from(proofData.nullifier_hash, "hex");

    const identityHash = Buffer.from(proofData.identity_hash, "hex");
    const attestationHash = Buffer.from(proofData.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(proofData.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(proofData.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(proofData.firmware_hash, "hex");

    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16);
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32);

    const publicInputs = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    const realMerkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);

    const nonce = new anchor.BN(Date.now() + 1);
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash], program.programId
    );

    // Write chunks but tamper with chunk0 (proof_a / proof_b)
    const proofCtxPda = await setupProofContextChunks(
      nonce, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, gateway,
      { corruptChunk0: true }
    );

    try {
      await program.methods
        .verifyProofContext(nonce)
        .accounts({
          proofContext: proofCtxPda,
          registry: registryPda,
          shieldedPool: shieldedPoolPda,
          nullifierRecord: nullifierPda,
          gateway: gateway.publicKey,
          treasury: treasuryAddress,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gateway])
        .rpc();
      assert.fail("Should have rejected tampered proof");
    } catch (e: any) {
      assert.include(e.message, "Zero Knowledge witness verification failed");
      console.log("  ✅ Tampered proof rejected on production chunked path");
    }

    // Clean up
    await program.methods.closeProofContext(nonce).accounts({ proofContext: proofCtxPda, gateway: gateway.publicKey }).signers([gateway]).rpc();
  });

  it("Rejects wrong deposit amount", async () => {
    try {
      await program.methods
        .depositShielded(new anchor.BN(100000), Array.from(Buffer.alloc(32, 0xAA)))
        .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
        .signers([sender])
        .rpc();
      assert.fail("Should have rejected wrong deposit amount");
    } catch (e: any) {
      assert.include(e.message, "Invalid deposit amount");
      console.log("  ✅ Wrong deposit amount correctly rejected");
    }
  });

  it("Rejects nullifier replay", async () => {
    // Generate a unique proof
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const proofData = JSON.parse(
      execSync(`"${proverPath}" generate 000000000000000000000000000000000000000000000000000000000000000f 0000000000000000000000000000000000000000000000000000000000000002 123456789 0000000000000000000000000000000000000000000000000000000000000003 ${gatewayHex} 150000`).toString()
    );

    // Deposit leaf
    const leafHash = Buffer.from(proofData.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(leafHash))
      .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(leafHash);

    const poolState = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolState.nextIndex.toNumber() - 1;

    const proofA = Buffer.from(proofData.proof_a, "hex");
    const proofB = Buffer.from(proofData.proof_b, "hex");
    const proofC = Buffer.from(proofData.proof_c, "hex");
    const nullifierHash = Buffer.from(proofData.nullifier_hash, "hex");

    const identityHash = Buffer.from(proofData.identity_hash, "hex");
    const attestationHash = Buffer.from(proofData.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(proofData.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(proofData.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(proofData.firmware_hash, "hex");

    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16);
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32);

    const publicInputs = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    const realMerkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash], program.programId
    );

    // First, verify successfully on the chunked path
    const nonce1 = new anchor.BN(Date.now() + 10);
    const proofCtxPda1 = await setupProofContextChunks(
      nonce1, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, gateway
    );
    await program.methods
      .verifyProofContext(nonce1)
      .accounts({
        proofContext: proofCtxPda1, registry: registryPda, shieldedPool: shieldedPoolPda,
        nullifierRecord: nullifierPda, gateway: gateway.publicKey, treasury: treasuryAddress,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([gateway])
      .rpc();
    await program.methods.closeProofContext(nonce1).accounts({ proofContext: proofCtxPda1, gateway: gateway.publicKey }).signers([gateway]).rpc();

    // Now attempt nullifier replay via a second chunked verify context
    const nonce2 = new anchor.BN(Date.now() + 20);
    const proofCtxPda2 = await setupProofContextChunks(
      nonce2, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, gateway
    );

    try {
      await program.methods
        .verifyProofContext(nonce2)
        .accounts({
          proofContext: proofCtxPda2, registry: registryPda, shieldedPool: shieldedPoolPda,
          nullifierRecord: nullifierPda, gateway: gateway.publicKey, treasury: treasuryAddress,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gateway])
        .rpc();
      assert.fail("Should have rejected nullifier replay");
    } catch (e: any) {
      console.log("  ✅ Nullifier replay correctly rejected on chunked path");
    }

    await program.methods.closeProofContext(nonce2).accounts({ proofContext: proofCtxPda2, gateway: gateway.publicKey }).signers([gateway]).rpc();
  });

  it("Rejects unapproved firmware hash", async () => {
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const badFwProof = JSON.parse(
      execSync(`"${proverPath}" generate 000000000000000000000000000000000000000000000000000000000000000b 0000000000000000000000000000000000000000000000000000000000000002 123456789 000000000000000000000000000000000000000000000000000000000000dead ${gatewayHex} 150000`).toString()
    );

    const leafHash = Buffer.from(badFwProof.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(leafHash))
      .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(leafHash);

    const poolState = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolState.nextIndex.toNumber() - 1;

    const proofA = Buffer.from(badFwProof.proof_a, "hex");
    const proofB = Buffer.from(badFwProof.proof_b, "hex");
    const proofC = Buffer.from(badFwProof.proof_c, "hex");
    const nullifierHash = Buffer.from(badFwProof.nullifier_hash, "hex");

    const identityHash = Buffer.from(badFwProof.identity_hash, "hex");
    const attestationHash = Buffer.from(badFwProof.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(badFwProof.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(badFwProof.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(badFwProof.firmware_hash, "hex");

    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16);
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32);

    const publicInputs = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    const realMerkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash], program.programId
    );

    const nonce = new anchor.BN(Date.now() + 30);
    const proofCtxPda = await setupProofContextChunks(
      nonce, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, gateway
    );

    try {
      await program.methods
        .verifyProofContext(nonce)
        .accounts({
          proofContext: proofCtxPda, registry: registryPda, shieldedPool: shieldedPoolPda,
          nullifierRecord: nullifierPda, gateway: gateway.publicKey, treasury: treasuryAddress,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gateway])
        .rpc();
      assert.fail("Should have rejected unapproved firmware");
    } catch (e: any) {
      assert.include(e.message, "Invalid firmware attestation hash");
      console.log("  ✅ Unapproved firmware correctly rejected on chunked path");
    }

    await program.methods.closeProofContext(nonce).accounts({ proofContext: proofCtxPda, gateway: gateway.publicKey }).signers([gateway]).rpc();
  });

  it("Rejects wrong gateway signer", async () => {
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const wrongGwProof = JSON.parse(
      execSync(`"${proverPath}" generate 000000000000000000000000000000000000000000000000000000000000000c 0000000000000000000000000000000000000000000000000000000000000002 123456789 0000000000000000000000000000000000000000000000000000000000000003 ${gatewayHex} 150000`).toString()
    );

    const leafHash = Buffer.from(wrongGwProof.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(leafHash))
      .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(leafHash);

    const poolState = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolState.nextIndex.toNumber() - 1;

    const proofA = Buffer.from(wrongGwProof.proof_a, "hex");
    const proofB = Buffer.from(wrongGwProof.proof_b, "hex");
    const proofC = Buffer.from(wrongGwProof.proof_c, "hex");
    const nullifierHash = Buffer.from(wrongGwProof.nullifier_hash, "hex");

    const identityHash = Buffer.from(wrongGwProof.identity_hash, "hex");
    const attestationHash = Buffer.from(wrongGwProof.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(wrongGwProof.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(wrongGwProof.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(wrongGwProof.firmware_hash, "hex");

    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16);
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32);

    const publicInputs = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    const realMerkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash], program.programId
    );

    const fakeGateway = anchor.web3.Keypair.generate();
    const sig = await provider.connection.requestAirdrop(fakeGateway.publicKey, anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);

    const nonce = new anchor.BN(Date.now() + 40);
    const proofCtxPda = await setupProofContextChunks(
      nonce, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, fakeGateway
    );

    try {
      await program.methods
        .verifyProofContext(nonce)
        .accounts({
          proofContext: proofCtxPda, registry: registryPda, shieldedPool: shieldedPoolPda,
          nullifierRecord: nullifierPda, gateway: fakeGateway.publicKey, treasury: treasuryAddress,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([fakeGateway])
        .rpc();
      assert.fail("Should have rejected wrong gateway signer");
    } catch (e: any) {
      console.log("  ✅ Wrong gateway signer correctly rejected on chunked path");
    }

    await program.methods.closeProofContext(nonce).accounts({ proofContext: proofCtxPda, gateway: fakeGateway.publicKey }).signers([fakeGateway]).rpc();
  });

  it("Rejects wrong Merkle proof", async () => {
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const badMerkleProof = JSON.parse(
      execSync(`"${proverPath}" generate 000000000000000000000000000000000000000000000000000000000000000d 0000000000000000000000000000000000000000000000000000000000000002 123456789 0000000000000000000000000000000000000000000000000000000000000003 ${gatewayHex} 150000`).toString()
    );

    const leafHash = Buffer.from(badMerkleProof.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(leafHash))
      .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(leafHash);

    const poolState = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolState.nextIndex.toNumber() - 1;

    const proofA = Buffer.from(badMerkleProof.proof_a, "hex");
    const proofB = Buffer.from(badMerkleProof.proof_b, "hex");
    const proofC = Buffer.from(badMerkleProof.proof_c, "hex");
    const nullifierHash = Buffer.from(badMerkleProof.nullifier_hash, "hex");

    const identityHash = Buffer.from(badMerkleProof.identity_hash, "hex");
    const attestationHash = Buffer.from(badMerkleProof.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(badMerkleProof.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(badMerkleProof.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(badMerkleProof.firmware_hash, "hex");

    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16);
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32);

    const publicInputs = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    const realMerkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash], program.programId
    );

    const nonce = new anchor.BN(Date.now() + 50);
    const proofCtxPda = await setupProofContextChunks(
      nonce, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, gateway,
      { corruptChunk2: true }
    );

    try {
      await program.methods
        .verifyProofContext(nonce)
        .accounts({
          proofContext: proofCtxPda, registry: registryPda, shieldedPool: shieldedPoolPda,
          nullifierRecord: nullifierPda, gateway: gateway.publicKey, treasury: treasuryAddress,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gateway])
        .rpc();
      assert.fail("Should have rejected wrong Merkle proof");
    } catch (e: any) {
      assert.include(e.message, "Merkle proof does not match");
      console.log("  ✅ Wrong Merkle proof correctly rejected on chunked path");
    }

    await program.methods.closeProofContext(nonce).accounts({ proofContext: proofCtxPda, gateway: gateway.publicKey }).signers([gateway]).rpc();
  });

  it("Rejects incomplete context", async () => {
    const gatewayHex = Buffer.from(gateway.publicKey.toBytes()).toString("hex");
    const proverPath = process.platform === "win32" ? "target\\release\\zk_lorawan_prove.exe" : "./target/release/zk_lorawan_prove";
    const proofData = JSON.parse(
      execSync(`"${proverPath}" generate 000000000000000000000000000000000000000000000000000000000000000e 0000000000000000000000000000000000000000000000000000000000000002 123456789 0000000000000000000000000000000000000000000000000000000000000003 ${gatewayHex} 150000`).toString()
    );

    const leafHash = Buffer.from(proofData.identity_hash, "hex");
    await program.methods
      .depositShielded(new anchor.BN(150000), Array.from(leafHash))
      .accounts({ sender: sender.publicKey, shieldedPool: shieldedPoolPda, systemProgram: anchor.web3.SystemProgram.programId })
      .signers([sender])
      .rpc();

    accumulatedLeaves.push(leafHash);

    const poolState = await program.account.shieldedEscrowPool.fetch(shieldedPoolPda) as any;
    const leafIndex = poolState.nextIndex.toNumber() - 1;

    const proofA = Buffer.from(proofData.proof_a, "hex");
    const proofB = Buffer.from(proofData.proof_b, "hex");
    const proofC = Buffer.from(proofData.proof_c, "hex");
    const nullifierHash = Buffer.from(proofData.nullifier_hash, "hex");

    const identityHash = Buffer.from(proofData.identity_hash, "hex");
    const attestationHash = Buffer.from(proofData.attestation_hash, "hex");
    const ciphertextHash = Buffer.from(proofData.ciphertext_hash, "hex");
    const depositCommitment = Buffer.from(proofData.deposit_commitment, "hex");
    const firmwareHash = Buffer.from(proofData.firmware_hash, "hex");

    const gwBytes = Buffer.from(gateway.publicKey.toBytes());
    const gwPart1 = Buffer.alloc(32);
    gwBytes.copy(gwPart1, 0, 0, 16);
    const gwPart2 = Buffer.alloc(32);
    gwBytes.copy(gwPart2, 0, 16, 32);

    const publicInputs = [
      identityHash, nullifierHash, attestationHash, ciphertextHash,
      gwPart1, gwPart2, depositCommitment, firmwareHash,
    ];

    const realMerkleProof = buildOnChainMerkleProof(accumulatedLeaves, leafIndex);
    const [nullifierPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("nullifier"), nullifierHash], program.programId
    );

    const nonce = new anchor.BN(Date.now() + 60);
    const proofCtxPda = await setupProofContextChunks(
      nonce, proofA, proofB, proofC, publicInputs, realMerkleProof, leafIndex, gateway,
      { skipChunk2: true }
    );

    try {
      await program.methods
        .verifyProofContext(nonce)
        .accounts({
          proofContext: proofCtxPda, registry: registryPda, shieldedPool: shieldedPoolPda,
          nullifierRecord: nullifierPda, gateway: gateway.publicKey, treasury: treasuryAddress,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([gateway])
        .rpc();
      assert.fail("Should have rejected incomplete proof context");
    } catch (e: any) {
      assert.include(e.message, "ProofContextIncomplete");
      console.log("  ✅ Incomplete proof context correctly rejected");
    }

    await program.methods.closeProofContext(nonce).accounts({ proofContext: proofCtxPda, gateway: gateway.publicKey }).signers([gateway]).rpc();
  });

});
