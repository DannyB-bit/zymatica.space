# Raspberry Pi / RAK Setup Report

Status: layout documented; hardware photo and packet-forwarder capture pending.

Milestone 1 now includes a 3-node hardware layout in `docs/milestone1_hardware_layout.md`. This local Windows environment cannot truthfully generate Raspberry Pi photos, Semtech HAL logs, packet-forwarder `rxpk` JSON, or Helium/TTN/ChirpStack gateway captures.

## Required evidence before claiming physical validation

- Photo-backed Node A, Node B, and Node C setup.
- Raspberry Pi/RAK OS and hardware inventory commands.
- Packet-forwarder, Semtech HAL, Helium, TTN, ChirpStack, or SDR logs showing the RF packet path.
- ZK-LoRa application logs with matching packet reference and timestamp.
- `python verify_all_proofs.py` and `python benchmark_milestone1.py --iterations 250` run on the Pi or RAK host.

Until that capture is added, the repository should claim Milestone 1 prototype completion and local verifier portability, not completed physical RAK deployment.
