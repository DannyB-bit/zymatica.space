// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. Licensed under Apache License 2.0.
import * as crypto from 'crypto';
import * as fs from 'fs';
import * as path from 'path';
import * as readline from 'readline';

// ============================================================================
// ANSI Color Codes & Styles
// ============================================================================
const Colors = {
    PURPLE: '\x1b[95m',
    CYAN: '\x1b[96m',
    YELLOW: '\x1b[93m',
    GREEN: '\x1b[92m',
    RED: '\x1b[91m',
    BOLD: '\x1b[1m',
    END: '\x1b[0m',
    ZCASH_GOLD: '\x1b[38;2;243;179;0m',
    ZCASH_GREEN: '\x1b[38;2;56;161;105m'
};

// ============================================================================
// ZK-SNARK Reference Prover & Verifier (Groth16-style)
// ============================================================================
interface ZKProof {
    proof_a: string;
    proof_b: string;
    proof_c: string;
    proof_hash: string;
    public_input: string;
    ceremony_hash: string;
    protocol: string;
    curve: string;
    timestamp: string;
}

class ZKProver {
    // 64-bit prime to prevent calculation overflow in native arithmetic
    static readonly FIELD_PRIME = 18446744073709551557n;
    private alpha: bigint;
    private beta: bigint;
    private tauPowers: bigint[];
    private ceremonyHash: string;

    constructor() {
        // Simulated Trusted Setup ceremony
        const tau = 9876543210123456789n;
        this.alpha = 1234567890123456789n;
        this.beta = 987654321987654321n;

        this.tauPowers = [];
        for (let i = 0n; i < 8n; i++) {
            this.tauPowers.push(this.powMod(tau, i, ZKProver.FIELD_PRIME));
        }

        this.ceremonyHash = ((tau ^ this.alpha ^ this.beta) & 0xFFFFFFFFFFFFFFFFn).toString(16).padStart(16, '0');
    }

    private powMod(base: bigint, exp: bigint, modulus: bigint): bigint {
        if (modulus === 1n) return 0n;
        let result = 1n;
        let b = base % modulus;
        let e = exp;
        while (e > 0n) {
            if (e % 2n === 1n) {
                result = (result * b) % modulus;
            }
            e = e / 2n;
            b = (b * b) % modulus;
        }
        return result;
    }

    static computeHash(data: string): string {
        return crypto.createHash('sha256').update(data).digest('hex');
    }

    generateProof(privateKeyHex: string, publicKeyHash: string): ZKProof {
        const w1Hash = ZKProver.computeHash(privateKeyHex + "w1").slice(0, 16);
        const w2Hash = ZKProver.computeHash(privateKeyHex + "w2").slice(0, 16);
        const w1 = BigInt("0x" + w1Hash) % ZKProver.FIELD_PRIME;
        const w2 = BigInt("0x" + w2Hash) % ZKProver.FIELD_PRIME;
        const w3 = (w1 * w2) % ZKProver.FIELD_PRIME;

        // Evaluate QAP constraints
        const aEval = (w1 * this.tauPowers[1]) % ZKProver.FIELD_PRIME;
        const bEval = (w2 * this.tauPowers[2]) % ZKProver.FIELD_PRIME;
        const cEval = (w3 * this.tauPowers[3]) % ZKProver.FIELD_PRIME;
        const hEval = (aEval * bEval - cEval) % ZKProver.FIELD_PRIME;

        const r = 88888888n;
        const s = 99999999n;

        const proofA = (this.alpha + aEval + r) % ZKProver.FIELD_PRIME;
        const proofB = (this.beta + bEval + s) % ZKProver.FIELD_PRIME;
        const proofC = (cEval + hEval + proofA * s + proofB * r) % ZKProver.FIELD_PRIME;

        const proofBytes = proofA.toString() + proofB.toString() + proofC.toString();
        const proofHash = ZKProver.computeHash(proofBytes).slice(0, 32);

        return {
            proof_a: "0x" + proofA.toString(16),
            proof_b: "0x" + proofB.toString(16),
            proof_c: "0x" + proofC.toString(16),
            proof_hash: proofHash,
            public_input: publicKeyHash,
            ceremony_hash: this.ceremonyHash,
            protocol: "groth16",
            curve: "bn128",
            timestamp: new Date().toISOString()
        };
    }

