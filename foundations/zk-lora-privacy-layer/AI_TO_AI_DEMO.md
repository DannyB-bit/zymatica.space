# ZK-LoRa AI-Assisted RF Evidence Summary

This document summarizes the AI-assisted test flow used while preparing the
public Milestone 1 evidence package. The grant-review source of truth is the
Milestone 1 repository and its committed artifacts:

- Milestone 1 reviewer evidence:
  https://github.com/DannyB-bit/zk-lora-milestone-1/blob/main/docs/MILESTONE_1_REVIEWER_EVIDENCE.md
- Secure packet RF result:
  https://github.com/DannyB-bit/zk-lora-milestone-1/blob/main/artifacts/milestone1/hardware_capture/secure_packet_rf/node-b-rx_20260630T135643Z/result_summary.txt
- Raw RF result:
  https://github.com/DannyB-bit/zk-lora-milestone-1/blob/main/artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-b-rx_20260630T012005Z/result_summary.txt

## Scope

Two AI-assisted Codex sessions coordinated work on physical RAK/Raspberry Pi
nodes:

- `RakMiner-A`: transmitter-side node
- `RakMiner-B`: receiver-side node

The useful claim is not that the AI agents are trusted. The useful claim is
that the nodes produced timestamped, committed artifacts showing real RF
packet movement and secure packet validation.

## Public Evidence Produced

Milestone 1 now includes:

- Native C++ verifier artifact.
- WebAssembly verifier artifact.
- `verify_all_proofs.py` proof-suite validation.
- RAK/Raspberry Pi hardware layout documentation.
- Real RAK Miner A-to-B LoRa RF packet transfer.
- CRC OK receiver evidence.
- Matching TX/RX SHA-256 payload validation.
- Secure encrypted proof-referenced packet transfer over LoRa RF.
- Decrypt pass, packet authentication pass, proof-reference verification pass.
- Tamper rejection, wrong-key rejection, and replay rejection.

## Important Boundary

Older private prototype transcripts referenced future Zcash payment-integration
work. Those logs are not the current public Milestone 1 claim. The current
public claim is the RF transport, reference proof, and secure-packet baseline.

Milestone 2 and Milestone 3 remain proposed funded work and will be opened as
deliverables progress.
