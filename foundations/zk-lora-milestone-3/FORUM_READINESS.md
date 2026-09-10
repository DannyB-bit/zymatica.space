# Forum Readiness Notes

Status: prototype-ready for community review, not production-ready.

Verified locally:
- Python proof runner passes.
- Rust operator builds and `verify_mempool_scanner.py` passes.
- TypeScript operator builds and runs `npm test`.
- Go operator builds and runs `go test` plus `go run . --test`.
- Decrypted payment event verification uses the same 2% zatoshi split contract as milestone 2.
- Multi-hop, marketplace, HAL, ToF, and neighbor-audit paths are executable simulations/demos.

Do not overclaim:
- Multi-hop routing, HAL radio output, ZK-PoD, ToF, and neighbor auditing are not yet production RF evidence.
- Real hardware proof still needs packet-forwarder, Semtech HAL, ChirpStack, TTN, Helium, or SDR logs.
- Mempool protection and HTLC examples are design stubs until backed by Zcash-compatible implementation.

Forum checklist:
- Present milestone 3 as a mesh/security simulation harness.
- Separate simulation results from hardware results.
- Ask for review on the minimum next hardware proof: one compact packet, one receiver capture, and one matching Zcash memo reference.
