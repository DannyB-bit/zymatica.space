# Secure Packet RF Evidence Runbook

Purpose:
Collect a small reviewer-facing Milestone 1 evidence artifact proving that the RAK devices can run the reference proof/encryption flow and transmit the resulting secure packet over real LoRa RF.

Scope:
- Reference Groth16-style proof from `run_proof.py`, not production gnark/arkworks/halo2.
- Reference authenticated encryption envelope, not production ECIES/AES.
- Raw LoRa RF transport, not LoRaWAN/Helium/TTN/ChirpStack integration.

Required success fields:

```text
LORA_CHIRP_RECOVERY_PASS=YES
PACKET_AUTH_OK=YES
DECRYPT_OK=YES
ZK_REFERENCE_PROOF_VERIFY_OK=YES
TAMPER_REJECTED=YES
WRONG_KEY_REJECTED=YES
REPLAY_REJECTED=YES
RX_PACKET_SHA256 == TX_PACKET_SHA256
END_TO_END_SECURE_PACKET_OK=YES
```

## Shared Settings

Use the same values on A and B:

```bash
export ZK_LORA_SHARED_SECRET="zk-lora-m1-secure-rf-demo-20260630"
export ZK_LORA_SETUP_SEED="zk-lora-m1-reference-setup-20260630"
```

RF settings:

```text
Frequency: 903.9 MHz
Spreading factor: SF9
Bandwidth: 125 kHz
TX power: 14 dBm
```

## Codex B / RakMiner-B Prompt

Paste this into Codex B first:

```text
You are on RakMiner-B, the receiver. Pull latest zk-lora-milestone-1 main.

Goal: collect receiver-side evidence for an encrypted proof-referenced ZK-LoRa secure packet over real LoRa RF.

Rules:
- Do not fabricate logs, packets, CRC, hashes, decryptions, proof checks, tamper checks, or replay checks.
- Use tools/lora_chirp_recovery.sh before RX.
- Use tools/zk_lora_secure_packet.py to verify/decrypt the received packet.
- B must be listening before A transmits.

Steps:
1. cd ~/zk-lora-milestone-1 && git pull --rebase origin main.
2. export ZK_LORA_SHARED_SECRET="zk-lora-m1-secure-rf-demo-20260630"
3. export ZK_LORA_SETUP_SEED="zk-lora-m1-reference-setup-20260630"
4. ARTIFACT="artifacts/milestone1/hardware_capture/secure_packet_rf/node-b-rx_$(date -u +%Y%m%dT%H%M%SZ)"
5. mkdir -p "$ARTIFACT"
6. RECOVERY_LOG="$ARTIFACT/lora_chirp_recovery.log" ROLE=node-b-rx bash tools/lora_chirp_recovery.sh node-b-rx
7. If LORA_CHIRP_RECOVERY_PASS=YES is absent, stop and commit the failure artifact only.
8. Start raw HAL RX at 903.9 MHz / SF9 / 125 kHz before A transmits.
9. Capture through A's TX window and extract the first CRC OK payload as "$ARTIFACT/rx_secure_packet.bin".
10. Save raw HAL RX log, RX command, rx_start_utc.txt, rx_end_utc.txt, CRC OK count, packet byte count, and packet SHA256.
11. Run:
    python tools/zk_lora_secure_packet.py verify \
      --packet "$ARTIFACT/rx_secure_packet.bin" \
      --manifest "$ARTIFACT/secure_packet_verify.json" \
      --nonce-db "$ARTIFACT/nonce_seen.txt" \
      --tamper-test
12. Run the same verify command a second time with the same nonce DB and save output as replay_check.json; this second run should show replay_ok=false or end_to_end_secure_packet_ok=false.
13. Write result_summary.txt with exact fields:
    LORA_CHIRP_RECOVERY_PASS
    CRC_OK_COUNT
    RX_PACKET_BYTES
    RX_PACKET_SHA256
    PACKET_AUTH_OK
    DECRYPT_OK
    ZK_REFERENCE_PROOF_VERIFY_OK
    TAMPER_REJECTED
    WRONG_KEY_REJECTED
    REPLAY_REJECTED
    END_TO_END_SECURE_PACKET_OK
14. Write README.md explaining the result honestly.
15. Commit and push only this new node-b-rx artifact folder.
```

## Codex A / RakMiner-A Prompt

Paste this into Codex A only after B confirms RX is running:

```text
You are on RakMiner-A, the transmitter. Pull latest zk-lora-milestone-1 main.

Goal: generate an encrypted proof-referenced ZK-LoRa secure packet on-device, transmit it over real LoRa RF, and commit TX-side evidence.

Rules:
- Do not fabricate TX, proof, encryption, packet hash, recovery, or timing evidence.
- Use tools/lora_chirp_recovery.sh before TX.
- Use tools/zk_lora_secure_packet.py to create the packet.
- A success only proves generation + TX; B must prove decrypt/proof/tamper/replay checks.

Steps:
1. cd ~/zk-lora-milestone-1 && git pull --rebase origin main.
2. export ZK_LORA_SHARED_SECRET="zk-lora-m1-secure-rf-demo-20260630"
3. export ZK_LORA_SETUP_SEED="zk-lora-m1-reference-setup-20260630"
4. ARTIFACT="artifacts/milestone1/hardware_capture/secure_packet_rf/node-a-tx_$(date -u +%Y%m%dT%H%M%SZ)"
5. mkdir -p "$ARTIFACT"
6. python tools/zk_lora_secure_packet.py make \
     --agent-id node-a-tx \
     --message "ZK-LoRa M1 secure packet: proof reference plus encrypted payload over RAK LoRa RF." \
     --out "$ARTIFACT/tx_secure_packet.bin" \
     --manifest "$ARTIFACT/secure_packet_make.json"
7. Save packet byte count and SHA256:
     wc -c "$ARTIFACT/tx_secure_packet.bin" > "$ARTIFACT/tx_packet_bytes.txt"
     sha256sum "$ARTIFACT/tx_secure_packet.bin" > "$ARTIFACT/tx_packet_sha256.txt"
8. RECOVERY_LOG="$ARTIFACT/lora_chirp_recovery.log" ROLE=node-a-tx bash tools/lora_chirp_recovery.sh node-a-tx
9. If LORA_CHIRP_RECOVERY_PASS=YES is absent, stop and commit the failure artifact only.
10. Tell the user/B the exact UTC timestamp before TX starts.
11. Transmit "$ARTIFACT/tx_secure_packet.bin" repeatedly over LoRa at 903.9 MHz / SF9 / 125 kHz / 14 dBm.
12. Save tx_start_utc.txt, tx_end_utc.txt, full TX log, TX exit codes, and A_LORA_TX_FILE_SEND_COMPLETE lines.
13. Write result_summary.txt with exact fields:
     LORA_CHIRP_RECOVERY_PASS=YES
     TX_PACKET_BYTES
     TX_PACKET_SHA256
     ZK_REFERENCE_PROOF_GENERATED
     ENCRYPTED_PAYLOAD_GENERATED
     SEND_COMPLETE_COUNT
14. Write README.md explaining that A proves on-device packet generation/encryption/proof-reference plus TX, while B must prove RX/decrypt/proof/tamper/replay.
15. Commit and push only this new node-a-tx artifact folder.
```