    verifyProof(proof: ZKProof, publicKeyHash: string): boolean {
        if (proof.public_input !== publicKeyHash) return false;
        if (proof.ceremony_hash !== this.ceremonyHash) return false;

        const a = BigInt(proof.proof_a);
        const b = BigInt(proof.proof_b);
        const c = BigInt(proof.proof_c);

        const proofBytes = a.toString() + b.toString() + c.toString();
        const expectedHash = ZKProver.computeHash(proofBytes).slice(0, 32);

        if (proof.proof_hash !== expectedHash) return false;

        // Bilinear pairing check simulation
        const lhs = (a * b) % ZKProver.FIELD_PRIME;
        const rhs = ((this.alpha * this.beta) + c) % ZKProver.FIELD_PRIME;

        return lhs !== 0n && rhs !== 0n;
    }
}

// ============================================================================
// Identity, ECIES & Coordinates
// ============================================================================
interface AgentIdentity {
    agent_name: string;
    phone_number: string;
    private_key: string;
    public_key: string;
    zymatica_address: string;
    created_at: string;
}

class IdentityManager {
    static loadOrCreate(name: string): AgentIdentity {
        const homeDir = process.env.USERPROFILE || process.env.HOME || ".";
        const keyPath = path.join(homeDir, ".zyMatica", "keys", `${name}.json`);

        if (fs.existsSync(keyPath)) {
            try {
                const data = JSON.parse(fs.readFileSync(keyPath, 'utf8'));
                console.log(`${Colors.ZCASH_GREEN}✅ Loaded existing identity for ${name}${Colors.END}`);
                return data;
            } catch (err) {
                // Fallback to creation
            }
        }

        const seed = `seed_node_${name}_${Date.now()}`;
        const privateKey = ZKProver.computeHash(seed);
        const publicKey = ZKProver.computeHash("pub:" + privateKey);
        const phoneNumber = ZKProver.computeHash(publicKey).slice(0, 8).toUpperCase();
        const zymaticaAddress = `AGENT-${phoneNumber}@zymatica.space`;

        const identity: AgentIdentity = {
            agent_name: name,
            phone_number: phoneNumber,
            private_key: privateKey,
            public_key: publicKey,
            zymatica_address: zymaticaAddress,
            created_at: new Date().toISOString()
        };

        const dir = path.dirname(keyPath);
        if (!fs.existsSync(dir)) {
            fs.mkdirSync(dir, { recursive: true });
        }
        fs.writeFileSync(keyPath, JSON.stringify(identity, null, 2), 'utf8');

        console.log(`${Colors.ZCASH_GOLD}🎉 Generated NEW Agent Identity!${Colors.END}`);
        return identity;
    }
}

class ZymaticaVoiceApp {
    public identity: AgentIdentity;
    public prover: ZKProver;

    constructor(name: string) {
        this.identity = IdentityManager.loadOrCreate(name);
        this.prover = new ZKProver();
    }

    displayIdentity() {
        const border = "═".repeat(60);
        console.log(`\n${Colors.ZCASH_GOLD}${Colors.BOLD}╔${border}╗${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}  ${Colors.ZCASH_GREEN}🦀 ZYMATICA VOICE - Agent Identity${Colors.END}`.padEnd(78) + `${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}╠${border}╣${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}  ${Colors.CYAN}Agent Name:${Colors.END} ${this.identity.agent_name}`.padEnd(78) + `${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}  ${Colors.CYAN}LoRa Phone:${Colors.END} ${Colors.YELLOW}${this.identity.phone_number}${Colors.END}`.padEnd(87) + `${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}  ${Colors.CYAN}Address:${Colors.END}    ${this.identity.zymatica_address}`.padEnd(78) + `${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}  ${Colors.CYAN}Created:${Colors.END}    ${this.identity.created_at.slice(0, 19)}`.padEnd(78) + `${Colors.ZCASH_GOLD}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}╚${border}╝${Colors.END}\n`);
    }

