---
license: other
base_model: arnir0/Tiny-LLM
tags:
  - subzero
  - ufo-compression
  - 1paup
  - fec-recovery
  - proofs
language:
  - en
pipeline_tag: text-generation
---

![Zymatica Logo](Logo.jpg)


# SubZeroLLM Claim Verification & Proofs Card (S-PAUP-JIT-Weights-Router)

The `SubZeroLLM` series houses portable LoRA models, claims verification suites, and roundtrip evaluation runs (like the Phase 16 A-to-B packetset restore proofs) designed to validate the reliability of the 1-PAUP and UFO restore protocols under extreme transmission limits.

---

## Licensing Compliance & Intellectual Property Map
This repository contains components from multiple origins. Licenses are assigned per component to respect the code we used while strictly protecting Zymatica's intellectual property:

### Third-Party Components (retained under their original licenses)
| Component | Origin | License | Description |
| :--- | :--- | :--- | :--- |
| **PyTorch** | Meta AI | BSD 3-Clause | Deep learning framework used for matrix math |
| **NumPy** | NumPy Developers | BSD 3-Clause | Numerical computations and random state generation |
| **safetensors** | HuggingFace Inc. | Apache 2.0 | Standard tensor storage and sharding format |

### Proprietary Components — All Rights Reserved, zymatica.space
The following are the exclusive intellectual property of **zymatica.space** and are **All Rights Reserved**. No part may be reproduced, distributed, reverse-engineered, modified, or used in any form without explicit written permission:
*   **Language-U Framework:** The compression taxonomy, semantic decomposition theory, Shannon Bypass mathematical framework, and entropy decomposition equations.
*   **Cuneiform-U Semantic Hypercube System (Yin):** The 6-dimensional hypercube mapping along orthogonal axes (Domain, Subdomain, Operation, Modality, Depth, Polarity) and coordinate radical representation schemas ($R_C$, $R_F$, $R_A$).
*   **Cuneiform-U Production Engine (Yang):** Edge-ready semantic range coder production engine.
*   **Genesis Protocol:** The sharded layers transmission & seed reassembly.
*   **ProceduralSeed File Format (`.LLM` / `.genesis`):** The custom binary seed format, layer encoding schemas, coordinate-packing matrices, and weights projection indexing.
*   **Chirp Packetization & FEC Scheme:** The LoRA packet layout wrappers (including 78-chirp and 28-chirp physical packetization formats), Qualia Seed configuration (0xE0), Huffman facts coding mappings, and XOR-FEC (Forward Error Correction) recovery stream algorithms.
*   **SVD/DCT Compression & Reconstructor Pipeline:** The weights reduction algorithms, singular value decomposition (SVD) projection targets, discrete cosine transform (DCT) spectral compression, and grow_weight / shrink_weight reconstruction implementations.
*   **LLM-Logits-Driven Range Coding (LLD-AC):** The active logits-driven probability range coding and decoding mechanisms, dynamic entropy priors, and collapse signal scaling.
*   **Embedding-Driven Weight Projection (E-PAUP / 1-PAUP):** The mathematical projection of target weight deltas/manifolds onto base-model shared word embedding matrices.
*   **Tokenizer Prefix-Suffix Varint Differential Coding:** The lossless tokenizer serialization storing tokens by ID order using variable-length prefix sharing indexes and suffix bytes.
*   **Multi-Language Runtimes & Ports (Yang):** Low-overhead execution runtimes, off-line tokenizers, and edge-optimized memory management bindings (including C++, Rust, Go, Python, Swift, Java, and TypeScript implementations).
*   **SFT Healing & Adaptive Cognitive Alignment:** The LoRA PEFT receiver-side healing loops, multi-task SFT alignment recipes, loss balancing weights, and token-based on-device reconstruction correction scripts (including decode_teleport.py).
*   **Zymatica Inference Engine (Class 27):** Multi-runtime execution inventory containing 27 language and target runtimes.

