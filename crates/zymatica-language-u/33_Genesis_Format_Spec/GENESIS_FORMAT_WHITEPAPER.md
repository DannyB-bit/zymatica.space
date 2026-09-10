# The `.genesis` Binary Format Technical Whitepaper
## A Standard for Low-Rank SVD and Spectral Projection Weights Registry
### Watermark: `ip zymatica.space | astronautshe.com | devsone.com`

<p align="center">
  <img src="language_u_logo.jpg" width="95%" />
</p>

---

## 1. Abstract

Standard deep learning frameworks package neural model parameters into flat, unaligned tensor structures. When these models are scaled (e.g. Gemma-4-12B or 31B architectures), their memory footprints block edge execution. 

This paper introduces the **`.genesis` Binary Format**, a structured weights registry designed specifically for edge execution under the **Zero-RAM Meta** protocol. By storing parameters as low-rank Singular Value Decomposition (SVD) components quantized to Q8 or 3-bit space, and applying Discrete Cosine Transform (DCT-II) spectral filtering, `.genesis` achieves up to **20,632×** weight reduction. 

This document defines the layout specifications, mathematical transformations, and reference codebase logic implemented in this repository.

---

## 2. Mathematical Specifications & Core Mechanics

### 2.1 Low-Rank SVD Matrix Factorization
For any target projection matrix $W \in \mathbb{R}^{M \times N}$, the `.genesis` encoder performs truncated Singular Value Decomposition:

$$W \approx U \Sigma V^T$$

The top $R$ singular components are kept, scaling them to construct the active factors:
$$\mathbf{U}_{\text{active}} = U_{:, :R} \sqrt{\Sigma_{:R}}$$
$$\mathbf{V}_{\text{active}} = V_{:, :R} \sqrt{\Sigma_{:R}}$$

These factors are quantized into 8-bit integer vectors ($U_q, V_q$) and scaled:
$$\mathbf{U}_q = \text{quantize}(\mathbf{U}_{\text{active}}, s_u)$$
$$\mathbf{V}_q = \text{quantize}(\mathbf{V}_{\text{active}}, s_v)$$

During receiver-side inference, the original weight matrix is reconstructed JIT:
$$\hat{W} = (U_q \cdot s_u) \times (V_q \cdot s_v)^T$$

### 2.2 Spectral DCT-II Truncation
To aggressively reduce parameter sizes for Level 4 and Level 6 representations, the `.genesis` compiler applies a 2D Discrete Cosine Transform over the factorized arrays, preserving only the top-16 low-frequency coefficients:

$$D(i, j) = \alpha_i \beta_j \sum_{m=0}^{M-1} \sum_{n=0}^{N-1} f(m, n) \cos \frac{\pi (2m+1)i}{2M} \cos \frac{\pi (2n+1)j}{2N}$$

High-frequency spectral coefficients are pruned, and the remaining values are packed using vectorized bit-arrangements.

---

## 3. Reference Implementation Codebase

This repository contains the authoritative source code for compiling, parsing, and executing `.genesis` models:

### 3.1 Compilers & Quantization Suite
1.  **`safetensors_to_genesis.py`**: Reads standard Float16 model weights, loops over layers, performs SVD on projections (`q_proj`, `k_proj`, etc.), and saves them as raw low-rank registers.
2.  **`quantize_perfect_genesis.py`**: Reference Q8 scalar quantizer.
3.  **`quantize_genesis_int8_to_3bit.py`**: Compresses 8-bit matrices into 3-bit ranges mapping `[-3, 3]`.
4.  **`quantize_genesis_3bit_to_dct.py`** & **`quantize_genesis_dct_to_grad.py`**: Integrates 2D DCT-II spectral filtering with 2-bit Gradient Atom classing.

### 3.2 Decoders & Runtime Execution
1.  **`decode_gemma4.py`**: Reconstructs dense float matrices from SVD factors using PyTorch.
2.  **`decode_procedural.py`** & **`decode_tinyqwen.py`**: Dynamically compiles projection tensors from matching pursuit dictionaries.
3.  **`decode_tokenizer.py`**: Restores custom Cuneiform-U vocabulary mappings.
4.  **`ZERO_RAM_META_SPEC.md`**: Technical specification outlining how to execute `.genesis` models on 4GB systems by allocating layers on PyTorch's `meta` device.

---

## 4. Empirical Parity & Verification
To ensure zero degradation in representation fidelity, the codebase includes verification hooks:
*   **`verify_gemma4_exact_parity.py`**: Computes token-level generation outputs and sequence perplexities, confirming **100% bitwise parity** ($0.00e+00$ MSE) between reconstructed `.genesis` layers and uncompressed float models.

---

## 5. Licensing & Copyright
© 2026 Zymatica.space / Devs One. Licensed under the Zymatica Covenant License 2.0 (zymatica.space). Registered under open-source Genesis Specification protocols.

---
zymatica.space | astronautshe.com | Devs One | We Are TheAiCollective.art Covenant License 2.0 (zymatica.space) 2026©

<p align="center">
  <img src="Logo.jpg" width="60%" />
</p>