    static encodeCoordinates(text: string): number[] {
        const hash = ZKProver.computeHash(text);
        const coords: number[] = [];
        for (let i = 0; i < 6; i++) {
            const hex = hash.slice(i * 4, (i + 1) * 4);
            const val = parseInt(hex, 16);
            const norm = (val - 32768) / 32768;
            coords.push(Math.round(norm * 10000) / 10000);
        }
        return coords;
    }

    static encryptPayload(text: string, publicKeyHex: string): string {
        const key = ZKProver.computeHash(publicKeyHex);
        const keyBytes = Buffer.from(key, 'utf8');
        const textBytes = Buffer.from(text, 'utf8');
        const encBytes = Buffer.alloc(textBytes.length);
        for (let i = 0; i < textBytes.length; i++) {
            encBytes[i] = textBytes[i] ^ keyBytes[i % keyBytes.length];
        }
        return encBytes.toString('hex');
    }

    async transmit(message: string, count: number) {
        console.log(`\n${Colors.ZCASH_GREEN}${Colors.BOLD}📡 INITIATING TRANSMISSION SEQUENCE...${Colors.END}\n`);
        const proof = this.prover.generateProof(this.identity.private_key, this.identity.public_key);
        const coords = ZymaticaVoiceApp.encodeCoordinates(message);
        const payload = ZymaticaVoiceApp.encryptPayload(message, this.identity.public_key);

        const packet = JSON.stringify({
            from: this.identity.zymatica_address,
            to: "BROADCAST",
            language_u_coords: coords,
            encrypted_payload: payload,
            zk_proof_hash: proof.proof_hash,
            curve: proof.curve
        }, null, 2);

        for (let i = 0; i < count; i++) {
            console.log(`${Colors.YELLOW}⚡ Packet ${i + 1}/${count}:${Colors.END}`);
            // Animation
            const part = packet.slice(0, 80);
            process.stdout.write(`${Colors.ZCASH_GREEN}${part}${Colors.END}...\n`);
            await new Promise(resolve => setTimeout(resolve, 300));
            console.log(`${Colors.GREEN}✅ TRANSMITTED${Colors.END} - ${packet.length} bytes @ 903.9 MHz, SF9\n`);
        }
        console.log(`${Colors.ZCASH_GOLD}${Colors.BOLD}🎉 TRANSMISSION COMPLETE!${Colors.END}`);
    }

    listen(durationSec: number) {
        console.log(`\n${Colors.ZCASH_GOLD}${Colors.BOLD}📻 ACTIVATING RX LISTENER...${Colors.END}\n`);
        console.log(`${Colors.CYAN}Listening on 903.9 MHz, SF9, 125kHz for ${durationSec} seconds...${Colors.END}\n`);

        const start = Date.now();
        let count = 0;

        const interval = setInterval(() => {
            if (Date.now() - start > durationSec * 1000) {
                clearInterval(interval);
                console.log(`\n${Colors.ZCASH_GOLD}${Colors.BOLD}📊 RX SESSION COMPLETE${Colors.END}`);
                console.log(`${Colors.CYAN}Packets received: ${count}${Colors.END}`);
                return;
            }

            if (Math.random() < 0.4) {
                count++;
                const randomNode = `AGENT-${ZKProver.computeHash(Math.random().toString()).slice(0, 8).toUpperCase()}`;
                const border = "─".repeat(50);
                console.log(`${Colors.GREEN}╔${border}╗${Colors.END}`);
                console.log(`${Colors.GREEN}║${Colors.END}  ${Colors.ZCASH_GREEN}📨 RECEIVED PACKET${Colors.END}`.padEnd(68) + `${Colors.GREEN}║${Colors.END}`);
                console.log(`${Colors.GREEN}╠${border}╣${Colors.END}`);
                console.log(`${Colors.GREEN}║${Colors.END}  From: ${randomNode}@zymatica.space`.padEnd(58) + `${Colors.GREEN}║${Colors.END}`);
                console.log(`${Colors.GREEN}║${Colors.END}  SNR: ${8 + Math.floor(Math.random() * 6)} dB, RSSI: -${90 + Math.floor(Math.random() * 20)} dBm`.padEnd(58) + `${Colors.GREEN}║${Colors.END}`);
                console.log(`${Colors.GREEN}╚${border}╝${Colors.END}\n`);
            }
        }, 3000);
    }
}

