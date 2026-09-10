#include <stdio.h>
#include "cuneiform_u_v3.h"

int run_test() {
    printf("======================================================================\n");
    printf("  CUNEIFORM-U V3.0 / LANGUAGE U V4.0 C-BASED RANGE CODER BENCHMARK\n");
    printf("  zymatica.space | astronautshe.com\n");
    printf("======================================================================\n\n");

    /* 1. Define Reboot Sequence (40 Concepts) */
    /* Repeating: SYNC, ENERGY, CYCLE, ACK */
    /* We map these to their 6D coordinates:
       SYNC (0x8104): domain=8 (ENGINEER), subdomain=0, operation=0 (IDENTITY), modality=4 (LIKELY), depth=7 (TRANSCENDENT), polarity=7 (SUPERPOSED) - wait,
       let's use the exact coordinates or standard indices.
       From Python:
         SYNC (0x8104): rc=0x80 (DOMAIN=8, SUBDOMAIN=0), rf=0x04 (OPERATION=0, MODALITY=4), ra=0x78 (DEPTH=7, POLARITY=8)
         ENERGY (0x80FE): rc=0x00 (DOMAIN=0, SUBDOMAIN=0), rf=0xFE (OPERATION=15, MODALITY=14), ra=0x51 (DEPTH=5, POLARITY=1)
         CYCLE (0x807B): rc=0x00, rf=0x7B, ra=0xDF
         ACK (0x807E): rc=0x00, rf=0x7E, ra=0x0B
    */
    Concept6D reboot_sequence[40];
    Concept6D block[4] = {
        {8, 0, 0, 4, 7, 8},   /* SYNC */
        {0, 0, 15, 14, 5, 1}, /* ENERGY */
        {0, 0, 7, 11, 13, 15},/* CYCLE */
        {0, 0, 7, 14, 0, 11}  /* ACK */
    };
    for (int i = 0; i < 10; i++) {
        memcpy(&reboot_sequence[i * 4], block, sizeof(block));
    }

    uint8_t compressed_buf[256];
    uint32_t max_bytes = sizeof(compressed_buf);

    /* Encode */
    int bit_count = cuneiform_u_v3_encode(reboot_sequence, 40, compressed_buf, max_bytes, 1, 128);
    if (bit_count < 0) {
        printf("Encoding failed!\n");
        return 1;
    }
    int byte_count = (bit_count + 7) / 8;

    printf("--- BENCHMARK 1: REBOOT SEQUENCE (40 Concepts) ---\n");
    printf("  Cuneiform-U v3.0 C-Coder size:   %d bytes (%d bits)\n", byte_count, bit_count);
    printf("  Semantic bits/concept:           %.2f bits\n", (float)bit_count / 40.0f);

    /* Decode & Verify */
    Concept6D decoded_sequence[40];
    int decode_success = cuneiform_u_v3_decode(compressed_buf, byte_count, decoded_sequence, 40, 1, 128);
    
    int match = 1;
    for (int i = 0; i < 40; i++) {
        if (reboot_sequence[i].domain != decoded_sequence[i].domain ||
            reboot_sequence[i].subdomain != decoded_sequence[i].subdomain ||
            reboot_sequence[i].operation != decoded_sequence[i].operation ||
            reboot_sequence[i].modality != decoded_sequence[i].modality ||
            reboot_sequence[i].depth != decoded_sequence[i].depth ||
            reboot_sequence[i].polarity != decoded_sequence[i].polarity) {
            match = 0;
            printf("  Mismatch at index %d!\n", i);
            break;
        }
    }
    printf("  Fidelity verification:           %s\n\n", match && decode_success ? "PASS ✅" : "FAIL ❌");

    /* 2. Zero-Shot Dynamic Concept Composition */
    Concept6D dynamic_block[2] = {
        {1, 3, 3, 13, 4, 6}, /* Relativistic warp anomaly warning */
        {1, 1, 0, 6, 5, 5}   /* Quantum equilibrium balance */
    };
    Concept6D dynamic_sequence[20];
    for (int i = 0; i < 10; i++) {
        memcpy(&dynamic_sequence[i * 2], dynamic_block, sizeof(dynamic_block));
    }

    bit_count = cuneiform_u_v3_encode(dynamic_sequence, 20, compressed_buf, max_bytes, 1, 128);
    byte_count = (bit_count + 7) / 8;

    printf("--- BENCHMARK 2: ZERO-SHOT DYNAMIC SEMANTIC EXPRESSION (20 Concepts) ---\n");
    printf("  Cuneiform-U v3.0 C-Coder size:   %d bytes (%d bits)\n", byte_count, bit_count);
    printf("  Semantic bits/concept:           %.2f bits\n", (float)bit_count / 20.0f);

    Concept6D decoded_dynamic[20];
    decode_success = cuneiform_u_v3_decode(compressed_buf, byte_count, decoded_dynamic, 20, 1, 128);

    match = 1;
    for (int i = 0; i < 20; i++) {
        if (dynamic_sequence[i].domain != decoded_dynamic[i].domain ||
            dynamic_sequence[i].subdomain != decoded_dynamic[i].subdomain ||
            dynamic_sequence[i].operation != decoded_dynamic[i].operation ||
            dynamic_sequence[i].modality != decoded_dynamic[i].modality ||
            dynamic_sequence[i].depth != decoded_dynamic[i].depth ||
            dynamic_sequence[i].polarity != decoded_dynamic[i].polarity) {
            match = 0;
            printf("  Mismatch at index %d!\n", i);
            break;
        }
    }
    printf("  Fidelity verification:           %s\n", match && decode_success ? "PASS ✅" : "FAIL ❌");

    return 0;
}

int main() {
    return run_test();
}
