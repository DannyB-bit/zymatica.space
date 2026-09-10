import { Keypair, PublicKey } from "@solana/web3.js";
import { CuneiformClient } from "../app/src/cuneiform_client.js";
import { SolanaPayMeshGateway } from "../app/src/solana_pay_mock.js";
import * as crypto from "crypto";
async function runTests() {
    console.log("--- Executing Zymatica-Solana Integration Test Suite ---");
    const programId = "CunE111111111111111111111111111111111111111";
    const client = new CuneiformClient("https://api.devnet.solana.com", programId);
    const payGateway = new SolanaPayMeshGateway("https://api.devnet.solana.com");
    const authority = Keypair.generate();
    const sessionId = crypto.randomBytes(16);
    // Domain=12, Subdomain=34, Modality=2, Polarity=1, Strength=99, Depth=5
    const coords = [12, 34, 2, 1, 99, 5];
    const merkleRoot = crypto.createHash("sha256").update("zymatica-consensus").digest();
    // Test 1: PDA Derivation
    console.log("\n[Test 1] Verifying PDA Derivation...");
    const [recordPda, bump] = client.deriveRecordAddress(authority.publicKey, sessionId);
    console.log(`PDA Derived: ${recordPda.toBase58()}`);
    console.log(`Bump: ${bump}`);
    if (!recordPda)
        throw new Error("PDA Derivation failed");
    console.log("-> Test 1 Passed.");
    // Test 2: Anchor Account Deserialization Mockup
    console.log("\n[Test 2] Verifying Anchor Account Deserialization...");
    // Calculate expected account discriminator
    const expectedDiscriminator = crypto.createHash("sha256").update("account:CoordinateRecord").digest().subarray(0, 8);
    const authorityBuffer = authority.publicKey.toBuffer();
    const coordsBuffer = Buffer.from(coords);
    const timestampBuffer = Buffer.alloc(8);
    timestampBuffer.writeBigInt64LE(BigInt(Math.floor(Date.now() / 1000)));
    const bumpBuffer = Buffer.from([bump]);
    // Construct binary payload matching Rust struct layout
    const mockAccountData = Buffer.concat([
        expectedDiscriminator, // 8 bytes
        authorityBuffer, // 32 bytes
        sessionId, // 16 bytes
        coordsBuffer, // 6 bytes
        merkleRoot, // 32 bytes
        timestampBuffer, // 8 bytes
        bumpBuffer, // 1 byte
    ]);
    // Override connection.getAccountInfo to return our mock account
    client.connection.getAccountInfo = async (pubkey) => {
        if (pubkey.equals(recordPda)) {
            return {
                executable: false,
                owner: new PublicKey(programId),
                lamports: 10000000,
                data: mockAccountData,
            };
        }
        return null;
    };
    const record = await client.fetchRecord(authority.publicKey, sessionId);
    if (!record)
        throw new Error("Failed to fetch record");
    console.log("Deserialized Record:");
    console.log(` - Authority: ${record.authority.toBase58()}`);
    console.log(` - Session ID: ${record.sessionId.toString("hex")}`);
    console.log(` - Coordinates: [${record.coords.join(", ")}]`);
    console.log(` - Merkle Root: sha256:${record.merkleRoot.toString("hex")}`);
    console.log(` - Timestamp: ${record.timestamp}`);
    console.log(` - Bump: ${record.bump}`);
    // Assertions
    if (!record.authority.equals(authority.publicKey))
        throw new Error("Authority mismatch");
    if (!record.sessionId.equals(sessionId))
        throw new Error("Session ID mismatch");
    if (record.coords.join(",") !== coords.join(","))
        throw new Error("Coords mismatch");
    if (!record.merkleRoot.equals(merkleRoot))
        throw new Error("Merkle root mismatch");
    console.log("-> Test 2 Passed.");
    // Test 3: Solana Pay Integration & Gateway QR request
    console.log("\n[Test 3] Verifying Solana Pay Gateway QR encoding...");
    const rewardDetails = payGateway.generatePaymentRequest(authority.publicKey, // recipient node
    0.05, // 0.05 USDC/USDG reward
    sessionId);
    console.log(`Solana Pay Reference: ${rewardDetails.reference.toBase58()}`);
    console.log(`Memo details: ${rewardDetails.memo}`);
    console.log(`Message details: ${rewardDetails.message}`);
    if (!rewardDetails.reference)
        throw new Error("Solana Pay request failed");
    console.log("-> Test 3 Passed.");
    console.log("\n--- All tests completed successfully! ---");
}
runTests().catch((err) => {
    console.error("Test execution failed:", err);
    process.exit(1);
});
