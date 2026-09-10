# Milestone 1 Reviewer Evidence

This file is the reviewer-facing index for the Milestone 1 evidence package.

## Executive status

Milestone 1 is complete as a reference prototype, hardware bring-up package, raw LoRa RF transfer proof, and encrypted proof-referenced secure packet RF proof:

- The multi-language proof suite runs and passes.
- The C++ verifier builds and runs.
- The WASM verifier builds and is committed as an artifact.
- The benchmark script records proof generation and verification timing.
- RAK Miner A proves real SX1302/SPI LoRa transmit bring-up.
- RAK Miner B proves real SX1302/SPI LoRa receive bring-up.
- RAK Miner A and B prove one end-to-end raw LoRa RF payload transfer with CRC OK packets and matching payload SHA-256.
- RAK Miner A and B prove one end-to-end encrypted proof-referenced secure packet transfer with decrypt, reference-proof verify, tamper rejection, wrong-key rejection, replay rejection, and matching packet SHA-256.
- The hardware layout and capture process are documented.

Older failed B-side RX attempts are preserved honestly. The later committed success artifacts under `artifacts/milestone1/hardware_capture/end_to_end_rf_success/` are the reviewer-grade RF proof.

## Artifact map

| Evidence area | Path | Reviewer read |
| :--- | :--- | :--- |
| Full proof suite | `artifacts/milestone1/verify_all_proofs_report.txt`, `VERIFICATION_REPORT.md` | 20/20 runtimes passed |
| C++ native verifier | `artifacts/milestone1/cpp_verifier_build_report.md`, `artifacts/milestone1/cpp_verifier_run.log` | Builds and runs with Clang |
| WASM verifier | `artifacts/milestone1/wasm_verifier_build_report.md`, `artifacts/milestone1/zk_lora_wasm_verifier.wasm` | WASM artifact committed with SHA-256 |
| Benchmark | `artifacts/milestone1/benchmark_report.md` | Local reference proof timing captured |
| Hardware layout | `docs/milestone1_hardware_layout.md` | 3-node RAK/Raspberry Pi topology documented |
| RF recovery/runbook | `tools/lora_chirp_recovery.sh`, `docs/codex_a_b_rf_evidence_runbook.md` | LoRa Chirp service stop, reset, chip check, and coordinated A/B capture procedure |
| Secure packet RF runbook | `tools/zk_lora_secure_packet.py`, `docs/secure_packet_rf_evidence_runbook.md` | Reference proof/encryption packet generation, decrypt/verify, tamper, replay, and RF capture procedure |
| RAK Miner A baseline | `artifacts/milestone1/hardware_capture/node-a-tx/README.md` | A host inventory and proof/benchmark run |
| RAK Miner A real TX | `artifacts/milestone1/hardware_capture/node-a-tx/interactive_privileged_retry/README.md` | SPI opened, SX1302 chip `0x10`, TX completed |
| RAK Miner A repeated TX | `artifacts/milestone1/hardware_capture/node-a-tx/manual_tx_for_b_rx_20260630/repeated_tx_burst.log` | Five LoRa TX bursts completed with exit 0 |
| RAK Miner B baseline | `artifacts/milestone1/hardware_capture/node-b-rx/README.md` | B host inventory and proof/benchmark run |
| RAK Miner B earlier RX attempts | `artifacts/milestone1/hardware_capture/node-b-rx/real_lora_rx_20260630/README.md` | RX hardware bring-up succeeded; no valid packet decoded in earlier attempts |
| End-to-end RF success, A side | `artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-a-tx_20260630T012251Z/README.md` | A recovered concentrator and completed five TX bursts |
| End-to-end RF success, B side | `artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-b-rx_20260630T012005Z/result_summary.txt` | B decoded CRC OK packets and matched A payload SHA-256 |
| Secure packet RF success, A side | `artifacts/milestone1/hardware_capture/secure_packet_rf/node-a-tx_20260630T135914Z/result_summary.txt` | A generated encrypted proof-referenced packet and completed five TX bursts |
| Secure packet RF success, B side | `artifacts/milestone1/hardware_capture/secure_packet_rf/node-b-rx_20260630T135643Z/result_summary.txt` | B matched packet SHA-256, decrypted, verified reference proof, and rejected tamper/wrong-key/replay |

