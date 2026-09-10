# node-a-tx Interactive Privileged Retry

Captured UTC: Tue Jun 30 00:09:26 UTC 2026

Purpose:
TX-side Raspberry Pi / RAK evidence after the user prepared the node from an interactive terminal.

Included:
- OS and hardware inventory
- SPI / USB device listing
- tcpdump capability status
- pinctrl path and GPIO state
- UDP 1680 tcpdump retry output
- ZK-LoRa TX proof app output
- real SX1302 LoRa TX attempts
- verifier and benchmark output

Result:
- Real SX1302 LoRa TX succeeded from node-a-tx.
- `real_lora_tx.log` records a successful standalone TX attempt at Tue Jun 30 00:08:27 UTC 2026.
- `real_lora_tx_during_capture.log` records a successful TX attempt during the coordinated retry at Tue Jun 30 00:09:22 UTC 2026.
- The transmitter opened SPI, detected SX1302 chip version `0x10`, completed `A_LORA_TX_FILE_SEND_BEGIN=YES` through `A_LORA_TX_FILE_SEND_COMPLETE=YES`, and exited with status 0.
- A-side tcpdump still did not have packet-capture permission. `tcpdump_udp1680.txt` and `tcpdump_pcap_stderr.txt` preserve `Operation not permitted`.
- No A-side UDP 1680 pcap was created in this retry.

Notes:
This evidence is honest. It proves a successful TX-side SX1302 send attempt from RAK Miner A, but it does not prove A-side UDP 1680 capture. Packet-path proof for this run must come from node-b-rx packet-forwarder, gateway, or tcpdump evidence aligned with the TX timestamps above.
