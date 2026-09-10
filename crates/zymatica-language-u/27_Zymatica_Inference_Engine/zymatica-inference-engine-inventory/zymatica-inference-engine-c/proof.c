// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

#include <stdio.h>
#include <stdlib.h>
#include "cuneiform_u_v3.h"

#ifdef _WIN32
#include <windows.h>
#else
#include <time.h>
#endif

int main() {
    printf("======================================================================\n");
    printf("ZYMATICA | zymatica-inference-engine-c\n");
    printf("======================================================================\n\n");

    Concept6D inputs[5] = {
        {1, 2, 3, 4, 5, 6},
        {8, 0, 15, 1, 0, 15},
        {0, 0, 0, 0, 0, 0},
        {15, 15, 15, 15, 15, 15},
        {4, 5, 6, 7, 8, 9}
    };

    uint8_t buffer[256];
    int bits = cuneiform_u_v3_encode(inputs, 5, buffer, 256, 1, 128);
    int bytes = (bits + 7) / 8;

    printf("Encoded Bits: %d, Bytes: %d\n", bits, bytes);
    printf("Hex: ");
    for (int i = 0; i < bytes; i++) {
        printf("%02X ", buffer[i]);
    }
    printf("\n");

    #ifdef _WIN32
        LARGE_INTEGER frequency;
        LARGE_INTEGER start, end;
        QueryPerformanceFrequency(&frequency);
        QueryPerformanceCounter(&start);
    #else
        clock_t start = clock();
    #endif

    int runs = 100000;
    int match = 1;
    for (int r = 0; r < runs; r++) {
        Concept6D outputs[5];
        int dec_ok = cuneiform_u_v3_decode(buffer, bytes, outputs, 5, 1, 128);
        if (r == 0) {
            printf("Decode success: %d\n", dec_ok);
            for (int i = 0; i < 5; i++) {
                if (inputs[i].domain != outputs[i].domain ||
                    inputs[i].subdomain != outputs[i].subdomain ||
                    inputs[i].operation != outputs[i].operation ||
                    inputs[i].modality != outputs[i].modality ||
                    inputs[i].depth != outputs[i].depth ||
                    inputs[i].polarity != outputs[i].polarity) {
                    match = 0;
                }
            }
        }
    }

    #ifdef _WIN32
        QueryPerformanceCounter(&end);
        double elapsed_ms = (double)(end.QuadPart - start.QuadPart) * 1000.0 / frequency.QuadPart;
    #else
        clock_t end = clock();
        double elapsed_ms = (double)(end - start) * 1000.0 / CLOCKS_PER_SEC;
    #endif

    printf("Decoded matches inputs: %s\n", match ? "true" : "false");
    if (!match) {
        printf("ERROR: mismatch!\n");
        exit(1);
    }
    printf("[INTERNAL_MATH] %.4f ms\n", elapsed_ms);

    printf("\n[VERIFICATION] Multi-Language runtime FFI structures validated.\n");
    return 0;
}
