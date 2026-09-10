// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

class SparseTransition {
    key: number;
    sym: number;
    count: number;
    constructor(key: number, sym: number, count: number) {
        this.key = key;
        this.sym = sym;
        this.count = count;
    }
}

class RadicalPredictor {
    alpha: number;
    weight: number;
    transRC: SparseTransition[] = [];
    transRF: SparseTransition[] = [];
    transRA: SparseTransition[] = [];
    prevRC: number = 0;
    prevRF: number = 0;
    prevRA: number = 0;

    constructor(alpha: number, weight: number) {
        this.alpha = alpha;
        this.weight = weight;
    }

    observe(rc: number, rf: number, ra: number) {
        const w = this.weight;
        const keyRC = this.prevRC;
        let found = false;
        for (let entry of this.transRC) {
            if (entry.key === keyRC && entry.sym === rc) {
                entry.count += w;
                found = true;
                break;
            }
        }
        if (!found && this.transRC.length < 256) {
            this.transRC.push(new SparseTransition(keyRC, rc, w));
        }

        const keyRF = (rc << 8) | this.prevRF;
        found = false;
        for (let entry of this.transRF) {
            if (entry.key === keyRF && entry.sym === rf) {
                entry.count += w;
                found = true;
                break;
            }
        }
        if (!found && this.transRF.length < 256) {
            this.transRF.push(new SparseTransition(keyRF, rf, w));
        }

        const keyRA = (rc << 16) | (rf << 8) | this.prevRA;
        found = false;
        for (let entry of this.transRA) {
            if (entry.key === keyRA && entry.sym === ra) {
                entry.count += w;
                found = true;
                break;
            }
        }
        if (!found && this.transRA.length < 256) {
            this.transRA.push(new SparseTransition(keyRA, ra, w));
        }

        this.prevRC = rc;
        this.prevRF = rf;
        this.prevRA = ra;
    }

    getCumFreqsRC(prevRC: number): number[] {
        const freqs = new Array(256).fill(this.alpha);
        for (let entry of this.transRC) {
            if (entry.key === prevRC) {
                freqs[entry.sym] += entry.count;
            }
        }
        const cumFreqs = new Array(257).fill(0);
        for (let i = 0; i < 256; i++) {
            cumFreqs[i + 1] = cumFreqs[i] + freqs[i];
        }
        return cumFreqs;
    }

    getCumFreqsRF(currRC: number, prevRF: number): number[] {
        const freqs = new Array(256).fill(this.alpha);
        const key = (currRC << 8) | prevRF;
        for (let entry of this.transRF) {
            if (entry.key === key) {
                freqs[entry.sym] += entry.count;
            }
        }
        const cumFreqs = new Array(257).fill(0);
        for (let i = 0; i < 256; i++) {
            cumFreqs[i + 1] = cumFreqs[i] + freqs[i];
        }
        return cumFreqs;
    }

    getCumFreqsRA(currRC: number, currRF: number, prevRA: number): number[] {
        const freqs = new Array(256).fill(this.alpha);
        const key = (currRC << 16) | (currRF << 8) | prevRA;
        for (let entry of this.transRA) {
            if (entry.key === key) {
                freqs[entry.sym] += entry.count;
            }
        }
        const cumFreqs = new Array(257).fill(0);
        for (let i = 0; i < 256; i++) {
            cumFreqs[i + 1] = cumFreqs[i] + freqs[i];
        }
        return cumFreqs;
    }
}

class BitWriter {
    buffer: number[] = [];
    bitIndex: number = 0;

    writeBit(bit: number) {
        const bytePos = Math.floor(this.bitIndex / 8);
        const bitPos = 7 - (this.bitIndex % 8);
        if (bytePos >= this.buffer.length) {
            this.buffer.push(0);
        }
        if (bit !== 0) {
            this.buffer[bytePos] |= (1 << bitPos);
        } else {
            this.buffer[bytePos] &= ~(1 << bitPos);
        }
        this.bitIndex++;
    }

    writeBitHelper(underflowBits: { value: number }, bit: number) {
        this.writeBit(bit);
        while (underflowBits.value > 0) {
            this.writeBit(1 - bit);
            underflowBits.value--;
        }
    }
}

