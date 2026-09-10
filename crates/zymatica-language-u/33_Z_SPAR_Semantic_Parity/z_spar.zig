//! Class 33: Z-SPAR (Zymatica Semantic Parity & Repair) - Zig Implementation
//! Author: Danny Bouldiez | Codebase by Devs One
//! License: SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0

const std = @import("std");

pub const GF16_EXP = [32]u8{
    1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1,
    2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2,
};

pub const GF16_LOG = [16]u8{
    0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12,
};

pub inline fn gf16Add(a: u8, b: u8) u8 {
    return (a ^ b) & 0x0F;
}

pub inline fn gf16Mul(a: u8, b: u8) u8 {
    const ma = a & 0x0F;
    const mb = b & 0x0F;
    if (ma == 0 or mb == 0) return 0;
    const idx = (@as(usize, GF16_LOG[ma]) + @as(usize, GF16_LOG[mb])) % 15;
    return GF16_EXP[idx];
}

pub inline fn gf16Div(a: u8, b: u8) u8 {
    const ma = a & 0x0F;
    const mb = b & 0x0F;
    if (mb == 0 or ma == 0) return 0;
    const diff = @as(i32, GF16_LOG[ma]) - @as(i32, GF16_LOG[mb]) + 15;
    return GF16_EXP[@as(usize, @intCast(@mod(diff, 15)))];
}

pub inline fn gf16Power(a: u8, exp: usize) u8 {
    const ma = a & 0x0F;
    if (ma == 0) return 0;
    const idx = (@as(usize, GF16_LOG[ma]) * exp) % 15;
    return GF16_EXP[idx];
}

pub fn encodeZSpar8D(state: [8]u8) [4]u8 {
    var parity: [4]u8 = [_]u8{0} ** 4;
    for (0..4) |j| {
        const root = GF16_EXP[j + 1];
        var sum: u8 = 0;
        for (0..8) |i| {
            const w = gf16Power(root, i + 1);
            sum = gf16Add(sum, gf16Mul(state[i], w));
        }
        parity[j] = sum;
    }
    return parity;
}

pub fn verifyAndRepairZSpar(reconstructed: [8]u8, parity: [4]u8, repaired: *[8]u8) bool {
    var syndromes: [4]u8 = [_]u8{0} ** 4;
    var all_zero: bool = true;

    for (0..4) |j| {
        const root = GF16_EXP[j + 1];
        var sum: u8 = 0;
        for (0..8) |i| {
            const w = gf16Power(root, i + 1);
            sum = gf16Add(sum, gf16Mul(reconstructed[i], w));
        }
        syndromes[j] = gf16Add(parity[j], sum);
        if (syndromes[j] != 0) all_zero = false;
    }

    repaired.* = reconstructed;
    if (all_zero) return true;

    // 1-error correction
    for (0..8) |target| {
        var cand_err: u8 = 0;
        var match: bool = true;
        for (0..4) |j| {
            const root = GF16_EXP[j + 1];
            const w = gf16Power(root, target + 1);
            const err = gf16Div(syndromes[j], w);
            if (j == 0) {
                cand_err = err;
            } else if (cand_err != err) {
                match = false;
                break;
            }
        }
        if (match and cand_err != 0) {
            repaired[target] = gf16Add(repaired[target], cand_err);
            return true;
        }
    }

    // 2-error correction
    for (0..8) |i1| {
        for ((i1 + 1)..8) |i2| {
            const r0 = GF16_EXP[1];
            const r1 = GF16_EXP[2];
            const a11 = gf16Power(r0, i1 + 1);
            const a12 = gf16Power(r0, i2 + 1);
            const a21 = gf16Power(r1, i1 + 1);
            const a22 = gf16Power(r1, i2 + 1);
            const det = gf16Add(gf16Mul(a11, a22), gf16Mul(a12, a21));
            if (det == 0) continue;

            const num1 = gf16Add(gf16Mul(a22, syndromes[0]), gf16Mul(a12, syndromes[1]));
            const num2 = gf16Add(gf16Mul(a11, syndromes[1]), gf16Mul(a21, syndromes[0]));
            const e1 = gf16Div(num1, det);
            const e2 = gf16Div(num2, det);

            const r2 = GF16_EXP[3];
            const r3 = GF16_EXP[4];
            const s2 = gf16Add(gf16Mul(gf16Power(r2, i1 + 1), e1), gf16Mul(gf16Power(r2, i2 + 1), e2));
            const s3 = gf16Add(gf16Mul(gf16Power(r3, i1 + 1), e1), gf16Mul(gf16Power(r3, i2 + 1), e2));

            if (s2 == syndromes[2] and s3 == syndromes[3]) {
                repaired[i1] = gf16Add(repaired[i1], e1);
                repaired[i2] = gf16Add(repaired[i2], e2);
                return true;
            }
        }
    }
    return false;
}