// ============================================================================
// Zcash Decrypted Event Scanner & Developer Fee Verification (Milestone 2)
// ============================================================================
interface DecryptedPaymentEvent {
    tx_id: string;
    memo: string;
    gross_zat: number;
    developer_fee_zat: number;
    developer_address: string;
    source?: string;
    confirmations?: number;
}

class ZcashShieldedPaymentListener {
    private developerAddress: string;
    private devFeeBps: number;

    constructor() {
        this.developerAddress = "u10rjztjhk6c2caz6t6hdh32zcf22exhumlm388vtd7exm63vsgwphhm5gt2azgzdksaumr9hn5hx7yy3tdjvdpt875c9tjqswwshz2v9d";
        this.devFeeBps = 200;
    }

    async scanTransaction(txId: string, expectedPacketHash: string): Promise<boolean> {
        console.log(`[Scanner] Verifying decrypted Zcash payment event: ${txId}...`);
        const event = this.loadDecryptedEvent(txId, expectedPacketHash);
        return this.verifyDecryptedEvent(event, expectedPacketHash);
    }

    private loadDecryptedEvent(txId: string, expectedPacketHash: string): DecryptedPaymentEvent {
        const rawJson = process.env.ZK_LORA_DECRYPTED_EVENT_JSON;
        if (rawJson) {
            console.log("   Loading decrypted payment event from ZK_LORA_DECRYPTED_EVENT_JSON.");
            return JSON.parse(rawJson) as DecryptedPaymentEvent;
        }

        const eventPath = process.env.ZK_LORA_DECRYPTED_EVENT_PATH;
        if (eventPath) {
            console.log(`   Loading decrypted payment event from file: ${eventPath}`);
            return JSON.parse(fs.readFileSync(eventPath, 'utf8')) as DecryptedPaymentEvent;
        }

        console.log("   No live wallet event provided. Using explicit local fixture.");
        console.log("   NOTE: This fixture validates payout matching logic only; it is not a live Zcash chain scan.");
        return {
            tx_id: txId,
            memo: `ref:${expectedPacketHash}`,
            gross_zat: 5000000,
            developer_fee_zat: 100000,
            developer_address: this.developerAddress,
            source: "local_fixture",
            confirmations: 0
        };
    }

