# Zero-RAM Meta Specification

Date: 2026-06-16

Zero-RAM Meta is the project rule set that lets the Gemma-4-31B Sumerian runtime execute on this PC without allocating dense 31B projection weights.

## Invariants

1. Dense projection weights must never be allocated for the compressed runtime path.
2. The genesis capsule is authoritative for projection tensors.
3. Missing compressed projection records are fatal, not silently replaced by dense zero tensors.
4. Non-projection tensors are seek-read or packaged into compact runtime state.
5. Runtime initialization may allocate persistent factor buffers and reusable workspaces.
6. The autoregressive loop must not allocate CUDA buffers per token.
7. Runtime state must match the base model shape contract:
   - hidden size: 5376
   - language layers: 60
   - attention heads: 32
   - key/value heads: 16
   - vocabulary: 262144
8. Gemma text architecture rules must be explicit:
   - `attention_k_eq_v=true`
   - full-attention layers are every sixth layer starting at layer 5
   - full-attention RoPE uses partial rotary factor 0.25 and theta 1,000,000
   - sliding-attention RoPE uses theta 10,000
   - MLP activation is `gelu_pytorch_tanh`
   - RMSNorm epsilon is `1e-6`

## Proprietary Assets

### Zero-Allocation JIT SVD Swapping

Projection weights are represented as rank-factor INT8 SVD records in the genesis capsule. Runtime initializes the model execution graph around those records and refuses to allocate the dense bf16 projection matrices.

### Strict Shape-Filtered Initializers

Runtime initialization distinguishes scalar layer controls from 5376-wide normalization vectors. This prevents name-based layer matching from mixing `[1]` parameters with full hidden-size norms.

### Dynamic Multimodal CUDA Buffer Sweeping

The broader Python/HF fallback path must sweep incidental CUDA buffers back to the intended execution device when the structure is initialized under meta or CPU contexts. The no-libtorch Rust path avoids most of this class by not instantiating HF modules at all.

## Current Implementation Mapping

- `gemma4_31b_subzero.genesis`: rank-16 compressed projection capsule.
- `genesis_resident_generate.rs`: resident no-libtorch Rust/Zig autoregressive runtime.
- `gemma4_runtime_state.g4rt`: compact real Gemma language norms and q/k norm state.
- `export_gemma4_runtime_state.py`: seek-exporter for the compact runtime state.
- `resident_generation_report.json`: current machine-readable resident proof.
- `zero_ram_meta_selftest.py`: invariant self-test.

## Completion Criteria

Zero-RAM Meta is considered active for a run when:

- genesis validation reports 599 records;
- runtime state validates as `G4RT` version 1 with 60 layers;
- resident report shows `loop_cuda_allocations=0`;
- final hidden state has 5376 finite values;
- generated token ids are present;
- the runtime does not require libtorch or dense projection allocation.



---

## 📜 License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.
