# Gemma-4-31B Safetensors to Genesis Converter
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import json
import struct
import time
import torch
from safetensors import safe_open

GENESIS_MAGIC   = 0x47454E45   # "GENE"
GENESIS_VERSION = 4            # INT8 version
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")

def main():
    print("=" * 80)
    print("  GEMMA-4-31B SAFETENSORS TO GENESIS CONVERTER")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    input_safetensors = "J:/gemma-4-31B-it-local/working/gemma4_sumerian_real_weights.safetensors"
    output_genesis = "J:/gemma-4-31B-it-local/working/gemma4_31b_subzero.genesis"
    
    if not os.path.exists(input_safetensors):
        print(f"[-] Error: Safetensors file not found at {input_safetensors}")
        sys.exit(1)
        
    print(f"[*] Reading from: {input_safetensors}")
    print(f"[*] Writing to:   {output_genesis}\n")
    
    t_start = time.time()
    
    # 1. Inspect safetensors header to identify SVD layers
    print("[*] Inspecting safetensors keys...")
    svd_layers = {}
    
    with safe_open(input_safetensors, framework="pt", device="cpu") as f:
        keys = list(f.keys())
        
    for key in keys:
        if key.endswith(".U_q"):
            layer_name = key[:-4]  # Remove ".U_q"
            svd_layers[layer_name] = {}
            
    print(f"    - Found {len(svd_layers)} SVD-compressed projection weight matrices.")
    
    # 2. Open and load SVD values
    print("[*] Loading SVD parameters...")
    with safe_open(input_safetensors, framework="pt", device="cpu") as f:
        for layer_name in svd_layers.keys():
            U_q = f.get_tensor(f"{layer_name}.U_q")
            V_q = f.get_tensor(f"{layer_name}.V_q")
            scale_u = f.get_tensor(f"{layer_name}.scale_u")
            scale_v = f.get_tensor(f"{layer_name}.scale_v")
            
            # Extract scalar values from scale tensors
            su_val = float(scale_u.item()) if scale_u.ndim == 0 or len(scale_u) == 1 else float(scale_u[0])
            sv_val = float(scale_v.item()) if scale_v.ndim == 0 or len(scale_v) == 1 else float(scale_v[0])
            
            m, rank = U_q.shape
            n, rank_v = V_q.shape
            assert rank == rank_v, f"Rank mismatch for {layer_name}: {rank} vs {rank_v}"
            
            svd_layers[layer_name] = {
                "U_q": U_q.to(torch.int8),
                "V_q": V_q.to(torch.int8),
                "scale_u": su_val,
                "scale_v": sv_val,
                "m": m,
                "n": n,
                "rank": rank
            }
            
    print("    [+] Successfully loaded SVD matrices.")
    
    # 3. Write .genesis file (Version 4 Format)
    print(f"[*] Packaging into .genesis format...")
    
    # Architecture config for Gemma-4-31B
    hidden_dim = 5376
    num_heads = 32
    kv_heads = 16
    ffn_dim = 21504
    num_blocks = 60
    vocab_size = 262144
    
    with open(output_genesis, "wb") as fout:
        # Write Header
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', GENESIS_VERSION))
        fout.write(WATERMARK)
        fout.write(struct.pack('>I', PERFECT_MAGIC))
        
        # Arch params
        fout.write(struct.pack('>IIIIII', hidden_dim, num_heads, kv_heads, ffn_dim, num_blocks, vocab_size))
        
        # Energy targets (attn, ffn, lm, other)
        fout.write(struct.pack('>ffff', 1.0, 1.0, 1.0, 1.0))
        
        # Layer count
        fout.write(struct.pack('>I', len(svd_layers)))
        
        # Process and write each layer
        for name, data in svd_layers.items():
            name_bytes = name.encode('utf-8')
            fout.write(struct.pack('>H', len(name_bytes)))
            fout.write(name_bytes)
            
            # Dimensions and rank
            fout.write(struct.pack('>III', data["m"], data["n"], data["rank"]))
            
            # Scales
            fout.write(struct.pack('>ff', data["scale_u"], data["scale_v"]))
            
            # INT8 SVD factors
            fout.write(data["U_q"].numpy().tobytes())
            fout.write(data["V_q"].numpy().tobytes())
            
            # Residual flag (no residual blocks in this output format)
            fout.write(struct.pack('>?', False))
            
    elapsed = time.time() - t_start
    print(f"\n[+] SUCCESS: Packaged .genesis file saved to {output_genesis}")
    print(f"    File size: {os.path.getsize(output_genesis) / (1024**2):.2f} MB")
    print(f"    Completed in {elapsed:.1f}s.")
    print("=" * 80)

if __name__ == "__main__":
    main()