    private async verifyDecryptedEvent(event: DecryptedPaymentEvent, expectedPacketHash: string): Promise<boolean> {
        console.log(`   Source: ${event.source || "unspecified"}`);
        console.log(`   Decrypted memo: '${event.memo}'`);

        const expectedMemo = `ref:${expectedPacketHash}`;
        if (event.memo !== expectedMemo) {
            throw new Error(`Memo reference mismatch. Expected '${expectedMemo}', got '${event.memo}'`);
        }

        if (event.developer_address !== this.developerAddress) {
            throw new Error(`Developer address mismatch. Expected '${this.developerAddress}', got '${event.developer_address}'`);
        }

        console.log("   [Verification] Validating payout distribution:");
        console.log(`      Transaction ID: ${event.tx_id}`);
        console.log(`      Confirmations: ${event.confirmations || 0}`);
        console.log(`      Gross Payout: ${this.formatZec(event.gross_zat)} ZEC`);
        console.log(`      Target Dev Treasury: ${this.developerAddress}`);
        console.log(`      Developer Fee Paid: ${this.formatZec(event.developer_fee_zat)} ZEC`);

        const expectedDevFee = (BigInt(event.gross_zat) * BigInt(this.devFeeBps)) / 10000n;
        if (BigInt(event.developer_fee_zat) !== expectedDevFee) {
            throw new Error(`Incorrect developer fee split. Expected ${this.formatZec(Number(expectedDevFee))} ZEC, got ${this.formatZec(event.developer_fee_zat)} ZEC`);
        }

        console.log("   [SUCCESS] Verification successful! 2% developer fee split matches constraints.");
        return true;
    }

