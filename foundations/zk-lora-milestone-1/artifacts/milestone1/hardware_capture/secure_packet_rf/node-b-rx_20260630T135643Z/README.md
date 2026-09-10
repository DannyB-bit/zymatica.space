# node-b-rx Secure Packet RF Evidence

Captured UTC: Tue Jun 30 14:08:30 UTC 2026

Purpose:
Receiver-side evidence for a proof-referenced encrypted ZK-LoRa secure packet over real RakMiner-A to RakMiner-B LoRa RF.

Recovery:
- Recovery command: `ROLE=node-b-rx bash tools/lora_chirp_recovery.sh node-b-rx`
- Recovery result: `LORA_CHIRP_RECOVERY_PASS=YES`
- Chip ID evidence: `Note: chip version is 0x10 (v1.0)`
- Concentrator EUI: `0x0016c001ff18afa3`

RX settings:
- Target frequency: 903.9 MHz
- HAL radio A center: 904.3 MHz
- Channel IF: -400000 Hz
- Spreading factor: SF9
- Bandwidth: 125 kHz
- CRC: enabled

RX command:
`timeout 540 sudo ./test_loragw_hal_rx -d /dev/spidev0.0 -r 1250 -a 904.3 -b 905.0 -k 0 -m 0 -j -z 255 -n 1`

Timing:
- B recovery pass: 2026-06-30T13:56:58Z
- B RX start file: 2026-06-30T13:59:27Z
- B RX end: 2026-06-30T14:08:30Z
- A TX start: Tue Jun 30 13:59:26 UTC 2026
- A TX end: Tue Jun 30 14:00:37 UTC 2026
- A commit: 9c75ecb
- A artifact: `artifacts/milestone1/hardware_capture/secure_packet_rf/node-a-tx_20260630T135914Z`

Timing note:
B recovery completed before A was instructed and B captured repeated CRC OK packets during A's TX window. The recorded `rx_start_utc.txt` is one second after A's recorded `tx_start_utc.txt`; this artifact preserves that timestamp honestly.

RF packet result:
- Valid LoRa packet entries: 5
- CRC OK count: 4
- First CRC OK secure packet bytes: 217
- TX packet SHA256: `874ff62e365dd9d90e460c8c51c475e319968e62bc9c03836e9ae98c9d0f4137`
- RX packet SHA256: `874ff62e365dd9d90e460c8c51c475e319968e62bc9c03836e9ae98c9d0f4137`
- TX/RX packet SHA256 match: YES

Secure packet verification:
- `PACKET_AUTH_OK=YES`
- `DECRYPT_OK=YES`
- `ZK_REFERENCE_PROOF_VERIFY_OK=YES`
- `TAMPER_REJECTED=YES`
- `WRONG_KEY_REJECTED=YES`
- `REPLAY_REJECTED=YES`
- `END_TO_END_SECURE_PACKET_OK=YES`

Verification files:
- `secure_packet_verify.json` is the first verification with tamper tests and nonce insertion.
- `replay_check.json` is the second verification with the same nonce DB; it reports `replay_ok: false`.

Notes:
- `rx_exit_code.txt` is `124` because the receiver was intentionally bounded by `timeout 540`.
- The final non-CRC-OK packet in the raw log was not used for secure packet verification.