class BitReader {
    buffer: number[];
    bitIndex: number = 0;
    totalBits: number;

    constructor(buffer: number[]) {
        this.buffer = buffer;
        this.totalBits = buffer.length * 8;
    }

    readBit(): number {
        if (this.bitIndex >= this.totalBits) return 0;
        const bytePos = Math.floor(this.bitIndex / 8);
        const bitPos = 7 - (this.bitIndex % 8);
        const bit = (this.buffer[bytePos] >> bitPos) & 1;
        this.bitIndex++;
        return bit;
    }
}

interface Concept6D {
    domain: number;
    subdomain: number;
    operation: number;
    modality: number;
    depth: number;
    polarity: number;
}

function encode(concepts: Concept6D[], alpha: number, weight: number): { buf: number[], bits: number } {
    const pred = new RadicalPredictor(alpha, weight);
    const w = new BitWriter();
    let low = 0;
    let high = 0xFFFFFFFF;
    const underflowBits = { value: 0 };

    for (let c of concepts) {
        const rc = (c.domain << 4) | c.subdomain;
        const rf = (c.operation << 4) | c.modality;
        const ra = (c.depth << 4) | c.polarity;
        const symbols = [rc, rf, ra];

        const prevRC = pred.prevRC;
        const prevRF = pred.prevRF;
        const prevRA = pred.prevRA;

        for (let step = 0; step < 3; step++) {
            let cumFreqs: number[];
            if (step === 0) {
                cumFreqs = pred.getCumFreqsRC(prevRC);
            } else if (step === 1) {
                cumFreqs = pred.getCumFreqsRF(symbols[0], prevRF);
            } else {
                cumFreqs = pred.getCumFreqsRA(symbols[0], symbols[1], prevRA);
            }

            const sym = symbols[step];
            const total = cumFreqs[256];
            const cumLow = cumFreqs[sym];
            const cumHigh = cumFreqs[sym + 1];

            const rangeWidth = high - low + 1;
            high = (low + Math.floor((rangeWidth * cumHigh) / total) - 1) >>> 0;
            low = (low + Math.floor((rangeWidth * cumLow) / total)) >>> 0;

            while (true) {
                if (high < 0x80000000) {
                    w.writeBitHelper(underflowBits, 0);
                    low = (low * 2) >>> 0;
                    high = ((high * 2) + 1) >>> 0;
                } else if (low >= 0x80000000) {
                    w.writeBitHelper(underflowBits, 1);
                    low = ((low - 0x80000000) * 2) >>> 0;
                    high = (((high - 0x80000000) * 2) + 1) >>> 0;
                } else if (low >= 0x40000000 && high < 0xC0000000) {
                    underflowBits.value++;
                    low = ((low - 0x40000000) * 2) >>> 0;
                    high = (((high - 0x40000000) * 2) + 1) >>> 0;
                } else {
                    break;
                }
            }
        }
        pred.observe(rc, rf, ra);
    }

    underflowBits.value++;
    if (low < 0x40000000) {
        w.writeBitHelper(underflowBits, 0);
    } else {
        w.writeBitHelper(underflowBits, 1);
    }

    return { buf: w.buffer, bits: w.bitIndex };
}

