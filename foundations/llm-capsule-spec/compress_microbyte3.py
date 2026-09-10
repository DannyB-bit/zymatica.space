# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
import struct
import zlib
import torch
import numpy as np
from safetensors.torch import load_file

CAPSULE_PATH = "j:/Language-U/qwen-3.5-0.8b-microbyte-3.capsule"
MODEL_DIR = "j:/Language-U/SubZeroLLM-LORA/model"
ADAPTER_PATH = "j:/Language-U/SubZeroLLM-LORA/adapter_strong/adapter_model.safetensors"
MAGIC = bytes([0xA7, 0x07, 0xC4])

def main():
    print("=" * 72)
    print("  QWEN-3.5-0.8B-MICROBYTE-3 CAPSULE GENERATOR (E-PAUP OPTIMIZED)")
    print("  Watermark: ip zymatica.space")
    print("=" * 72)

    # 1. Load Embeddings
    print("Loading base model embeddings...")
    model_files = [f for f in os.listdir(MODEL_DIR) if f.endswith(".safetensors")]
    if not model_files:
        print("[-] Error: No safetensors files found.")
        return

    embed_weights = None
    for f in model_files:
        try:
            sd = load_file(os.path.join(MODEL_DIR, f))
            for k in sd.keys():
                if "embed_tokens.weight" in k:
                    embed_weights = sd[k].float()
                    print(f"[+] Loaded embeddings: {embed_weights.shape}")
                    break
            if embed_weights is not None:
                break
        except Exception as e:
            print(f"[-] Error reading {f}: {e}")

    if embed_weights is None:
        print("[-] Error: Embeddings not found.")
        return

    V, d = embed_weights.shape
    # Determine adaptive index size: 2 bytes if V <= 65535, else 3 bytes
    idx_size = 2 if V <= 65535 else 3
    print(f"Adaptive index size set to: {idx_size} bytes (Vocab: {V})")

    # 2. Load Adapter Tensors
    print("\nLoading adapter weights...")
    adapter_sd = load_file(ADAPTER_PATH)
    target_keys = sorted(list(adapter_sd.keys()))
    
    # 3. Build Capsule Payload
    payload = bytearray()
    payload.extend(MAGIC)

    # L0 SFT Recipe Header (13 bytes)
    seed = 0xA11E4
    lr = 2e-4
    steps = 150
    qualia_seed = 0b_01_11_10_00
    
    payload.extend(struct.pack('>I', seed))
    payload.extend(struct.pack('>e', lr))
    payload.extend(struct.pack('>H', steps))
    payload.extend(bytes([qualia_seed]))
    payload.extend(bytes([idx_size])) # 1 byte for index size

    # Target Fact Count
    payload.extend(bytes([18])) # 18 Q&A facts

    # Write compressed weight update tensors
    print("\nCompressing weight tensors...")
    K = 3 # Number of matching embedding vectors for large dimensions

    for key_idx, key in enumerate(target_keys):
        tensor = adapter_sd[key].float() # shape: (R, M)
        R, M = tensor.shape
        
        # Check if transposing benefits compression
        transposed = False
        if M < 128 and R >= 128:
            tensor = tensor.t()
            R, M = tensor.shape
            transposed = True
            
        # 1 byte key index, 2 bytes M, 2 bytes R
        payload.append(key_idx)
        payload.extend(struct.pack('>H', M))
        payload.extend(struct.pack('>H', R))

        if M >= 128:
            # Mode: 2 if transposed else 1
            mode = 2 if transposed else 1
            payload.append(mode)
            
            for i in range(R):
                row = tensor[i]
                # Partition row into chunks of size d
                for start in range(0, M, d):
                    end = min(start + d, M)
                    chunk_sz = end - start
                    chunk = row[start:end]
                    
                    # Slice embedding table for this chunk size
                    E_chunk = embed_weights[:, :chunk_sz]
                    E_chunk_norms = torch.nn.functional.normalize(E_chunk, p=2, dim=1)
                    chunk_norm = torch.nn.functional.normalize(chunk, p=2, dim=0)
                    
                    # Search top-K similarities
                    sims = torch.matmul(E_chunk_norms, chunk_norm)
                    top_k_vals, top_k_indices = torch.topk(torch.abs(sims), K)
                    
                    # Solve for coefficients
                    E_sub = E_chunk[top_k_indices].t()
                    res = torch.linalg.lstsq(E_sub, chunk.unsqueeze(1))
                    c = res.solution.squeeze(1)

                    # Pack indices and coefficients
                    for idx in top_k_indices.tolist():
                        if idx_size == 2:
                            payload.extend(struct.pack('>H', idx))
                        else:
                            payload.extend(struct.pack('>I', idx)[1:])
                    for coef in c.tolist():
                        payload.extend(struct.pack('>e', coef))
        else:
            # Mode: 3 if transposed else 0
            mode = 3 if transposed else 0
            payload.append(mode)
            for i in range(R):
                for j in range(M):
                    payload.extend(struct.pack('>e', tensor[i, j].item()))

    print(f"\nRaw binary payload size: {len(payload)} bytes")

    # Zlib compress
    compressed = zlib.compress(payload, 9)
    print(f"Compressed capsule size (Zlib): {len(compressed)} bytes")

    # Save to disk
    with open(CAPSULE_PATH, "wb") as f:
        f.write(compressed)
    print(f"[+] Successfully wrote capsule to {CAPSULE_PATH}")
    print("=" * 72)

if __name__ == "__main__":
    main()
