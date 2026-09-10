import { Connection, PublicKey, SystemProgram, TransactionInstruction, Transaction, sendAndConfirmTransaction, } from "@solana/web3.js";
import * as crypto from "crypto";
export class CuneiformClient {
    constructor(endpoint, programIdStr) {
        this.connection = new Connection(endpoint, "confirmed");
        this.programId = new PublicKey(programIdStr);
    }
    /**
     * Helper to derive the PDA for a given authority and session ID.
     */
    deriveRecordAddress(authority, sessionId) {
        if (sessionId.length !== 16) {
            throw new Error("Session ID must be exactly 16 bytes.");
        }
        return PublicKey.findProgramAddressSync([Buffer.from("cuneiform"), authority.toBuffer(), sessionId], this.programId);
    }
    /**
     * Calculates the 8-byte Anchor discriminator for a given prefix and name.
     */
    getDiscriminator(prefix, name) {
        const hash = crypto.createHash("sha256").update(`${prefix}:${name}`).digest();
        return hash.subarray(0, 8);
    }
    /**
     * Registers a new Cuneiform-U coordinate record on Solana.
     */
    async registerCoordinates(authority, sessionId, coords, merkleRoot) {
        if (sessionId.length !== 16)
            throw new Error("Session ID must be 16 bytes.");
        if (coords.length !== 6)
            throw new Error("Coordinates must be exactly 6 elements.");
        if (merkleRoot.length !== 32)
            throw new Error("Merkle root must be 32 bytes.");
        const [recordPda] = this.deriveRecordAddress(authority.publicKey, sessionId);
        // Compute instruction discriminator for "global:register_coordinates"
        const ixDiscriminator = this.getDiscriminator("global", "register_coordinates");
        // Serialize parameters: session_id (16B) + coords (6B) + merkle_root (32B)
        const data = Buffer.concat([
            ixDiscriminator,
            sessionId,
            Buffer.from(coords),
            merkleRoot,
        ]);
        const instruction = new TransactionInstruction({
            keys: [
                { pubkey: recordPda, isSigner: false, isWritable: true },
                { pubkey: authority.publicKey, isSigner: true, isWritable: true },
                { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
            ],
            programId: this.programId,
            data,
        });
        const tx = new Transaction().add(instruction);
        return await sendAndConfirmTransaction(this.connection, tx, [authority]);
    }
    /**
     * Updates an existing coordinate record on Solana.
     */
    async updateCoordinates(authority, sessionId, coords, merkleRoot) {
        if (sessionId.length !== 16)
            throw new Error("Session ID must be 16 bytes.");
        if (coords.length !== 6)
            throw new Error("Coordinates must be exactly 6 elements.");
        if (merkleRoot.length !== 32)
            throw new Error("Merkle root must be 32 bytes.");
        const [recordPda] = this.deriveRecordAddress(authority.publicKey, sessionId);
        // Compute instruction discriminator for "global:update_coordinates"
        const ixDiscriminator = this.getDiscriminator("global", "update_coordinates");
        // Serialize parameters: coords (6B) + merkle_root (32B)
        const data = Buffer.concat([
            ixDiscriminator,
            Buffer.from(coords),
            merkleRoot,
        ]);
        const instruction = new TransactionInstruction({
            keys: [
                { pubkey: recordPda, isSigner: false, isWritable: true },
                { pubkey: authority.publicKey, isSigner: true, isWritable: false },
            ],
            programId: this.programId,
            data,
        });
        const tx = new Transaction().add(instruction);
        return await sendAndConfirmTransaction(this.connection, tx, [authority]);
    }
    /**
     * Fetches and deserializes a coordinate record from Solana.
     */
    async fetchRecord(authority, sessionId) {
        const [recordPda] = this.deriveRecordAddress(authority, sessionId);
        const info = await this.connection.getAccountInfo(recordPda);
        if (!info)
            return null;
        const data = info.data;
        if (data.length < 103) {
            throw new Error("Invalid account data length. Expected at least 103 bytes.");
        }
        // Verify Anchor account discriminator
        const expectedDiscriminator = this.getDiscriminator("account", "CoordinateRecord");
        const accountDiscriminator = data.subarray(0, 8);
        if (!accountDiscriminator.equals(expectedDiscriminator)) {
            throw new Error("Account discriminator mismatch.");
        }
        const recAuthority = new PublicKey(data.subarray(8, 40));
        const recSessionId = data.subarray(40, 56);
        const recCoords = Array.from(data.subarray(56, 62));
        const recMerkleRoot = data.subarray(62, 94);
        // Read timestamp as 64-bit little endian integer
        const recTimestamp = Number(data.readBigInt64LE(94));
        const recBump = data[102];
        return {
            authority: recAuthority,
            sessionId: recSessionId,
            coords: recCoords,
            merkleRoot: recMerkleRoot,
            timestamp: recTimestamp,
            bump: recBump,
        };
    }
}
