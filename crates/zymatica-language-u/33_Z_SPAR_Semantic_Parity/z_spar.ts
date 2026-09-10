/**
 * Class 33: Z-SPAR (Zymatica Semantic Parity & Repair) - TypeScript Implementation
 * Author: Danny Bouldiez | Codebase by Devs One
 * License: SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
 */

export class GF16 {
    static readonly EXP: number[] = [
        1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1,
        2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2
    ];

    static readonly LOG: number[] = [
        0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12
    ];

    static add(a: number, b: number): number {
        return (a ^ b) & 0x0F;
    }

    static mul(a: number, b: number): number {
        a &= 0x0F;
        b &= 0x0F;
        if (a === 0 || b === 0) return 0;
        return this.EXP[(this.LOG[a] + this.LOG[b]) % 15];
    }

    static div(a: number, b: number): number {
        a &= 0x0F;
        b &= 0x0F;
        if (b === 0) throw new Error("GF(16) Division by Zero");
        if (a === 0) return 0;
        return this.EXP[(this.LOG[a] - this.LOG[b] + 15) % 15];
    }

    static power(a: number, exp: number): number {
        a &= 0x0F;
        if (a === 0) return 0;
        return this.EXP[(this.LOG[a] * exp) % 15];
    }
}

export interface ZSparResult {
    status: 'EXACT_MATCH' | 'REPAIRED_1_AXIS' | 'REPAIRED_2_AXIS' | 'UNCORRECTABLE_DIVERGENCE';
    repairedState: number[];
    driftedAxes: number[];
}

export class ZSparEngine {
    static encode8D(state: number[]): number[] {
        const parity: number[] = [0, 0, 0, 0];
        for (let j = 0; j < 4; j++) {
            const root = GF16.EXP[j + 1];
            let sum = 0;
            for (let i = 0; i < 8; i++) {
                const w = GF16.power(root, i + 1);
                sum = GF16.add(sum, GF16.mul(state[i], w));
            }
            parity[j] = sum;
        }
        return parity;
    }

    static verifyAndRepair(reconstructed: number[], expectedParity: number[]): ZSparResult {
        const syndromes: number[] = [0, 0, 0, 0];
        let allZero = true;

        for (let j = 0; j < 4; j++) {
            const root = GF16.EXP[j + 1];
            let sum = 0;
            for (let i = 0; i < 8; i++) {
                const w = GF16.power(root, i + 1);
                sum = GF16.add(sum, GF16.mul(reconstructed[i], w));
            }
            syndromes[j] = GF16.add(expectedParity[j], sum);
            if (syndromes[j] !== 0) allZero = false;
        }

        if (allZero) {
            return { status: 'EXACT_MATCH', repairedState: [...reconstructed], driftedAxes: [] };
        }

        // 1-error correction
        for (let target = 0; target < 8; target++) {
            let candidateErr: number | null = null;
            let match = true;
            for (let j = 0; j < 4; j++) {
                const root = GF16.EXP[j + 1];
                const w = GF16.power(root, target + 1);
                const err = GF16.div(syndromes[j], w);
                if (candidateErr === null) candidateErr = err;
                else if (candidateErr !== err) { match = false; break; }
            }
            if (match && candidateErr !== null && candidateErr !== 0) {
                const repaired = [...reconstructed];
                repaired[target] = GF16.add(repaired[target], candidateErr);
                return { status: 'REPAIRED_1_AXIS', repairedState: repaired, driftedAxes: [target] };
            }
        }

        // 2-error correction
        for (let i1 = 0; i1 < 8; i1++) {
            for (let i2 = i1 + 1; i2 < 8; i2++) {
                const r0 = GF16.EXP[1], r1 = GF16.EXP[2];
                const a11 = GF16.power(r0, i1 + 1);
                const a12 = GF16.power(r0, i2 + 1);
                const a21 = GF16.power(r1, i1 + 1);
                const a22 = GF16.power(r1, i2 + 1);
                const det = GF16.add(GF16.mul(a11, a22), GF16.mul(a12, a21));
                if (det === 0) continue;

                const num1 = GF16.add(GF16.mul(a22, syndromes[0]), GF16.mul(a12, syndromes[1]));
                const num2 = GF16.add(GF16.mul(a11, syndromes[1]), GF16.mul(a21, syndromes[0]));
                const e1 = GF16.div(num1, det);
                const e2 = GF16.div(num2, det);

                const r2 = GF16.EXP[3], r3 = GF16.EXP[4];
                const s2 = GF16.add(GF16.mul(GF16.power(r2, i1 + 1), e1), GF16.mul(GF16.power(r2, i2 + 1), e2));
                const s3 = GF16.add(GF16.mul(GF16.power(r3, i1 + 1), e1), GF16.mul(GF16.power(r3, i2 + 1), e2));

                if (s2 === syndromes[2] && s3 === syndromes[3]) {
                    const repaired = [...reconstructed];
                    repaired[i1] = GF16.add(repaired[i1], e1);
                    repaired[i2] = GF16.add(repaired[i2], e2);
                    return { status: 'REPAIRED_2_AXIS', repairedState: repaired, driftedAxes: [i1, i2] };
                }
            }
        }

        return { status: 'UNCORRECTABLE_DIVERGENCE', repairedState: [...reconstructed], driftedAxes: [] };
    }
}
