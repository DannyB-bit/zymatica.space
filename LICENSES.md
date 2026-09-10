# Zymatica Intellectual Property & Licensing Architecture

## 1. Primary License: Zymatica Commercial & Novel-Holder Covenant License (v2.0)

All original engineering architectures, algorithms, protocols, semantic codecs, and runtime implementations in this repository are governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](./LICENSE)**, which is a **Source-Available** licensing model:

- **Source-Available Terms**: The source code is publicly accessible and transparent for inspection, audit, and reproducible verification.
- **Individual Developers, Researchers, and Students**:
  Granted a perpetual, worldwide, non-exclusive license upon owning a legally purchased copy (Kindle eBook, Paperback, or Hardcover) of the novel:
  > **"200 AMSTERDAM: THE VERTICAL CITY"**  
  > *Book One of ZYMATICA A TRILOGY* by Danny Bouldiez (Available on Amazon.com)

- **Commercial Companies, AI Labs, and Enterprise Deployers**:
  Commercial rights are granted either by maintaining verifiable Amazon.com purchase receipts of novel copies equivalent to at least **10% of total active workforce, including employees and direct technical contractors**, OR through an explicit commercial enterprise agreement executed directly with [zymatica.space](https://zymatica.space).

- **OSI Open-Source Designation**: Upstream and standalone components licensed under MIT, Apache-2.0, or BSD-3-Clause remain standard **Open-Source** under OSI definitions.

---

## 2. Inventions & Proprietary Modules Covered

The Covenant License covers all 35 Classes of Zymatica Inventions, including but not limited to:
- **Classes 01–27**: Native Qwen3.5/Gemma Runtimes, LoRaWAN Chirp Packetizers, Cuneiform 6D Manifolds, ZK-VDE Decryption Circuits.
- **Class 28**: `neural_swarm_hypergraph` — Ephemeral subagent spawning, hypergraph routing, authenticated quorum consensus.
- **Class 29**: `hyper_manifold_kv_folding` — Continuous trajectory parametric knot LUT compression for KV attention caches.
- **Class 30**: `holomorphic_speculative_engine` — Holomorphic velocity draft candidate projection and fused tree attention.
- **Class 31**: `epigenetic_manifold_crystallizer` — Gram-Schmidt nullspace weight projection and 70-byte crystal serialization.
- **Class 32**: `cuneiform_8d_octonion_engine` — Non-associative Cayley-Dickson octonion algebra ($\mathbb{O}$) and lossless 8D geodesic codecs.
- **Class 33**: `zymatica-zspar` — Model-independent finite-field $\text{GF}(16)$ $\text{RS}(12,8)$ semantic parity error correction and auto-repair.
- **Class 34**: `z_wormhole` — Cross-model zero-shot direct latent thought transfer protocol [EVIDENCE: evidence/10_00/latest/wormhole.json].
- **Class 35**: `z_mcts` — Continuous manifold test-time latent reasoning engine on 8D Riemannian geodesics.

---

## 3. Third-Party & Upstream Open-Source Software Components

Per Section 3 of the Covenant License, all third-party open-source libraries, base neural model architectures, and cryptographic dependencies incorporated into or interoperating with this workspace retain their respective original licenses and developer attributions:

| Component / Subsystem | Upstream Creator / Project | Upstream License | Attribution & Role |
| :--- | :--- | :--- | :--- |
| **Qwen-3.5 Base Model** | Alibaba Cloud / Qwen Team | Apache-2.0 / Qwen License | Base neural architecture, transformer blocks, and initial parameter priors for Cuneiform VLM, DNA-GROW, and 28-Chirp Morphogenesis. |
| **Gemma Architecture** | Google DeepMind / Gemma Team | Google Gemma Terms / Apache-2.0 | Base architecture for English Hidden-State Steering (EHSS) and SVD low-rank tensor compression. |
| **Hugging Face Tokenizers** | Hugging Face Inc. | Apache-2.0 | BPE and WordPiece tokenization infrastructure and vocabulary mapping (`tokenizers`, `transformers`, `safetensors`). |
| **Arkworks zkSNARKs** | Arkworks Contributors | MIT / Apache-2.0 | Groth16 proof synthesis and BN254 pairing-friendly elliptic curve arithmetic (`ark-groth16`, `ark-bn254`, `ark-relations`, `ark-ff`, `ark-ec`). |
| **Dalek Cryptography** | Dalek Cryptography | BSD-3-Clause | Curve25519 and Ed25519 fast high-security scalar multiplication (`ed25519-dalek`, `curve25519-dalek`). |
| **PyTorch Core** | Linux Foundation / PyTorch | BSD-3-Clause | Reference tensor operations, autograd engine, and evaluation harnesses. |
| **ONNX Runtime** | Microsoft Corporation | MIT | Open Neural Network Exchange graph optimization and execution. |
| **zlib Compression Library** | Jean-loup Gailly & Mark Adler | Zlib License | High-performance DEFLATE/zlib compression for neural voice audio frames and telemetry payloads. |
| **Tokio / Axum / Rayon** | Tokio & Rayon Contributors | MIT | Async I/O, thread-pool parallelism, and web microservice execution. |
| **Solana Program SDK** | Solana Labs / Anza | Apache-2.0 | On-chain Anchor program state router and BPF byte-order serialization. |
| **Claude Shannon (1948)** | Mathematical Foundation | Public Domain / Academic | *A Mathematical Theory of Communication* ($H(X) = -\sum P(x_i) \log_2 P(x_i)$). |

No term of the Zymatica Covenant License restricts, modifies, or diminishes any rights granted to the public under these upstream open-source licenses for their respective third-party library files. `zymatica-agent-harness` is maintained outside the default Rust workspace as reference evaluation infrastructure.
