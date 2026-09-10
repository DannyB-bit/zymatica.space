# node-b-rx End-to-End RF RX Evidence

Captured UTC: Tue Jun 30 01:27:40 UTC 2026

Purpose:
Receiver-side evidence for RakMiner-A to RakMiner-B Milestone 1 LoRa RF.

Recovery:
- Recovery command: `ROLE=node-b-rx bash tools/lora_chirp_recovery.sh node-b-rx`
- Recovery result: `LORA_CHIRP_RECOVERY_PASS=YES`
- Derived concentrator result: `CONCENTRATOR_RECOVERY_PASS=YES`
- Chip ID evidence: `Note: chip version is 0x10 (v1.0)`
- Concentrator EUI: `0x0016c001ff18afa3`

RX settings:
- Target frequency: 903.9 MHz
- HAL radio A center: 904.3 MHz
- Channel IF: -400000 Hz
- Spreading factor: SF9
- Bandwidth: 125 kHz
- Coding rate: 4/5
- CRC: enabled

RX command:
`timeout 420 sudo ./test_loragw_hal_rx -d /dev/spidev0.0 -r 1250 -a 904.3 -b 905.0 -k 0 -m 0 -j -z 255 -n 1`

Timing:
- B RX start: 2026-06-30T01:20:37Z
- B RX end: 2026-06-30T01:27:40Z
- A TX start: Tue Jun 30 01:23:02 UTC 2026
- A TX end: Tue Jun 30 01:24:14 UTC 2026
- A commit: 6a47fd8
- A artifact: `artifacts/milestone1/hardware_capture/end_to_end_rf_success/node-a-tx_20260630T012251Z`

RX result:
- Valid LoRa packet entries: 6
- CRC OK packet count: 5
- First CRC OK payload byte count: 240
- A payload SHA256: `ef4b31ae0f7f159078191ea6169487bb66063a96c6927b83fe4070dcca0b4d3f`
- B RX payload SHA256: `ef4b31ae0f7f159078191ea6169487bb66063a96c6927b83fe4070dcca0b4d3f`
- Payload SHA256 match: YES

Conclusion:
End-to-end RF success was observed. RakMiner-B decoded CRC OK LoRa packets at 903900000 Hz / SF9 during RakMiner-A's TX window, and the received 240-byte payload SHA256 matched RakMiner-A's transmitted payload SHA256.

Notes:
- `rx_exit_code.txt` is `124` because the receiver was intentionally bounded by `timeout 420`.
- The first non-CRC-OK packet in the log was not used for the payload comparison.