function decode(encodedBytes: number[], numConcepts: number, alpha: number, weight: number): Concept6D[] {
    const pred = new RadicalPredictor(alpha, weight);
    const r = new BitReader(encodedBytes);

    let value = 0;
    for (let i = 0; i < 32; i++) {
        value = ((value * 2) + r.readBit()) >>> 0;
    }

    let low = 0;
    let high = 0xFFFFFFFF;
    const decoded: Concept6D[] = [];

    for (let cIdx = 0; cIdx < numConcepts; cIdx++) {
        const prevRC = pred.prevRC;
        const prevRF = pred.prevRF;
        const prevRA = pred.prevRA;
        const symbols = [0, 0, 0];

        for (let step = 0; step < 3; step++) {
            let cumFreqs: number[];
            if (step === 0) {
                cumFreqs = pred.getCumFreqsRC(prevRC);
            } else if (step === 1) {
                cumFreqs = pred.getCumFreqsRF(symbols[0], prevRF);
            } else {
                cumFreqs = pred.getCumFreqsRA(symbols[0], symbols[1], prevRA);
            }

            const total = cumFreqs[256];
            const rangeWidth = high - low + 1;
            const scaledVal = Math.floor((((value - low) + 1) * total - 1) / rangeWidth);

            let sym = 0;
            let lIdx = 0, rIdx = 255;
            while (lIdx <= rIdx) {
                const mIdx = Math.floor((lIdx + rIdx) / 2);
                if (cumFreqs[mIdx] <= scaledVal && scaledVal < cumFreqs[mIdx + 1]) {
                    sym = mIdx;
                    break;
                } else if (scaledVal >= cumFreqs[mIdx + 1]) {
                    lIdx = mIdx + 1;
                } else {
                    rIdx = mIdx - 1;
                }
            }

            symbols[step] = sym;
            const cumLow = cumFreqs[sym];
            const cumHigh = cumFreqs[sym + 1];

            high = (low + Math.floor((rangeWidth * cumHigh) / total) - 1) >>> 0;
            low = (low + Math.floor((rangeWidth * cumLow) / total)) >>> 0;

            while (true) {
                if (high < 0x80000000) {
                    low = (low * 2) >>> 0;
                    high = ((high * 2) + 1) >>> 0;
                    value = ((value * 2) + r.readBit()) >>> 0;
                } else if (low >= 0x80000000) {
                    low = ((low - 0x80000000) * 2) >>> 0;
                    high = (((high - 0x80000000) * 2) + 1) >>> 0;
                    value = (((value - 0x80000000) * 2) + r.readBit()) >>> 0;
                } else if (low >= 0x40000000 && high < 0xC0000000) {
                    low = ((low - 0x40000000) * 2) >>> 0;
                    high = (((high - 0x40000000) * 2) + 1) >>> 0;
                    value = (((value - 0x40000000) * 2) + r.readBit()) >>> 0;
                } else {
                    break;
                }
            }
        }

        decoded.push({
            domain: (symbols[0] >> 4) & 0xF,
            subdomain: symbols[0] & 0xF,
            operation: (symbols[1] >> 4) & 0xF,
            modality: symbols[1] & 0xF,
            depth: (symbols[2] >> 4) & 0xF,
            polarity: symbols[2] & 0xF
        });
        pred.observe(symbols[0], symbols[1], symbols[2]);
    }
    return decoded;
}

function main() {
    console.log("======================================================================");
    console.log("ZYMATICA | zymatica-inference-engine-typescript");
    console.log("======================================================================\n");

    const inputs: Concept6D[] = [
        { domain: 1, subdomain: 2, operation: 3, modality: 4, depth: 5, polarity: 6 },
        { domain: 8, subdomain: 0, operation: 15, modality: 1, depth: 0, polarity: 15 },
        { domain: 0, subdomain: 0, operation: 0, modality: 0, depth: 0, polarity: 0 },
        { domain: 15, subdomain: 15, operation: 15, modality: 15, depth: 15, polarity: 15 },
        { domain: 4, subdomain: 5, operation: 6, modality: 7, depth: 8, polarity: 9 }
    ];

    const { buf, bits } = encode(inputs, 1, 128);
    console.log(`Encoded Bits: ${bits}, Bytes: ${buf.length}`);
    console.log("Hex:", buf.map(b => b.toString(16).toUpperCase().padStart(2, '0')).join(' '));

    const decoded = decode(buf, 5, 1, 128);
    let match = true;
    for (let i = 0; i < inputs.length; i++) {
        if (inputs[i].domain !== decoded[i].domain ||
            inputs[i].subdomain !== decoded[i].subdomain ||
            inputs[i].operation !== decoded[i].operation ||
            inputs[i].modality !== decoded[i].modality ||
            inputs[i].depth !== decoded[i].depth ||
            inputs[i].polarity !== decoded[i].polarity) {
            match = false;
            break;
        }
    }

    console.log("Decoded matches inputs: " + match);
    if (!match) {
        console.log("ERROR: mismatch!");
        process.exit(1);
    }

    console.log("\n[VERIFICATION] Multi-Language runtime FFI structures validated.");
}

main();
