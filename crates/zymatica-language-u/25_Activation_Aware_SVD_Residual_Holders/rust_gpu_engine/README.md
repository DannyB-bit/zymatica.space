# Language-U-LLM High-Speed Rust-Zig GPU Execution Engine
**Watermark: ip zymatica.space | astronautshe.com**

This directory contains the high-speed, zero-copy Rust-Zig Hybrid CUDA execution engine built to accelerate the low-rank SVD projections of the Gemma-4-31B prior under strict memory budgets.

## 🚀 Key Architectural Innovations
1. **FFI Dynamic CUDA DLL Loader:** Dynamically locates and loads CUDA libraries at runtime, preventing Windows compiler toolchains from stripping out implicit dependencies.
2. **Phase-Separated SVD Pipeline:** Divides projection into separate Phase 1 (reduction) and Phase 2 (expansion) kernel launches, bypassing fused kernel redundancies that trigger a 168× compute regression.
3. **Zero-Allocation Batching:** Pre-allocates static scratchpads in GPU VRAM and streams in-place CPU-to-GPU updates via direct token embedding lookups.

---

## 🛠️ Prerequisites
To build and execute the hybrid engine, your environment must meet the following requirements:
*   **Rust:** Cargo and `rustc` compiler installed (edition 2021).
*   **Zig Compiler:** `zig` (v0.12.0 or v0.13.0) added to your system path.
*   **Python / PyTorch:** Python 3.10+ containing a CUDA-enabled PyTorch installation (such as PyTorch 2.5.1+cu121).

---

## 📦 Compilation & Building

Use the python build helper script to compile the release binary:
```bash
python build_rust_inference.py
```
This script automatically:
1. Detects your active Python PyTorch installation path.
2. Appends the native `torch/lib` directory to the linker search paths.
3. Compiles the Zig static library (`sumerian_cuda_core.zig`) using target `-target x86_64-windows-msvc` and flags `-O ReleaseFast`.
4. Builds the final Rust release binary at `target/release/run_gemma4_cuda_hybrid.exe`.

---

## 🏃 Execution Instructions

Run the compiled executable by setting the target model paths and appending the PyTorch DLL libraries to your system path:

### Windows (PowerShell)
```powershell
# Set configuration variables
$env:GEMMA4_MODEL_DIR="J:/gemma-4-31B-it-local"
$env:GEMMA4_GENESIS="J:/gemma-4-31B-it-local/working/gemma4_31b_subzero.genesis"
$env:GEMMA4_CUNEIFORM_BIN="./gemma4_vocab_cuneiform.bin"

# Set parallel execution batch size (B=1 or B=8)
$env:GEMMA4_BATCH_SIZE="1"

# Append PyTorch DLL path and run the engine
$env:PATH="C:\Users\freed\AppData\Local\Programs\Python\Python310\lib\site-packages\torch\lib;" + $env:PATH
./target/release/run_gemma4_cuda_hybrid.exe
```

### Linux / WSL2
```bash
export GEMMA4_MODEL_DIR="J:/gemma-4-31B-it-local"
export GEMMA4_GENESIS="J:/gemma-4-31B-it-local/working/gemma4_31b_subzero.genesis"
export GEMMA4_CUNEIFORM_BIN="./gemma4_vocab_cuneiform.bin"
export GEMMA4_BATCH_SIZE="8"

export LD_LIBRARY_PATH="/home/user/.local/lib/python3.10/site-packages/torch/lib:$LD_LIBRARY_PATH"
./target/release/run_gemma4_cuda_hybrid
```

---
*Governed by Zymatica Covenant License 2.0. Patent pending. © 2026 Zymatica Space.*
