# Milestone 1 Artifact Pack

Generated on 2026-06-29 against the updated 18-page whitepaper PDF in this repository.

## Promise-to-evidence map

| Milestone 1 promise | Evidence in this repo | Status |
| :--- | :--- | :--- |
| Real `verify_all_proofs.py` run artifact | `verify_all_proofs_report.txt`; generated `VERIFICATION_REPORT.md` | Complete: 20/20 runtimes passed |
| C++ native verifier build and run | `cpp_verifier_build_report.md`, `cpp_verifier_build.log`, `cpp_verifier_run.log` | Complete on Windows amd64 with Clang |
| WASM verifier build or artifact | `wasm_verifier_build_report.md`, `wasm_verifier_build.log`, `zk_lora_wasm_verifier.wasm` | Complete; WASM artifact committed |
| Proof generation and verification timing | `benchmark_report.md`, `benchmark_report.txt`, `run_proof_benchmark_raw.txt` | Complete for local Windows reference benchmark |
| 3-node gateway hardware layout | `docs/milestone1_hardware_layout.md` | Complete as implementation layout and capture checklist |
| Raspberry Pi/RAK hardware capture | `hardware_capture/node-a-tx/`, `hardware_capture/node-b-rx/`, `hardware_capture/end_to_end_rf_success/`, `hardware_capture/secure_packet_rf/`, `docs/MILESTONE_1_REVIEWER_EVIDENCE.md` | Complete for TX/RX hardware bring-up, one CRC/hash-matched raw LoRa RF transfer, and one encrypted proof-referenced secure packet RF transfer |

## Scope

This artifact pack proves that the Milestone 1 reference implementation builds and runs across the local toolchain and that the native C++ and WASM verifier artifacts are reproducible. It also contains RAK Miner A/B hardware evidence. RAK Miner A demonstrates successful SX1302 LoRa TX; RAK Miner B demonstrates SX1302 HAL RX and decoded CRC OK packets during the A-side TX window. The raw RF run matched a 240-byte payload SHA-256, and the secure packet run matched a 217-byte packet SHA-256 while also proving authenticated decrypt, reference-proof verification, tamper rejection, wrong-key rejection, and replay rejection.

The proof code is a structural Groth16-style reference prototype. Production ZK proof generation using gnark, arkworks, halo2, or Zcash circuit code remains future work as described in the updated whitepaper roadmap.
