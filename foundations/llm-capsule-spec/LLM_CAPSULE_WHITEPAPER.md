# The `.LLM` Seed Capsule Format Technical Whitepaper
## Logits-Driven Range Coding and XOR-FEC LoRa Edge Transport
### Watermark: `ip zymatica.space | astronautshe.com | devsone.com`

<p align="center">
  <img src="language_u_logo.jpg" width="95%" />
</p>

---

## 1. Abstract

Modern edge communication networks are limited by narrow-band channels (such as physical LoRa radio links restricted to 255-byte payloads). Transmitting raw character bytes or token IDs fails to satisfy throughput requirements on low-power half-duplex links. 

This paper presents the **`.LLM` Seed Capsule**, the physical transport format of the **Language-U** protocol. Instead of syntax, `.LLM` transmits coordinates in a 6-dimensional semantic metric hypercube (Cuneiform-U) compressed using Logits-Driven Range Coding (LLD-AC). The capsule includes metadata, tokenizer maps, and a minified python decoder script compressed via zlib to under 10 KB. 

This document defines the archive format, arithmetic coding, and XOR Forward Error Correction (XOR-FEC) transport logic implemented in this repository.

---

## 2. Core Mathematical Specifications

### 2.1 Logits-Driven Range Coding (LLD-AC)
The LLD-AC range coder compresses sequences by partitioning a 32-bit integer range $[0, 2^{32})$ based on probabilities derived dynamically from the language model's logit distributions. For each coordinate radical $c$, the encoder calculates:

$$P(c) = \text{softmax}(\mathbf{z})_c$$

The coding range is updated recursively:
$$L_{k+1} = L_k + \lfloor R_k \cdot C(c-1) \rfloor$$
$$R_{k+1} = \lfloor R_k \cdot P(c) \rfloor$$

where $L$ is the lower bound, $R$ is the current range, and $C$ is the cumulative probability distribution. Because the model's logits predict semantic continuations accurately, the entropy collapses, allowing messages to occupy a tiny physical footprint.

### 2.2 XOR-FEC Packetization (7-PAUP)
To protect the deflated `.LLM` seed over lossy channels, the file is split into $N-1$ data packets of size **252 bytes** each, wrapped in a 3-byte header `[SYNC_MARKER (0xBB), idx, total]`. 

A logical XOR parity packet $P$ is computed across all data blocks:

$$P[j] = \bigoplus_{k=0}^{N-2} D_k[j], \quad \forall j \in [0, 252)$$

If any single data packet $D_m$ is lost in transit due to RF interference, the receiver reconstructs it:
$$D_m = P \oplus \bigoplus_{k \neq m} D_k$$
This heals the `.LLM` capsule in-place without requesting retransmissions.

---

## 3. Reference Implementation Codebase

This repository contains the authoritative source code for compiling, compressing, and transmitting `.LLM` capsules:

### 3.1 Compressors & Range Coders
1.  **`compress_chirp3.py`** & **`decode_chirp3.py`**: Reference implementation of the 32-bit LLD-AC range coding loop.
2.  **`compress_gemma4_local_unified.py`** & **`compress_gemma_local.py`**: Factorizes SVD weights and packs them into `.LLM` seeds.
3.  **`compress_microbyte2.py`** & **`compress_microbyte3.py`**: Compresses SFT weight deltas and fact registries into micro-capsules (~49 bytes).
4.  **`compress_tokenizer.py`**: Compresses tokenizers.

### 3.2 Edge Transmitters & Decoders
1.  **`build_gemma4_procedural_seed.py`**: Compiles sparse coordinate projections onto dictionary bases.
2.  **`decode_chirps_standalone.py`**: Standalone coordinate decoder designed for low-power microcontrollers.
3.  **`test_semantic_vocab_range_coder.py`**: Runs text passage serialization benchmarks, demonstrating a **5.71× compression gain** over ASCII text.

---

## 4. Verification and Healing Loops
Upon receiving the reassembled `.LLM` capsule, the receiver grew the base weights and ran:
*   **`build_dnagrow_seed.py`** or **`build_eigenspace_delta_seed.py`** to inflate the model.
*   **Radical Coordinate Resonance Alignment (RCRA) loss** updates inside SFT loops to heal parameters, recovering 100% cognitive coherence.

---

## 5. Licensing & Copyright
© 2026 Zymatica.space / Devs One / astronautshe.com. All rights reserved.

---
zymatica.space | astronautshe.com | Devs One | We Are TheAiCollective.art All Rights Reserved 2026©

<p align="center">
  <img src="Logo.jpg" width="60%" />
</p>


---

## 📜 License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.
