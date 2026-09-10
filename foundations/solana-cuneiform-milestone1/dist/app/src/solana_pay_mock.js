import { Connection, PublicKey, Keypair, SystemProgram, Transaction } from "@solana/web3.js";
import { encodeURL, findReference } from "@solana/pay";
export class SolanaPayMeshGateway {
    constructor(endpoint) {
        this.connection = new Connection(endpoint, "confirmed");
        // Standard USDC mint address on Solana Mainnet (EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v)
        this.usdcMintAddress = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    }
    /**
     * Generates a Solana Pay payment request URL for rewarding a mesh packet relay.
     */
    generatePaymentRequest(recipientNode, amount, sessionId) {
        // Generate a unique reference keypair to track this specific transaction on-chain
        const reference = Keypair.generate().publicKey;
        const label = "Zymatica DePIN Mesh Relay";
        const message = `Reward for routing Cuneiform-U packet (Session: ${sessionId.toString("hex").substring(0, 8)})`;
        const memo = `zymatica:mesh:${sessionId.toString("hex")}`;
        // Create the Solana Pay URL
        const url = encodeURL({
            recipient: recipientNode,
            amount: amount, // amount in decimal representation
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
    async mockPayRelayReward(payer, rewardDetails) {
        // In a real wallet app, this builds a standard SPL token transfer transaction
        // We will build a mock transaction that includes the reference key and memo on-chain to demonstrate compliance
        const tx = new Transaction().add(SystemProgram.transfer({
            fromPubkey: payer.publicKey,
            toPubkey: rewardDetails.recipient,
            lamports: 1000000, // mock native lamports payment for gas/routing demo
        }));
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
    async verifyRelayReward(reference) {
        try {
            // Search for the transaction on-chain using the reference public key
            const signatureInfo = await findReference(this.connection, reference, { finality: "confirmed" });
            if (signatureInfo) {
                console.log(`Relay reward validated on-chain! Tx Signature: ${signatureInfo.signature}`);
                return true;
            }
            return false;
        }
        catch (e) {
            console.log(`Failed to verify payment reference: ${e.message}`);
            return false;
        }
    }
}