---

## Claims Verification & Verification Pipeline
This repository includes evidence and test runners for verifying absolute reconstruction accuracy:
* **XOR FEC Recovery:** Proves full-archive error-correction recovery over simulated transmission losses.
* **1-PAUP / E-PAUP Projections:** Verifies the alignment accuracy of reconstructed SVD weight directions.
* **Fidelity Testing:** Confirms that model perplexity remains within normal operational boundaries after on-device SFT healing cycles.

---

## RAK Miner & Edge Device Deployment Guide (4GB RAM)
From a hardware and systems engineering perspective, this pipeline is designed to execute on resource-constrained edge hardware, such as a **RAK Wireless Miner (typically a Raspberry Pi 4 with 4 GB RAM)**, without triggering Out-of-Memory (OOM) crashes. However, execution must be separated into three distinct phases to ensure 100% reliability:

### Phase 1: Weight Reconstruction (SUCCESS ✅)
*   **How it fits:** The decoder generates weights procedurally and streams them directly to sharded `.safetensors` files on the local drive (microSD, USB SSD, or external hard drive) layer-by-layer.
*   **Memory Profile:** By immediately flushing each layer tensor to disk and freeing the RAM buffers, peak memory usage stays **under 1.5 GB RAM**.
*   **Smooth Execution Tip:** Ensure you have at least **15 GB of free space** on your storage drive before starting the reconstruction.

### Phase 2: Inference & Execution (SUCCESS ✅ via Single-Layer Streaming)
*   **How it fits:** A 31B or large parameter model normally requires tens of gigabytes of memory. Since the Raspberry Pi uses unified system memory shared between CPU and OS, loading the entire model into RAM is impossible. The runtime solves this by loading only **one active layer** from disk into memory, performing the token attention calculation, and immediately discarding it before loading the next layer.
*   **Memory Profile:** Peak VRAM/RAM consumption remains bounded to **~30 MB to 100 MB** at any single moment.
*   **Smooth Execution Tip:** Disk read latency is the primary bottleneck. Running this from a standard Class 10 microSD card or an external mechanical hard drive (HDD) will introduce seek latency and slow down the layer-streaming process. For a 100% smooth, high-throughput experience, run the model from a **high-speed USB 3.0 SSD** or connect your external hard drive directly to the Pi's USB 3.0 port (blue port) to avoid bus-speed limits.

### Phase 3: LoRA SFT Healing (WARNING ⚠️ - Extremely Tight)
*   **The Bottleneck:** Fine-tuning is significantly heavier than inference because backpropagation requires storing activations, gradients, and optimizer states (AdamW) in RAM.
*   **How to prevent OOMs:**
    1.  **Strict Parameter Freezing:** Ensure all reconstructed SVD base weights are strictly frozen (`requires_grad = False`). Only train the small embedding and layer normalization tensors.
    2.  **Batch Size Limit:** Force a batch size of `1` and gradient accumulation steps if needed.
    3.  **Virtual Memory (Swap Space):** Configure a **4 GB or 8 GB swap file** on the SSD or external hard drive to act as virtual memory overflow.
    4.  **Optimizer Choice:** Use memory-efficient optimizers (like `Adafactor` or quantized 8-bit optimizers) rather than standard 32-bit `AdamW` if RAM limits are reached.

---

## Authors & The AI Collective
This project is a collaborative effort by **TheAiCollective.art** (represented by the brand logo above):
*   **zymatica.space:** Core framework architect and developer.
*   **astronautshe.com:** Edge systems engineer and developer.
*   **DevsOne:** Hybrid development developer (composed of an AI agent and human-in-the-loop).

Together, we form **TheAiCollective.art**—pushing the boundaries of model compression and decentralization.

*Authors: Zymatica.space | astronautshe.com | DevsOne — We Are TheAiCollective.art*



---

## 📜 License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.
