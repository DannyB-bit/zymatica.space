# Forum Readiness Notes

Status: prototype-ready for community review, not production-ready.

Verified locally:
- Python reference proof runner passes.
- Full `python verify_all_proofs.py` suite passes 20/20 runtimes.
- C++ verifier source builds and runs with Clang on Windows amd64.
- Rust WASM verifier builds for `wasm32-unknown-unknown`; artifact captured in `artifacts/milestone1/`.
- `benchmark_milestone1.py` records separate reference proof generation and verification timings.
- Rust operator builds.
- TypeScript operator builds and runs `npm test`.
- Go operator builds and runs `go test` plus `go run . --test`.
- RAK Miner A hardware evidence shows SX1302 SPI access, chip `0x10`, and repeated LoRa TX bursts completed.
- RAK Miner B hardware evidence shows SX1302 SPI access, chip `0x10`, and HAL RX bring-up.
- End-to-end RF evidence shows RakMiner-B decoded CRC OK packets during RakMiner-A's TX window, with matching 240-byte payload SHA-256.
- Secure packet RF evidence shows a 217-byte proof-referenced encrypted packet transmitted over LoRa, with TX/RX SHA-256 match, decrypt pass, reference-proof verify pass, tamper rejection, wrong-key rejection, and replay rejection.

Do not overclaim:
- The proof code is a reference/structural prototype.
- This milestone does not yet provide production Groth16, halo2, arkworks, or gnark proof artifacts.
- The committed RF success is a raw LoRa payload transfer, not a production LoRaWAN, Helium, TTN, ChirpStack, or Zcash mainnet integration.
- Zcash payment scanning belongs to later milestones.

Forum checklist:
- Present this repo as the multi-language identity/proof prototype.
- Link `artifacts/milestone1/README.md`, `docs/milestone1_hardware_layout.md`, and `docs/MILESTONE_1_REVIEWER_EVIDENCE.md`.
- Link `artifacts/milestone1/hardware_capture/secure_packet_rf/node-b-rx_20260630T135643Z/result_summary.txt` as the strongest Milestone 1 RF/security artifact.
- Include negative-test and production-proof work as next steps.
- Avoid implying this is audited cryptography.
