---
title: "ZK-LoRa Privacy Layer - Milestone 2"
language:
- en
tags:
- zero-knowledge
- zk-snark
- lora
- privacy
- zcash
- depin
license: conditional-grant
---

# ZK-LoRa Privacy Layer — Milestone 2

*Zcash Shielded Micropayments & Shielded Payment Listener for Private IoT Mesh Networks*

![ZK-LoRa Privacy Layer Logo](./logo.png)

### 📖 Download [ZK_LoRa_Whitepaper.pdf](./ZK_LoRa_Whitepaper.pdf) (18-Page PDF)

> *"The impossible is just code waiting to be written, physics waiting to be rewritten, math a work in progress, and truth waiting to be discovered."*

## 🎥 Video Presentation

An official video presentation introducing the ZK-LoRa Privacy Layer is available:

- **Web Platform:** [Watch on Objkt (Tezos NFT #110)](https://objkt.com/tokens/KT1PMD1QSurTyVXxcYDUPAjQt8DNShkBiv4m/110)
- **Direct CDN Stream:** [Direct Video Stream (MP4)](https://assets.objkt.media/file/assets-003/bafybeig2duv7vbxjrww3vmcxkxjqaikyc3hexepauvhuaemepk42v37hji/artifact)
- **Decentralized Storage:** [IPFS Gateway (Filebase)](https://ipfs.filebase.io/ipfs/bafybeig2duv7vbxjrww3vmcxkxjqaikyc3hexepauvhuaemepk42v37hji)

<p align="center">
  <video src="https://assets.objkt.media/file/assets-003/bafybeig2duv7vbxjrww3vmcxkxjqaikyc3hexepauvhuaemepk42v37hji/artifact" poster="https://assets.objkt.media/file/assets-003/bafkreihay7l4wsc5ztv3rbi3mvjsdydw5eiqvuznxvrujlsdtvppnzwefy/artifact" width="100%" controls></video>
</p>

---

## Overview

Milestone 2 implements a deterministic decrypted-payment event verifier for routing rewards. The scanner validates a wallet/light-client event containing a memo reference and verifies that a **2% programmatic developer fee** is split and routed to the developer treasury (`u10rjztjhk6c2caz6t6hdh32zcf22exhumlm388vtd7exm63vsgwphhm5gt2azgzdksaumr9hn5hx7yy3tdjvdpt875c9tjqswwshz2v9d`).

This component implements:
1. **Decrypted Event Verifier** - Consumes a wallet/light-client decrypted payment event from JSON or a fixture.
2. **Programmatic Dev Fee Split (2%)** - Verifies that a 2% cut is sent to the developer address.
3. **Reference Payment Matching** - Links the unique packet hash inside the memo reference (`ref:<hash>`) with cached LoRa frames.

Live Zcash scanning is the next integration step. A gateway must provide decrypted events from a real Zcash wallet or light-client adapter; public explorers cannot decrypt shielded memos.

## Files

| File | Purpose |
| :--- | :--- |
| [WHITEPAPER.md](./WHITEPAPER.md) | Full ZK-LoRa Zcash specification with threat model & security analysis |
| [verify_mempool_scanner.py](./verify_mempool_scanner.py) | Milestone 2 verification script compiling Rust daemon and validating a decrypted payment event |
| [verify_all_proofs.py](./verify_all_proofs.py) | Master orchestrator verifying ZK proofs across 20 programming languages |
| [run_proof.py](./run_proof.py) | ZK-SNARK prover/verifier implementation + CI proof runner |

## Quick Start

```bash
# Run the decrypted payment event verifier and developer fee verification suite
python verify_mempool_scanner.py
```

## Security & Payment Properties

| Property | Status |
| :--- | :---: |
| Decrypted event payout matching | ✅ |
| 2% Programmatic Developer fee split | ✅ |
| Live wallet/light-client scanner | Next |
| Replay protection | ✅ |

## License

Conditional Open-Source Grant License — see [LICENSE](./LICENSE)
