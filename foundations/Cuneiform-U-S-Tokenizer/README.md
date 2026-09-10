---
license: other
tags:
  - ufo-compression
  - cuneiform-u
  - range-coding
  - s-tokenizer
  - entropy-coding
language:
  - en
pipeline_tag: translation
---

![Zymatica Logo](Logo.jpg)

# Cuneiform-U Semantic Vocabulary Range Coder (S-Tokenizer)

This repository contains the official prototype and validation suite for the **Cuneiform-U 6D Vocabulary Range Coder** (the **S-Tokenizer**) developed by **TheAiCollective.art**.

The S-Tokenizer maps the massive 248,320-token vocabulary of the Qwen tokenizer into a 6-dimensional coordinate hypercube radical representation, bypassing classical Shannon entropy limits for low-bandwidth transmissions (e.g. over 915 MHz LoRA channels).

---

## Technical Specifications & Mapping Architecture

Traditional tokenizers map texts into arbitrary numbers, demanding 3-4 bytes per token during raw binary transmission. The **S-Tokenizer** changes this by decomposing tokens into three compact 1-byte radicals representing a 6D semantic hypercube coordinate:

$$R_C = (\text{Domain} \ll 4) \mid \text{Subdomain}$$
$$R_F = (\text{Operation} \ll 4) \mid \text{Modality}$$
$$R_A = (\text{Complexity} \ll 4) \mid \text{Polarity}$$

These radicals map to:
* **Domain & Subdomain:** Domain 1 (Hardware & Networks), Domain 2 (Mathematics & Info Theory), Domain 3 (Dialogue & Persona), Domain 4 (Software & Systems), and Domain 0 (Conversational/General).
* **Operation & Modality:** Identifies core actions (e.g., reset, compile, compress) and context data structures (e.g., bits, wave, capsule).
* **Complexity & Polarity:** Registers scale/depth metrics and outcome states (ACK, NACK, Error).

By running a hierarchical predictor, the decoder predicts the next radicals dynamically based on local context. The range coder only encodes the deviation from these predictions, shrinking the data footprint down to near-zero bit costs when the predictor is accurate.

---

## Local Validation Benchmarks
The validation script `test_semantic_vocab_range_coder.py` compresses test passages and measures size reductions against raw text and token-level zlib deflate (Level 9):

| Test Passage Context | Raw Token Count | ASCII Bytes | Zlib Bytes | Cuneiform-U Bytes | vs ASCII Gain | vs Zlib Gain |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Passage 1 (Hardware/LoRA)** | 27 tokens | 87 B | 93 B | **36 B** | **2.42x** | **2.58x** |
| **Passage 2 (Math/SVD/DCT)** | 18 tokens | 93 B | 74 B | **31 B** | **3.00x** | **2.39x** |
| **Passage 3 (Dialogue/Persona)**| 19 tokens | 85 B | 74 B | **28 B** | **3.04x** | **2.64x** |

**Lossless Guarantee:** Decompression reproduces the original token sequence bit-for-bit, proving absolute mathematical parity.

---

## Licensing Compliance & Intellectual Property Map
This repository contains pure software implementations and is **strictly proprietary** to Zymatica under the Zymatica Proprietary License. It does not include base weight models and is excluded from standard copyleft licensing.

### Proprietary Components — All Rights Reserved, zymatica.space
* **Language-U Framework:** The compression taxonomy and Shannon Bypass entropy calculations.
* **Cuneiform-U Semantic Hypercube System:** The 6D orthogonal coordinate mapping and radical representations.
* **LLM-Logits-Driven Range Coding (LLD-AC):** Active logits-driven probability ranges and entropy coding.
* **Tokenizer Prefix-Suffix Varint Differential Coding:** Lossless varint tokenizer serialization.

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
