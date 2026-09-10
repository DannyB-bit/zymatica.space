# node-a-tx Hardware Capture

Captured UTC: Mon Jun 29 23:15:20 UTC 2026

Purpose:
Reviewer-grade Raspberry Pi / RAK evidence for ZK-LoRa Milestone 1.

Included:
- OS and hardware inventory
- SPI / USB device listing
- gateway service status checks
- packet-forwarder / gateway journal checks
- UDP 1680 tcpdump text and pcap capture attempt
- ZK-LoRa verifier and benchmark output
- TX node application test log

Notes:
This capture was collected from host RakMiner-A as node-a-tx. Missing or inactive packet-forwarder services and failed privileged captures are preserved honestly in the corresponding files.

Limitations:
- This artifact directory covers only the locally accessible node-a-tx host.
- node-b-rx and node-c-verifier were not accessible from this session, so no artifacts were fabricated for those nodes.
- tcpdump was installed, but this session could not open the capture interface: see tcpdump_udp1680.txt and tcpdump_udp1680_pcap_command.txt.
- The generated repo-root VERIFICATION_REPORT.md was copied into this directory as VERIFICATION_REPORT.generated_on_pi.md, then the repo-root file was restored to keep this commit scoped to hardware_capture artifacts.
