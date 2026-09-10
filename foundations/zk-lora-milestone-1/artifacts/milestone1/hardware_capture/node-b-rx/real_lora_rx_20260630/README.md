# node-b-rx Real LoRa RX Attempt

Captured UTC: Tue Jun 30 00:36:35 UTC 2026

Purpose:
Receiver-side attempt to capture RAK Miner A direct SX1302 LoRa TX.

Expected A TX settings:
- Frequency: 903.9 MHz
- SF: 9
- Bandwidth: 125 kHz
- Power: 14 dBm

A-side TX correlation:
- TX start UTC: Tue Jun 30 00:33:35 UTC 2026
- TX end UTC: Tue Jun 30 00:33:39 UTC 2026
- A_LORA_TX_FILE_SEND_COMPLETE=YES
- TX_EXIT_STATUS=0
- A-side commit: eaa56df

Included:
- RX tool discovery
- initial real LoRa RX wrapper attempt log
- corrected sudo SX1302 HAL RX attempt log
- gateway/service logs around fresh TX
- grep summary for RX/RSSI/SNR/LoRa evidence

Result summary:
- Corrected B RX opened SPI and detected SX1302 chip version 0x10.
- Corrected B RX was running from 2026-06-30T00:32:58Z to 2026-06-30T00:36:04Z, covering A TX at 00:33:35-00:33:39Z.
- HAL summary reported: Nb valid packets received: 0 CRC OK (1).
- Command exited 124 due to timeout.

Notes:
This evidence is honest. No valid RX packet was found in the corrected HAL RX attempt; the logs preserve that result.

## Repeated TX Retry

Captured UTC: Tue Jun 30 00:44:11 UTC 2026

Additional retry logs:
- b_rx_one_frame_test_retry.log
- b_stock_hal_rx_9039_sf9_retry.log
- final_rx_retry_grep.txt
- repeated_tx_retry_summary.txt

Retry conclusion:
No B-side valid received packet was found. The one-frame retry used 903.0 MHz / SF7, which did not match A's requested 903.9 MHz / SF9 settings. The stock 903.9 MHz / SF9 wrapper failed before RX because reset_lgw.sh was called without its required {start|stop} argument. The prior corrected HAL RX remains the best matching B attempt and reported zero valid packets.
