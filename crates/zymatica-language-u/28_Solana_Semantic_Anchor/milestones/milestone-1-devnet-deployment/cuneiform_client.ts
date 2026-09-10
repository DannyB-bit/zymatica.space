import {
  Connection,
  PublicKey,
  Keypair,
  SystemProgram,
  TransactionInstruction,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import * as crypto from "crypto";

export interface CoordinateRecord {
  authority: PublicKey;
  sessionId: Buffer;
  coords: number[];
  merkleRoot: Buffer;
  timestamp: number;
  bump: number;
}

export interface ProgramState {
  admin: PublicKey;
  treasury: PublicKey;
  feeLamports: bigint;
}

export class CuneiformClient {
  public connection: Connection;
  public programId: PublicKey;

  constructor(endpoint: string, programIdStr: string) {
    this.connection = new Connection(endpoint, "confirmed");
    this.programId = new PublicKey(programIdStr);
  }

  /**
   * Helper to derive the PDA for a given authority and session ID.
   */
  public deriveRecordAddress(authority: PublicKey, sessionId: Buffer): [PublicKey, number] {
    if (sessionId.length !== 16) {
      throw new Error("Session ID must be exactly 16 bytes.");
    }
    return PublicKey.findProgramAddressSync(
      [Buffer.from("cuneiform"), authority.toBuffer(), sessionId],
      this.programId
    );
  }

  /**
   * Helper to derive the global program state PDA.
   */
  public deriveProgramStateAddress(): [PublicKey, number] {
    return PublicKey.findProgramAddressSync(
      [Buffer.from("state")],
      this.programId
    );
  }

  /**
   * Calculates the 8-byte Anchor discriminator for a given prefix and name.
   */
  private getDiscriminator(prefix: string, name: string): Buffer {
    const hash = crypto.createHash("sha256").update(`${prefix}:${name}`).digest();
    return hash.subarray(0, 8);
  }

  /**
   * Initializes the global program state configuration.
   */
  public async initializeProgram(
    admin: Keypair,
    treasury: PublicKey,
    feeLamports: bigint
  ): Promise<string> {
    const [statePda] = this.deriveProgramStateAddress();
    const ixDiscriminator = this.getDiscriminator("global", "initialize_program");

    // Serialize parameters: treasury (32B) + fee_lamports (8B LE)
    const feeBuffer = Buffer.alloc(8);
    feeBuffer.writeBigUInt64LE(feeLamports);

    const data = Buffer.concat([
      ixDiscriminator,
      treasury.toBuffer(),
      feeBuffer,
    ]);

    const instruction = new TransactionInstruction({
      keys: [
        { pubkey: statePda, isSigner: false, isWritable: true },
        { pubkey: admin.publicKey, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId: this.programId,
      data,
    });

    const tx = new Transaction().add(instruction);
    return await sendAndConfirmTransaction(this.connection, tx, [admin]);
  }

  /**
   * Updates the global program configuration state.
   */
  public async updateProgramState(
    admin: Keypair,
    newTreasury?: PublicKey,
    newFeeLamports?: bigint
  ): Promise<string> {
    const [statePda] = this.deriveProgramStateAddress();
    const ixDiscriminator = this.getDiscriminator("global", "update_program_state");

    // Option flags serialization: Option<Pubkey> (1B flag + 32B), Option<u64> (1B flag + 8B)
    const treasuryFlag = newTreasury ? Buffer.from([1]) : Buffer.from([0]);
    const treasuryBytes = newTreasury ? newTreasury.toBuffer() : Buffer.alloc(0);

    const feeFlag = newFeeLamports !== undefined ? Buffer.from([1]) : Buffer.from([0]);
    const feeBytes = Buffer.alloc(newFeeLamports !== undefined ? 8 : 0);
    if (newFeeLamports !== undefined) {
      feeBytes.writeBigUInt64LE(newFeeLamports);
    }

    const data = Buffer.concat([
      ixDiscriminator,
      treasuryFlag,
      treasuryBytes,
      feeFlag,
      feeBytes,
    ]);

    const instruction = new TransactionInstruction({
      keys: [
        { pubkey: statePda, isSigner: false, isWritable: true },
        { pubkey: admin.publicKey, isSigner: true, isWritable: false },
      ],
      programId: this.programId,
      data,
    });

    const tx = new Transaction().add(instruction);
    return await sendAndConfirmTransaction(this.connection, tx, [admin]);
  }

  /**
   * Registers a new Cuneiform-U coordinate record and forwards the protocol fee.
   */
  public async registerCoordinates(
    authority: Keypair,
    sessionId: Buffer,
    coords: number[],
    merkleRoot: Buffer,
    treasury: PublicKey
  ): Promise<string> {
    if (sessionId.length !== 16) throw new Error("Session ID must be 16 bytes.");
    if (coords.length !== 6) throw new Error("Coordinates must be exactly 6 elements.");
    if (merkleRoot.length !== 32) throw new Error("Merkle root must be 32 bytes.");

    const [recordPda] = this.deriveRecordAddress(authority.publicKey, sessionId);
    const [statePda] = this.deriveProgramStateAddress();

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
        { pubkey: statePda, isSigner: false, isWritable: false },
        { pubkey: treasury, isSigner: false, isWritable: true },
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
   * Updates an existing coordinate record on Solana (bypasses fee).
   */
  public async updateCoordinates(
    authority: Keypair,
    sessionId: Buffer,
    coords: number[],
    merkleRoot: Buffer
  ): Promise<string> {
    if (sessionId.length !== 16) throw new Error("Session ID must be 16 bytes.");
    if (coords.length !== 6) throw new Error("Coordinates must be exactly 6 elements.");
    if (merkleRoot.length !== 32) throw new Error("Merkle root must be 32 bytes.");

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
  public async fetchRecord(authority: PublicKey, sessionId: Buffer): Promise<CoordinateRecord | null> {
    const [recordPda] = this.deriveRecordAddress(authority, sessionId);
    const info = await this.connection.getAccountInfo(recordPda);
    
    if (!info) return null;

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

  /**
   * Fetches and deserializes the global ProgramState config.
   */
  public async fetchProgramState(): Promise<ProgramState | null> {
    const [statePda] = this.deriveProgramStateAddress();
    const info = await this.connection.getAccountInfo(statePda);

    if (!info) return null;

    const data = info.data;
    if (data.length < 80) {
      throw new Error("Invalid state account length. Expected at least 80 bytes.");
    }

    const expectedDiscriminator = this.getDiscriminator("account", "ProgramState");
    const accountDiscriminator = data.subarray(0, 8);
    if (!accountDiscriminator.equals(expectedDiscriminator)) {
      throw new Error("State account discriminator mismatch.");
    }

    const admin = new PublicKey(data.subarray(8, 40));
    const treasury = new PublicKey(data.subarray(40, 72));
    const feeLamports = data.readBigUInt64LE(72);

    return {
      admin,
      treasury,
      feeLamports,
    };
  }
}
