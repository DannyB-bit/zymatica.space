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
- Payment matching logic verifies a decrypted payment event fixture containing `ref:demo_packet_hash_hello_zcash_mesh`.
- The developer treasury split is checked as 2% using integer zatoshi math.
- RAK operator TX/RX logs are summarized in `artifacts/milestone1/rak_operator_log_summary.md`.
- The dedicated Milestone 1 repo contains end-to-end raw LoRa RF evidence: CRC OK packets and matching 240-byte payload SHA-256 between RakMiner-A and RakMiner-B.
- The dedicated Milestone 1 repo also contains secure-packet RF evidence: encrypted proof-referenced packet transfer, authenticated decrypt, reference-proof verification, tamper rejection, wrong-key rejection, replay rejection, and matching packet SHA-256.

Do not overclaim:
- The current scanner does not decrypt shielded memos from the public chain by itself.
- A real Zcash wallet/light-client adapter must supply decrypted payment events.
- Public explorers cannot decrypt shielded Zcash memos.
- The RF success evidence is a raw LoRa payload transfer, not a production LoRaWAN, Helium, TTN, ChirpStack, or Zcash mainnet integration.
- The proof code is a reference/structural prototype, not a production Groth16/halo2/arkworks verifier.

Forum checklist:
- Link to the four repos and this readiness note.
- Link `artifacts/milestone1/README.md` and `docs/milestone1_hardware_layout.md`.
- Link the Milestone 1 RF result: `zk-lora-milestone-1/artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-b-rx_20260630T012005Z/result_summary.txt`.
- Link the Milestone 1 secure-packet result: `zk-lora-milestone-1/artifacts/milestone1/hardware_capture/secure_packet_rf/node-b-rx_20260630T135643Z/result_summary.txt`.
- Say the current milestone proves deterministic packet-reference and 2% fee validation from decrypted events.
- Say the next grant-funded integration is real Zcash testnet/regtest wallet scanning plus production proof integration.
- Avoid "fully private", "production", "on-chain verified split", or "live mempool decrypted" language until those pieces are demonstrated.
