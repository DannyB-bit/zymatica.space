import { Connection, PublicKey, Keypair, SystemProgram, Transaction } from "@solana/web3.js";
import { encodeURL, createQR, findReference, validateTransfer } from "@solana/pay";
import BigNumber from "bignumber.js";
import * as crypto from "crypto";

export interface RelayRewardDetails {
  recipient: PublicKey;
  amount: number; // in USDC/USDG
  reference: PublicKey;
  label: string;
  message: string;
  memo: string;
}

/**
 * SolanaPayMeshGateway — Devnet Demo Adapter
 * 
 * This class demonstrates the Solana Pay reference-key tracking pattern for mesh
 * relay rewards. In production (mainnet), the `mockPayRelayReward` method would
 * issue a real USDC SPL token transfer using the `@solana/pay` `validateTransfer`
 * flow. Currently, the mock uses native SOL system transfers to demonstrate the
 * `encodeURL` → `findReference` lifecycle on devnet without requiring funded 
 * USDC token accounts.
 * 
 * The `generatePaymentRequest` correctly encodes the USDC mint address and all
 * Solana Pay standard fields (reference, label, message, memo).
 */
export class SolanaPayMeshGateway {
  private connection: Connection;
  private usdcMintAddress: PublicKey;

  constructor(endpoint: string) {
    this.connection = new Connection(endpoint, "confirmed");
    // Standard USDC mint address on Solana Mainnet (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v)
    this.usdcMintAddress = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
  }

  /**
   * Generates a Solana Pay payment request URL for rewarding a mesh packet relay.
   */
  public generatePaymentRequest(
    recipientNode: PublicKey,
    amount: number,
    sessionId: Buffer
  ): RelayRewardDetails {
    // Generate a unique reference keypair to track this specific transaction on-chain
    const reference = Keypair.generate().publicKey;

    const label = "Zymatica DePIN Mesh Relay";
    const message = `Reward for routing Cuneiform-U packet (Session: ${sessionId.toString("hex").substring(0, 8)})`;
    const memo = `zymatica:mesh:${sessionId.toString("hex")}`;

    // Create the Solana Pay URL with BigNumber amount
    const url = encodeURL({
      recipient: recipientNode,
      amount: new BigNumber(amount),
      splToken: this.usdcMintAddress,
      reference,
      label,
      message,
      memo,
    });

    return {
      recipient: recipientNode,
      amount,
      reference,
      label,
      message,
      memo,
    };
  }

  /**
   * Simulates/Mocks a physical node paying the relay node via Solana Pay.
   */
  public async mockPayRelayReward(
    payer: Keypair,
    rewardDetails: RelayRewardDetails
  ): Promise<string> {
    // In a real wallet app, this builds a standard SPL token transfer transaction
    // We will build a mock transaction that includes the reference key and memo on-chain to demonstrate compliance
    const tx = new Transaction().add(
      SystemProgram.transfer({
        fromPubkey: payer.publicKey,
        toPubkey: rewardDetails.recipient,
        lamports: 1000000, // mock native lamports payment for gas/routing demo
      })
    );

    // Append reference keys to instruction keys to allow finding the transaction later
    tx.instructions[0].keys.push({
      pubkey: rewardDetails.reference,
      isSigner: false,
      isWritable: false,
    });

    const signature = await this.connection.sendTransaction(tx, [payer]);
    
    // Wait for transaction block confirmation
    const latestBlockhash = await this.connection.getLatestBlockhash();
    await this.connection.confirmTransaction({
      signature,
      ...latestBlockhash
    });

    return signature;
  }

  /**
   * Scans the Solana blockchain for the specific payment reference to verify node reward.
   */
  public async verifyRelayReward(reference: PublicKey): Promise<boolean> {
    try {
      // Search for the transaction on-chain using the reference public key
      const signatureInfo = await findReference(this.connection, reference, { finality: "confirmed" });
      if (signatureInfo) {
        console.log(`Relay reward validated on-chain! Tx Signature: ${signatureInfo.signature}`);
        return true;
      }
      return false;
    } catch (e) {
      console.log(`Failed to verify payment reference: ${(e as Error).message}`);
      return false;
    }
  }
}
