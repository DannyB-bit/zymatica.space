# Codex A/B RF Evidence Runbook

This runbook is for collecting honest RAK Miner A to RAK Miner B LoRa RF evidence.

Goal:
- B starts first and proves the concentrator is recovered.
- A starts second and transmits the deterministic Milestone 1 payload.
- B proves reception with CRC OK and payload hash/bytes matching A.
- If reception fails, preserve the failure logs without claiming end-to-end RF success.

## Shared Rules

- Do not fabricate RX packets, packet-forwarder output, CRC results, hashes, photos, or logs.
- Use one radio owner at a time. Do not run packet-forwarder and raw HAL RX/TX together.
- Use the same RF settings on both nodes:
  - Frequency: `903.9 MHz`
  - Spreading factor: `SF9`
  - Bandwidth: `125 kHz`
  - Coding rate: `4/5`
  - CRC: enabled
  - Explicit header
  - Polarity inversion: disabled
- Run B before A.
- Save every command, start/end UTC timestamp, exit code, chip-id line, payload hash, and CRC result.

## Tell Codex B / RakMiner-B

Paste this as the task:

```text
You are on RakMiner-B, the receiver. Pull latest zk-lora-milestone-1 main. Use tools/lora_chirp_recovery.sh as the mandatory error-handling function before any RX attempt.

Objective: collect honest end-to-end RX evidence for Milestone 1.

Steps:
1. cd ~/zk-lora-milestone-1 && git pull --rebase origin main.
2. Create a new artifact folder:
   ARTIFACT="artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-b-rx_$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$ARTIFACT"
3. Run the LoRa Chirp concentrator recovery function and save its log in that folder:
   RECOVERY_LOG="$ARTIFACT/lora_chirp_recovery.log" ROLE=node-b-rx bash tools/lora_chirp_recovery.sh node-b-rx
4. If recovery does not produce LORA_CHIRP_RECOVERY_PASS=YES, stop and commit only the failure evidence.
5. Start raw HAL RX before A transmits. Use exactly 903.9 MHz / SF9 / 125 kHz. Do not use the old 903.0/SF7 path.
6. Capture from before A TX through at least 60 seconds after A finishes.
7. Extract and save:
   - rx_start_utc.txt
   - rx_end_utc.txt
   - raw RX log
   - command line used
   - valid packet count
   - CRC OK count
   - received payload hex or binary if available
   - received payload byte count
   - received payload SHA256
   - comparison against A payload SHA256
8. Success criteria are strict:
   CRC_OK_COUNT >= 1
   RX_PAYLOAD_BYTES matches A payload bytes
   RX_PAYLOAD_SHA256 equals A payload SHA256
9. If success criteria pass, write README.md saying end-to-end RF success was observed and include the exact evidence fields. If not, write README.md saying no valid end-to-end packet was decoded.
10. Commit and push only the new artifact folder.
```

## Tell Codex A / RakMiner-A

Paste this as the task after B says it is listening:

```text
You are on RakMiner-A, the transmitter. Pull latest zk-lora-milestone-1 main. Use tools/lora_chirp_recovery.sh as the mandatory error-handling function before TX.

Objective: transmit the deterministic Milestone 1 payload while RakMiner-B is already listening.

Steps:
1. cd ~/zk-lora-milestone-1 && git pull --rebase origin main.
2. Create a new artifact folder:
   ARTIFACT="artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-a-tx_$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$ARTIFACT"
3. Find or build the deterministic payload. Prefer the existing committed payload if present:
   artifacts/milestone1/hardware_capture/node-a-tx/interactive_privileged_retry/zk_lora_payload.bin
   or
   artifacts/milestone1/hardware_capture/node-a-tx/manual_tx_for_b_rx_20260630/zk_lora_payload.bin
4. Copy the selected payload to /tmp/zk_lora_m1_payload.bin and save:
   - payload_path.txt
   - payload_stat.txt
   - payload_sha256.txt
5. Run the LoRa Chirp concentrator recovery function and save its log in that folder:
   RECOVERY_LOG="$ARTIFACT/lora_chirp_recovery.log" ROLE=node-a-tx bash tools/lora_chirp_recovery.sh node-a-tx
6. If recovery does not produce LORA_CHIRP_RECOVERY_PASS=YES, stop and commit only the failure evidence.
7. Tell the user/B the exact UTC second before TX starts.
8. Transmit repeated payloads using exactly:
   903.9 MHz, SF9, 125 kHz, 14 dBm.
9. Save:
   - tx_start_utc.txt
   - tx_end_utc.txt
   - full TX log
   - TX exit code
   - every A_LORA_TX_FILE_SEND_COMPLETE line
10. Success on A only means TX succeeded. Do not claim end-to-end RF success unless B proves CRC OK and matching SHA256.
11. Commit and push only the new artifact folder.
```

## What Counts As Reviewer-Grade Success

The final evidence folder should contain:

- A recovery log with `LORA_CHIRP_RECOVERY_PASS=YES`.
- B recovery log with `LORA_CHIRP_RECOVERY_PASS=YES`.
- A TX log with chip detection and `A_LORA_TX_FILE_SEND_COMPLETE=YES`.
- B RX log with at least one valid packet and CRC OK.
- Matching payload byte count.
- Matching payload SHA256.
- README files on both nodes that state exact timestamps and commands.

Anything less is still useful engineering evidence, but it is not the final end-to-end RF proof.
