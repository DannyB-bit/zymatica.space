# node-a-tx Privileged Packet Capture Retry

Captured UTC: Mon Jun 29 23:46:44 UTC 2026

Purpose:
Retry UDP 1680 packet capture on node-a-tx with privileged tcpdump and an actual local SX1302 transmitter command.

Result:
- UDP 1680 capture did not succeed from this Codex session.
- `sudo -n timeout 120 tcpdump -ni any udp port 1680 -vv` failed immediately because sudo required a password.
- `sudo -n timeout 120 tcpdump -ni any udp port 1680 -w .../udp1680_capture.pcap` failed immediately because sudo required a password.
- `sudo setcap cap_net_raw,cap_net_admin=eip /usr/bin/tcpdump` failed because sudo required an interactive password.
- The non-sudo tcpdump retry failed with `Operation not permitted`.
- No pcap file was created.

Transmit attempt:
The available local hardware transmitter used for this retry was:

`/home/strawberry-z-model-1/LANGUAGEU_28CHIRPS_RF_PROOF_A/lora_tx_driver/A_LORA_TX_FILE --file artifacts/milestone1/hardware_capture/node-a-tx/privileged_retry/zk_lora_tx_payload.bin --frequency-mhz 903.9 --sf 9 --bandwidth-khz 125 --power-dbm 14 --reset-script /home/strawberry-z-model-1/LANGUAGEU_28CHIRPS_RF_PROOF_A/scripts/A_RESET_KNOWN_GOOD_GPIO25_GPIO22.sh`

That transmitter attempt did not complete RF transmission. It failed during SX1302 reset because the reset script requires `sudo -n pinctrl`, and sudo required a password.

Included:
- `privileged_tcpdump_udp1680.txt`: exact stdout/stderr for sudo tcpdump and non-sudo tcpdump retry.
- `privileged_udp1680_capture_command.txt`: exact stdout/stderr for the sudo pcap capture attempt.
- `setcap_attempt.txt`: exact stdout/stderr for the requested tcpdump capability attempt.
- `tx_command.log`: exact command and full output for the local hardware transmitter attempt.
- `zk_lora_payload_source.log`: proof-runner output used as the source for the bounded TX payload file.
- `zk_lora_tx_payload.bin`: 240-byte payload prepared for the TX attempt.
- `payload_build.log`: payload metadata and SHA256.

No evidence was fabricated. This retry documents that additional interactive root privilege, prior `sudo -v`, or root-set tcpdump capabilities are required before this node can produce reviewer-grade UDP 1680 packet capture from this Codex session.
