# Milestone 1 Hardware Layout

This document maps the Milestone 1 prototype to a reviewer-verifiable 3-node RAK/Raspberry Pi gateway setup.

## 3-node topology

| Node | Role | Hardware target | Radio role | Software role |
| :--- | :--- | :--- | :--- | :--- |
| Node A | Sender/client | Raspberry Pi 4 or RAK hotspot host with Semtech SX1302/SX1303 concentrator | US915 uplink transmitter | Generates identity, encrypted payload, and reference proof packet |
| Node B | Relay/gateway | Raspberry Pi 4 or RAK V2/MNTD hotspot with SX1302/SX1303 concentrator | LoRaWAN gateway receiver/forwarder | Receives RF packet, validates reference proof, and prepares relay accounting |
| Node C | Control/verifier | Laptop or Raspberry Pi verifier node | Ethernet/Wi-Fi control plane | Runs `verify_all_proofs.py`, benchmark scripts, and log capture checks |

## Packet path

The expected RAK/Helium-compatible packet path is:

1. LoRa RF frame reaches the Semtech SX1302/SX1303 concentrator.
2. The concentrator forwards baseband packet metadata to the Raspberry Pi host over SPI.
3. A Semtech UDP/GWMP packet forwarder, ChirpStack gateway bridge, TTN stack, or Helium `gateway-rs` style gateway process forwards packet metadata.
4. The local ZK-LoRa operator consumes the packet reference, verifies the local proof artifact, and records RSSI/SNR plus routing metadata.
5. Later milestones attach decrypted Zcash wallet/light-client payment events to the same packet reference.

Public explorers are not part of shielded memo decryption. A wallet, light client, or service with the correct viewing capability must provide decrypted payment events.

## Network and radio assumptions

| Setting | Prototype value |
| :--- | :--- |
| Region plan | US915 |
| Example frequency | 903.9 MHz |
| Spreading factor | SF9 |
| Bandwidth | 125 kHz |
| Gateway backhaul | Ethernet or Wi-Fi |
| Packet-forwarder port | UDP 1680 when using Semtech UDP/GWMP |

## Reviewer capture checklist

For a photo/log-backed Raspberry Pi/RAK validation, capture all of the following in one dated session:

- Photo of Node A, Node B, and Node C with labels visible.
- `uname -a`, `cat /etc/os-release`, and `lsusb` or `lspci` on each Raspberry Pi/RAK host.
- Packet-forwarder or gateway service status, for example `systemctl status packet-forwarder`, `systemctl status chirpstack-gateway-bridge`, or `systemctl status helium-gateway`.
- Live packet evidence from one source of truth: `journalctl -u packet-forwarder -f`, Semtech `rxpk` JSON, ChirpStack gateway logs, TTN logs, Helium gateway logs, or `tcpdump -ni any udp port 1680`.
- ZK-LoRa application log showing the same packet reference and matching RSSI/SNR timestamp.
- `python verify_all_proofs.py` and `python benchmark_milestone1.py --iterations 250` run on the Pi or RAK host.

Application-level logs alone are useful supporting evidence, but they do not replace packet-forwarder, Semtech HAL, gateway, or SDR capture evidence.