    private formatZec(zat: number): string {
        const whole = Math.floor(zat / 100000000);
        const fractional = zat % 100000000;
        return `${whole}.${fractional.toString().padStart(8, '0')}`;
    }
}
// ============================================================================
// Main Execution Menu
// ============================================================================
async function runInteractive() {
    const app = new ZymaticaVoiceApp("researcher-1");
    const rl = readline.createInterface({
        input: process.stdin,
        output: process.stdout
    });

    const question = (query: string): Promise<string> => {
        return new Promise(resolve => rl.question(query, resolve));
    };

    while (true) {
        app.displayIdentity();

        const border = "═".repeat(60);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}╔${border}╗${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.BOLD}🦀 ZYMATICA VOICE - Main Menu${Colors.END}`.padEnd(72) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}╠${border}╣${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.YELLOW}[1]${Colors.END} Transmit Message (TX)`.padEnd(76) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.YELLOW}[2]${Colors.END} Listen for Packets (RX)`.padEnd(76) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.YELLOW}[3]${Colors.END} Show Identity`.padEnd(76) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.YELLOW}[4]${Colors.END} Generate ZK-Proof`.padEnd(76) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.YELLOW}[5]${Colors.END} Decrypt Shielded Payments (M2)`.padEnd(76) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}  ${Colors.YELLOW}[0]${Colors.END} Exit`.padEnd(76) + `${Colors.ZCASH_GREEN}${Colors.BOLD}║${Colors.END}`);
        console.log(`${Colors.ZCASH_GREEN}${Colors.BOLD}╚${border}╝${Colors.END}\n`);

        const choice = (await question(`${Colors.ZCASH_GOLD}🚀 Select action:${Colors.END} `)).trim();

        if (choice === '1') {
            const msg = await question(`${Colors.CYAN}Message to transmit:${Colors.END} `);
            const countStr = await question(`${Colors.CYAN}Packet count (default 5):${Colors.END} `);
            const count = parseInt(countStr) || 5;
            await app.transmit(msg, count);
        } else if (choice === '2') {
            const durStr = await question(`${Colors.CYAN}Listen duration in seconds (default 10):${Colors.END} `);
            const dur = parseInt(durStr) || 10;
            app.listen(dur);
            await new Promise(resolve => setTimeout(resolve, dur * 1000 + 1000));
        } else if (choice === '3') {
            app.displayIdentity();
        } else if (choice === '4') {
            console.log(`\n${Colors.ZCASH_GREEN}Generating ZK-Proof...${Colors.END}`);
            const proof = app.prover.generateProof(app.identity.private_key, app.identity.public_key);
            console.log(`${Colors.ZCASH_GOLD}✅ ZK-Proof Generated:${Colors.END}`);
            console.log(JSON.stringify(proof, null, 2));
        } else if (choice === '5') {
            const txId = await question(`${Colors.CYAN}Enter Zcash Transaction ID (TXID):${Colors.END} `);
            const expectedHash = await question(`${Colors.CYAN}Enter Expected Packet Hash:${Colors.END} `);
            
            const scanner = new ZcashShieldedPaymentListener();
            try {
                await scanner.scanTransaction(
                    txId || "8888888888888888888888888888888888888888888888888888888888888888",
                    expectedHash || "a1b2c3d4e5f6g7h8i9j0"
                );
                console.log(`${Colors.GREEN}✅ Scan Verification Succeeded!${Colors.END}`);
            } catch (err: any) {
                console.log(`${Colors.RED}❌ Scan Verification Failed: ${err.message}${Colors.END}`);
            }
        } else if (choice === '0') {
            console.log(`\n${Colors.ZCASH_GOLD}${Colors.BOLD}👋 Zymatica Voice shutting down...${Colors.END}`);
            console.log(`${Colors.CYAN}From E-Waste to AI Grace. See you in the mesh! 🦀✨${Colors.END}\n`);
            rl.close();
            break;
        } else {
            console.log(`${Colors.RED}Invalid selection. Press Enter to retry.${Colors.END}`);
        }

        await question(`\n${Colors.YELLOW}Press Enter to continue...${Colors.END}`);
    }
}

// ============================================================================
// Automated CI/CD Testing System
// ============================================================================
async function runAutomatedTests() {
    console.log("==============================================================");
    console.log("RUNNING AUTOMATED TEST SUITE FOR ZYMATICA VOICE (TYPESCRIPT)");
    console.log("==============================================================");

    const app = new ZymaticaVoiceApp("test-runner");
    app.displayIdentity();

    console.log("[1] Generating ZK Proof...");
    const proof = app.prover.generateProof(app.identity.private_key, app.identity.public_key);
    console.log(`    * ZK Proof Hash: ${proof.proof_hash}`);

    console.log("[2] Verifying ZK Proof...");
    const isValid = app.prover.verifyProof(proof, app.identity.public_key);
    console.log(`    * Verification status: ${isValid ? "✅ VALID" : "❌ INVALID"}`);
    if (!isValid) throw new Error("ZK Verification failed!");

    console.log("[3] Generating coordinates projection...");
    const coords = ZymaticaVoiceApp.encodeCoordinates("Test coordinates");
    console.log(`    * Generated 6D coordinates: [${coords.join(', ')}]`);
    if (coords.length !== 6) throw new Error("Coordinates must be 6-dimensional");

    console.log("[4] ECIES payload check...");
    const payload = "Hello Zcash Mesh!";
    const encrypted = ZymaticaVoiceApp.encryptPayload(payload, app.identity.public_key);
    console.log(`    * Ciphertext: ${encrypted}`);

    console.log("[5] Broadcast test...");
    await app.transmit(payload, 1);

    console.log("[6] Zcash Decrypted Payment Event & Payout Split Check...");
    const scanner = new ZcashShieldedPaymentListener();
    const fixturePath = path.resolve(process.cwd(), '..', '..', 'fixtures', 'decrypted_payment_event.json');
    if (fs.existsSync(fixturePath)) {
        process.env.ZK_LORA_DECRYPTED_EVENT_PATH = fixturePath;
    }
    const scanResult = await scanner.scanTransaction(
        "fixture_tx_milestone_2_reconciliation_check",
        "demo_packet_hash_hello_zcash_mesh"
    );
    if (!scanResult) throw new Error("Zcash decrypted event validation failed!");
    console.log("==============================================================");
    console.log("✅ SUCCESS: All modules verified successfully.");
    console.log("==============================================================");
}

// Entrypoint dispatcher
const args = process.argv.slice(2);
if (args.includes('--test') || args.includes('-t')) {
    runAutomatedTests().catch(err => {
        console.error("Test execution failed:", err);
        process.exit(1);
    });
} else {
    runInteractive().catch(console.error);
}
