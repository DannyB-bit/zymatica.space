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
| Raspberry Pi/RAK hardware evidence | `raspberry_pi_rak_setup_report.md`, `rak_operator_log_summary.md`, [Milestone 1 RF artifacts](https://github.com/DannyB-bit/zk-lora-milestone-1/tree/main/artifacts/milestone1/hardware_capture/end_to_end_rf_success), [secure packet RF artifacts](https://github.com/DannyB-bit/zk-lora-milestone-1/tree/main/artifacts/milestone1/hardware_capture/secure_packet_rf) | Main repo logs present; dedicated Milestone 1 repo contains CRC/hash-matched raw LoRa RF proof and encrypted proof-referenced secure packet RF proof |
| Hardware bench photos | `rak_hardware_bench_photo_20260629.jpg`, `lab_staging_setup_photo_20260629.jpg` | Supplemental owner-supplied setup photos; RF proof remains the committed CRC/SHA logs |

## Scope

This artifact pack proves that the Milestone 1 reference implementation builds and runs across the local toolchain and that the native C++ and WASM verifier artifacts are reproducible. The dedicated Milestone 1 repo additionally contains a raw LoRa RF proof where RakMiner-B decoded CRC OK packets and matched the transmitted 240-byte payload SHA-256 from RakMiner-A, plus a secure-packet RF proof where RakMiner-B matched packet SHA-256, decrypted the authenticated payload, verified reference proof fields, and rejected tamper, wrong-key, and replay attempts. The proof code is a structural Groth16-style reference prototype. Production ZK proof generation using gnark, arkworks, halo2, or Zcash circuit code remains future work as described in the updated whitepaper roadmap.
