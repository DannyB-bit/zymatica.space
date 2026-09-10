# node-a-tx End-to-End RF TX Evidence

Captured UTC: Tue Jun 30 01:24:14 UTC 2026

Purpose:
A-side deterministic Milestone 1 payload transmission while RakMiner-B was already listening.

TX settings:
- Frequency: 903.9 MHz
- SF: 9
- Bandwidth: 125 kHz
- Power: 14 dBm

Included:
- LoRa Chirp recovery log with LORA_CHIRP_RECOVERY_PASS=YES
- deterministic payload path, byte count, and SHA256
- tx_start_utc.txt and tx_end_utc.txt
- full repeated TX log
- TX exit code
- all A_LORA_TX_FILE_SEND_COMPLETE lines

Evidence rule:
This artifact proves A-side TX only. End-to-end RF success requires RakMiner-B CRC OK and RX SHA256 matching A payload SHA256.
