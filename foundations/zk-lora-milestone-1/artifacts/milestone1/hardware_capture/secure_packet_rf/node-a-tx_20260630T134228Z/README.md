# node-a-tx Secure Packet RF TX Evidence

Captured UTC: Tue Jun 30 13:43:48 UTC 2026

Purpose:
Generate an encrypted proof-referenced ZK-LoRa secure packet on-device, then run mandatory LoRa Chirp recovery before TX.

Result:
A generated the secure packet, including reference proof and encrypted payload. Mandatory LoRa Chirp recovery did not produce LORA_CHIRP_RECOVERY_PASS=YES because sudo required a password in this Codex session, so no RF TX was run from this artifact.

Evidence rule:
A proves on-device secure packet generation, reference proof creation, and encrypted payload creation only in this artifact. B must prove RX, decrypt, proof verify, tamper rejection, wrong-key rejection, replay rejection, and matching packet SHA256 in a successful RF run.