## What the RAK evidence proves

RAK Miner A:

- Ran on Raspberry Pi / RAK hardware.
- Opened SPI against the SX1302 concentrator.
- Detected SX1302 chip version `0x10`.
- Used GPIO25 for SX1302 reset and GPIO22 for SX1261 reset in the successful TX path.
- Sent deterministic 240-byte payload bursts at 903.9 MHz, SF9, 125 kHz, 14 dBm.
- Completed all five repeated bursts with `A_LORA_TX_FILE_SEND_COMPLETE=YES` and exit status `0`.

RAK Miner B:

- Ran on Raspberry Pi / RAK hardware.
- Opened SPI against the SX1302 concentrator.
- Detected SX1302 chip version `0x10`.
- Ran a corrected HAL RX command during a timestamped A TX window.
- Preserved earlier failed RX attempts honestly.
- In the final end-to-end run, decoded five CRC OK packets during A's TX window.
- Verified a 240-byte received payload with SHA-256 `ef4b31ae0f7f159078191ea6169487bb66063a96c6927b83fe4070dcca0b4d3f`, matching A's transmitted payload.

## What is not claimed

This repository does not claim:

- Production LoRaWAN, Helium, TTN, ChirpStack, or Zcash mainnet integration.
- Production Groth16, halo2, gnark, arkworks, or Zcash consensus proof verification.

The RF evidence is a raw LoRa transport proof: SX1302 TX on RakMiner-A, SX1302 RX on RakMiner-B, CRC OK packets, and matching payload SHA-256.

The secure packet evidence is a reference-prototype security proof: a 217-byte packet transmitted over raw LoRa, authenticated and decrypted by RakMiner-B, with reference proof verification and negative tests. It is not a production ECIES/AES/gnark/halo2 proof.

## Success artifacts

The reviewer-grade RF success evidence is committed under:

`artifacts/milestone1/hardware_capture/end_to_end_rf_success/`

Key files:

- A-side TX evidence: `node-a-tx_20260630T012251Z/`
- B-side RX evidence: `node-b-rx_20260630T012005Z/`
- B machine-readable result: `node-b-rx_20260630T012005Z/result_summary.txt`
- B raw HAL RX log: `node-b-rx_20260630T012005Z/raw_hal_rx_9039_sf9_125k.log`

Reviewer-safe success test:

```text
CRC_OK_COUNT=5
RX_PAYLOAD_BYTES=240
A_PAYLOAD_SHA256=ef4b31ae0f7f159078191ea6169487bb66063a96c6927b83fe4070dcca0b4d3f
RX_PAYLOAD_SHA256=ef4b31ae0f7f159078191ea6169487bb66063a96c6927b83fe4070dcca0b4d3f
PAYLOAD_SHA256_MATCH=YES
END_TO_END_RF_SUCCESS=YES
```

## Secure packet artifacts

The strongest Milestone 1 security evidence is committed under:

`artifacts/milestone1/hardware_capture/secure_packet_rf/`

Key files:

- A-side secure packet TX evidence: `node-a-tx_20260630T135914Z/`
- B-side secure packet RX/verify evidence: `node-b-rx_20260630T135643Z/`
- B machine-readable result: `node-b-rx_20260630T135643Z/result_summary.txt`
- B verification JSON: `node-b-rx_20260630T135643Z/secure_packet_verify.json`
- B replay rejection JSON: `node-b-rx_20260630T135643Z/replay_check.json`

Reviewer-safe secure packet test:

```text
CRC_OK_COUNT=4
RX_PACKET_BYTES=217
RX_TX_PACKET_SHA256_MATCH=YES
PACKET_AUTH_OK=YES
DECRYPT_OK=YES
ZK_REFERENCE_PROOF_VERIFY_OK=YES
TAMPER_REJECTED=YES
WRONG_KEY_REJECTED=YES
REPLAY_REJECTED=YES
END_TO_END_SECURE_PACKET_OK=YES
```
