# Level 8 Weight Compiler — Procedural Seeded Projections
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import struct
import zlib
import json
import numpy as np
import torch
from safetensors.torch import safe_open

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")
GENESIS_VERSION = 12           # Version 12 for Level 8 Procedural Seed

def get_dictionary(dim, dictionary_size, seed):
    """Procedurally generate a normalized dictionary matrix."""
    rng = np.random.RandomState(seed)
    dict_mat = rng.standard_normal((dim, dictionary_size)).astype(np.float32)
    norms = np.linalg.norm(dict_mat, axis=0, keepdims=True) + 1e-9
    return dict_mat / norms

def main():
    model_dir = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
    output_genesis = "j:/Language-U/qwen3.5_procedural.genesis"
    output_seed = "j:/Language-U/ProceduralSeed.LLM"
    
    # Configuration
    R = 3              # Projection Rank
    DICT_SIZE = 256    # Candidates per dimension
    MASTER_SEED = 42   # Shared PRNG master seed
    
    print("=" * 80)
    print("  BUILDING PROCEDURAL SEED -- LEVEL 8 SPARSE DICTIONARY MATCHING PURSUIT")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    # Read config
    with open(os.path.join(model_dir, "config.json"), "r") as f:
        config = json.load(f)
    lang_config = config.get("text_config", config)
    
    hidden = lang_config.get("hidden_size", 1024)
    heads = lang_config.get("num_attention_heads", 8)
    kv_heads = lang_config.get("num_key_value_heads", 2)
    ffn_dim = lang_config.get("intermediate_size", 3584)
    blocks = lang_config.get("num_hidden_layers", 24)
    vocab = lang_config.get("vocab_size", 248320)
    
    # Find safetensors
    st_files = sorted(f for f in os.listdir(model_dir) if f.endswith('.safetensors'))
    if not st_files:
        print("Error: No safetensors found!")
        return
    st_path = os.path.join(model_dir, st_files[0])
    
    # Scan keys
    with safe_open(st_path, framework="pt", device="cpu") as f:
        all_keys = list(f.keys())
    
    # Process only 2D weight matrices (excluding embeddings)
    target_keys = []
    with safe_open(st_path, framework="pt", device="cpu") as f:
        for k in all_keys:
            if "embed_tokens" in k:
                continue
            t = f.get_tensor(k)
            if len(t.shape) == 2:
                target_keys.append(k)
                
    num_layers = len(target_keys)
    print(f"Found {num_layers} weight matrices for procedural projection compression")
    
    # Write genesis v12 file
    print(f"\n[1] Performing sparse dictionary projections and packaging as version 12 genesis...")
    with open(output_genesis, "wb") as fout:
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', GENESIS_VERSION))
        fout.write(WATERMARK)
        fout.write(struct.pack('>I', PERFECT_MAGIC))
        fout.write(struct.pack('>IIIIII', hidden, heads, kv_heads, ffn_dim, blocks, vocab))
        fout.write(struct.pack('>ffff', 1.0, 1.0, 1.0, 1.0))
        fout.write(struct.pack('>I', num_layers))
        
        for idx, key in enumerate(target_keys):
            with safe_open(st_path, framework="pt", device="cpu") as sf:
                W = sf.get_tensor(key).float().numpy()
            
            m, n = W.shape
            
            # Layer-specific seeds
            seed_u = MASTER_SEED + idx * 1000
            seed_v = MASTER_SEED + idx * 1000 + 500
            
            U_dict = get_dictionary(m, DICT_SIZE, seed_u)
            V_dict = get_dictionary(n, DICT_SIZE, seed_v)
            
            layer_payload = bytearray()
            W_res = W.copy()
            
            for rank in range(R):
                scores = U_dict.T @ W_res @ V_dict
                abs_scores = np.abs(scores)
                best_idx = np.argmax(abs_scores)
                idx_u, idx_v = np.unravel_index(best_idx, abs_scores.shape)
                c = scores[idx_u, idx_v]
                
                # Subtract projection
                W_res -= c * np.outer(U_dict[:, idx_u], V_dict[:, idx_v])
                
                # Write rank data: idx_u (1 byte), idx_v (1 byte), c (2 bytes float16)
                layer_payload.append(idx_u)
                layer_payload.append(idx_v)
                layer_payload.extend(struct.pack('>e', float(c)))
            
            name_b = key.encode('utf-8')
            fout.write(struct.pack('>H', len(name_b)))
            fout.write(name_b)
            fout.write(struct.pack('>III', m, n, R))
            fout.write(layer_payload)
            
            if (idx + 1) % 40 == 0 or (idx + 1) == num_layers:
                print(f"    [{idx+1:3d}/{num_layers}] Processed {key[-40:]}")
                
    genesis_size = os.path.getsize(output_genesis)
    print(f"\n[2] Genesis v12 file complete: {genesis_size:,} bytes ({genesis_size/1024:.2f} KB)")
    
    # Zlib level 9 compression to build ProceduralSeed.LLM
    print(f"\n[3] Compressing genesis file with Zlib deflate level 9 to output {output_seed}...")
    with open(output_genesis, "rb") as f_in:
        raw_data = f_in.read()
    
    compressed_data = zlib.compress(raw_data, level=9)
    with open(output_seed, "wb") as f_out:
        f_out.write(compressed_data)
        
    seed_size = os.path.getsize(output_seed)
    print(f"Success! Generated: {output_seed}")
    print(f"  Raw size:        {genesis_size:,} bytes ({genesis_size/1024:.2f} KB)")
    print(f"  Compressed size: {seed_size:,} bytes ({seed_size/1024:.2f} KB)")
    print(f"  Compression:     {genesis_size / seed_size:.2f}x")
    print(f"  Total reduction: {1746942600 / seed_size:,.0f}x vs raw 1.75 GB model!")
    print("=" * 80)

if __name__ == "__main__":
    main()
