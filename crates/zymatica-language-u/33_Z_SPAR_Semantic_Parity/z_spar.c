/**
 * Class 33: Z-SPAR (Zymatica Semantic Parity & Repair) - Pure C99 Implementation
 * Author: Danny Bouldiez | Codebase by Devs One
 * License: SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
 */

#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

static const uint8_t GF16_EXP[32] = {
    1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1,
    2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2
};

static const uint8_t GF16_LOG[16] = {
    0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12
};

static inline uint8_t gf16_add(uint8_t a, uint8_t b) {
    return (a ^ b) & 0x0F;
}

static inline uint8_t gf16_mul(uint8_t a, uint8_t b) {
    a &= 0x0F;
    b &= 0x0F;
    if (a == 0 || b == 0) return 0;
    return GF16_EXP[(GF16_LOG[a] + GF16_LOG[b]) % 15];
}

static inline uint8_t gf16_div(uint8_t a, uint8_t b) {
    a &= 0x0F;
    b &= 0x0F;
    if (b == 0) return 0;
    if (a == 0) return 0;
    return GF16_EXP[(GF16_LOG[a] - GF16_LOG[b] + 15) % 15];
}

static inline uint8_t gf16_power(uint8_t a, int exp) {
    a &= 0x0F;
    if (a == 0) return 0;
    return GF16_EXP[(GF16_LOG[a] * exp) % 15];
}

void z_spar_encode_8d(const uint8_t state[8], uint8_t parity[4]) {
    for (int j = 0; j < 4; ++j) {
        uint8_t root = GF16_EXP[j + 1];
        uint8_t sum = 0;
        for (int i = 0; i < 8; ++i) {
            uint8_t w = gf16_power(root, i + 1);
            sum = gf16_add(sum, gf16_mul(state[i], w));
        }
        parity[j] = sum;
    }
}

bool z_spar_verify_and_repair(const uint8_t reconstructed[8], const uint8_t parity[4], uint8_t repaired[8]) {
    uint8_t syndromes[4];
    bool all_zero = true;

    for (int j = 0; j < 4; ++j) {
        uint8_t root = GF16_EXP[j + 1];
        uint8_t sum = 0;
        for (int i = 0; i < 8; ++i) {
            uint8_t w = gf16_power(root, i + 1);
            sum = gf16_add(sum, gf16_mul(reconstructed[i], w));
        }
        syndromes[j] = gf16_add(parity[j], sum);
        if (syndromes[j] != 0) all_zero = false;
    }

    memcpy(repaired, reconstructed, 8);
    if (all_zero) return true; // Exact match

    // 1-error correction
    for (int target = 0; target < 8; ++target) {
        uint8_t cand_err = 0;
        bool match = true;
        for (int j = 0; j < 4; ++j) {
            uint8_t root = GF16_EXP[j + 1];
            uint8_t w = gf16_power(root, target + 1);
            uint8_t err = gf16_div(syndromes[j], w);
            if (j == 0) cand_err = err;
            else if (cand_err != err) { match = false; break; }
        }
        if (match && cand_err != 0) {
            repaired[target] = gf16_add(repaired[target], cand_err);
            return true;
        }
    }

    // 2-error correction
    for (int i1 = 0; i1 < 8; ++i1) {
        for (int i2 = i1 + 1; i2 < 8; ++i2) {
            uint8_t r0 = GF16_EXP[1], r1 = GF16_EXP[2];
            uint8_t a11 = gf16_power(r0, i1 + 1);
            uint8_t a12 = gf16_power(r0, i2 + 1);
            uint8_t a21 = gf16_power(r1, i1 + 1);
            uint8_t a22 = gf16_power(r1, i2 + 1);
            uint8_t det = gf16_add(gf16_mul(a11, a22), gf16_mul(a12, a21));
            if (det == 0) continue;

            uint8_t num1 = gf16_add(gf16_mul(a22, syndromes[0]), gf16_mul(a12, syndromes[1]));
            uint8_t num2 = gf16_add(gf16_mul(a11, syndromes[1]), gf16_mul(a21, syndromes[0]));
            uint8_t e1 = gf16_div(num1, det);
            uint8_t e2 = gf16_div(num2, det);

            uint8_t r2 = GF16_EXP[3], r3 = GF16_EXP[4];
            uint8_t s2 = gf16_add(gf16_mul(gf16_power(r2, i1 + 1), e1), gf16_mul(gf16_power(r2, i2 + 1), e2));
            uint8_t s3 = gf16_add(gf16_mul(gf16_power(r3, i1 + 1), e1), gf16_mul(gf16_power(r3, i2 + 1), e2));

            if (s2 == syndromes[2] && s3 == syndromes[3]) {
                repaired[i1] = gf16_add(repaired[i1], e1);
                repaired[i2] = gf16_add(repaired[i2], e2);
                return true;
            }
        }
    }
    return false;
}

int main(void) {
    uint8_t state[8] = {1, 4, 8, 15, 10, 1, 2, 14};
    uint8_t parity[4];
    z_spar_encode_8d(state, parity);

    uint8_t drifted[8] = {1, 4, 3, 15, 10, 1, 2, 14}; // OP drifted 8->3
    uint8_t repaired[8];
    bool ok = z_spar_verify_and_repair(drifted, parity, repaired);

    printf("[C99 Z-SPAR] Success: %s | Repaired OP: %d\n", ok ? "TRUE" : "FALSE", repaired[2]);
    return 0;
}
