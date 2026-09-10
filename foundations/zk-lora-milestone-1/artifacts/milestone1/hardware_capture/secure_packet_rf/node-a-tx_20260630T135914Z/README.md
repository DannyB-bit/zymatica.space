# node-a-tx Secure Packet RF TX Evidence

Captured UTC: Tue Jun 30 14:00:37 UTC 2026

Purpose:
A proves on-device secure packet generation, reference proof creation, encrypted payload creation, concentrator recovery, and LoRa TX.

TX settings:
- Frequency: 903.9 MHz
- SF: 9
- Bandwidth: 125 kHz
- Power: 14 dBm

Included:
- secure packet binary and make manifest
- packet byte count and SHA256
- LoRa Chirp recovery log with LORA_CHIRP_RECOVERY_PASS=YES
- tx_start_utc.txt and tx_end_utc.txt
- full TX log, TX exit codes, send-complete lines, and result summary

Evidence rule:
A proves on-device secure packet generation, reference proof creation, encrypted payload creation, concentrator recovery, and LoRa TX. B must prove RX, decrypt, proof verify, tamper rejection, wrong-key rejection, replay rejection, and matching packet SHA256.
