# node-a-tx Repeated TX Burst For B RX Retry

Captured UTC: Tue Jun 30 00:42:29 UTC 2026

Purpose:
RAK Miner A transmitted repeated SX1302 LoRa bursts while RAK Miner B ran a real LoRa RX retry.

TX settings:
- Frequency: 903.9 MHz
- SF: 9
- Bandwidth: 125 kHz
- Power: 14 dBm

Included:
- repeated_tx_burst.log with five TX attempts
- payload file size and SHA-256
- UTC start/end timestamps for each burst

Correlation:
RAK Miner B should inspect RX logs across the timestamps in repeated_tx_burst.log.

Evidence rule:
This proves A-side repeated TX only. B-side RX proof must come from B logs.
