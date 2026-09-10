// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica.
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

mod cuneiform_u_v3;

use cuneiform_u_v3::{Concept6D, cuneiform_u_v3_encode, cuneiform_u_v3_decode};

fn main() {
    println!("=========================================================");
    println!("  CUNEIFORM-U V3.0 RUST RANGE CODER BENCHMARK");
    println!("  zymatica.space | astronautshe.com");
    println!("=========================================================");

    // Benchmark 1: Reboot Sequence (40 concepts)
    let block = vec![
        Concept6D { domain: 8, subdomain: 0, operation: 0, modality: 4, depth: 7, polarity: 8 },  // SYNC
        Concept6D { domain: 0, subdomain: 0, operation: 15, modality: 14, depth: 5, polarity: 1 }, // ENERGY
        Concept6D { domain: 0, subdomain: 0, operation: 7, modality: 11, depth: 13, polarity: 15 }, // CYCLE
        Concept6D { domain: 0, subdomain: 0, operation: 7, modality: 14, depth: 0, polarity: 11 },  // ACK
    ];

    let mut reboot_sequence = Vec::new();
    for _ in 0..10 {
        reboot_sequence.extend_from_slice(&block);
    }

    let compressed = cuneiform_u_v3_encode(&reboot_sequence, 1, 128);
    let total_bits = compressed.len() * 8;

    println!("\n--- BENCHMARK 1: REBOOT SEQUENCE (40 Concepts) ---");
    println!("  Cuneiform-U Rust Coder size:     {} bytes ({} bits)", compressed.len(), total_bits);
    println!("  Semantic bits/concept:           {:.2} bits", (total_bits as f32) / 40.0);

    let decoded = cuneiform_u_v3_decode(compressed, 40, 1, 128);
    let match_success = reboot_sequence == decoded;
    println!("  Fidelity verification:           {}", if match_success { "PASS [OK]" } else { "FAIL [❌]" });

    // Benchmark 2: Zero-Shot Dynamic Concept Composition (20 concepts)
    let dynamic_block = vec![
        Concept6D { domain: 1, subdomain: 3, operation: 3, modality: 13, depth: 4, polarity: 6 },
        Concept6D { domain: 1, subdomain: 1, operation: 0, modality: 6, depth: 5, polarity: 5 },
    ];
    let mut dynamic_sequence = Vec::new();
    for _ in 0..10 {
        dynamic_sequence.extend_from_slice(&dynamic_block);
    }

    let compressed_dyn = cuneiform_u_v3_encode(&dynamic_sequence, 1, 128);
    let total_bits_dyn = compressed_dyn.len() * 8;

    println!("\n--- BENCHMARK 2: DYNAMIC SEMANTIC EXPRESSION (20 Concepts) ---");
    println!("  Cuneiform-U Rust Coder size:     {} bytes ({} bits)", compressed_dyn.len(), total_bits_dyn);
    println!("  Semantic bits/concept:           {:.2} bits", (total_bits_dyn as f32) / 20.0);

    let decoded_dyn = cuneiform_u_v3_decode(compressed_dyn, 20, 1, 128);
    let match_success_dyn = dynamic_sequence == decoded_dyn;
    println!("  Fidelity verification:           {}", if match_success_dyn { "PASS [OK]" } else { "FAIL [❌]" });
}
