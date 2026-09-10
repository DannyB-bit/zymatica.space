# RAK Operator Log Summary

Date reviewed: 2026-06-29

## Available logs

- `RAK_MINER_A_REAL_RUN.log`
- `RAK_MINER_B_REAL_RUN.log`

## Observed application-level evidence

`RAK_MINER_A_REAL_RUN.log` includes:

- TX node startup.
- Agent identity display for `researcher-1`.
- A one-packet transmit session.
- Transmission line: `TRANSMITTED - 336 bytes @ 903.9 MHz, SF9`.

`RAK_MINER_B_REAL_RUN.log` includes:

- RX node startup.
- Listening line: `Listening on 903.9 MHz, SF9, 125kHz for 15 seconds`.
- Three received packets.
- RSSI/SNR values: `10 dB / -98 dBm`, `11 dB / -91 dBm`, and `10 dB / -104 dBm`.

## Limit

These are ZK-LoRa operator logs. They are useful, but a grant reviewer may still ask for Semtech HAL, packet-forwarder, Helium gateway, TTN, ChirpStack, `tcpdump udp port 1680`, or SDR evidence to prove the RF packet path from the RAK concentrator to the host process.
