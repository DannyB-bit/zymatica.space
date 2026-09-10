# Forum Readiness Notes

Status: prototype-ready for community review, not production-ready.

Verified locally:
- Python proof runner passes.
- Rust operator builds and `verify_mempool_scanner.py` passes.
- TypeScript operator builds and runs `npm test`.
- Go operator builds and runs `go test` plus `go run . --test`.
- The scanner verifies a decrypted payment event fixture with:
  - memo `ref:demo_packet_hash_hello_zcash_mesh`
  - gross amount `0.05000000 ZEC`
  - developer treasury fee `0.00100000 ZEC`
  - exact 2% integer zatoshi split

Do not overclaim:
- This repo now verifies the decrypted-event contract, not a completed live Zcash scanner.
- Live mode must feed `ZK_LORA_DECRYPTED_EVENT_JSON` or `ZK_LORA_DECRYPTED_EVENT_PATH` from a real wallet/light-client adapter.
- Public explorers cannot decrypt shielded memos.
- The next proof needed for ZCG is a real testnet/regtest shielded payment whose decrypted memo is exported into this event schema.

Forum checklist:
- Lead with the reproducible local verifier.
- State clearly that the Zcash-native wallet/light-client adapter is the next milestone.
- Keep the 2% treasury split as a protocol option/prototype rule, not a claim of Foundation endorsement.
