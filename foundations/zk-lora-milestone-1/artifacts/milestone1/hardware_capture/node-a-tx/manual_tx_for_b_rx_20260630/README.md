# node-a-tx Manual TX For B RX Correlation

Captured UTC: Tue Jun 30 00:33:39 UTC 2026

Purpose:
RAK Miner A transmitted a ZK-LoRa payload while RAK Miner B was running a real LoRa/SX1302 RX listener.

TX settings:
- Frequency: 903.9 MHz
- SF: 9
- Bandwidth: 125 kHz
- Power: 14 dBm

Included:
- TX start/end UTC timestamps
- Payload file size and SHA-256
- Full TX command output

Correlation:
RAK Miner B should inspect RX logs around the UTC start/end timestamps in this folder.

Evidence rule:
This artifact proves the A-side TX attempt only. B-side RX proof must come from B logs.
