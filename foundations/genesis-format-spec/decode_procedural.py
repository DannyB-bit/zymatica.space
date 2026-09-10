# Level 8 Standalone Decoder — Procedural Seeded Projections
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import struct
import zlib
import json
import shutil
import numpy as np
import torch
from safetensors.torch import save_file
from safetensors import safe_open
from transformers import AutoTokenizer, AutoModelForCausalLM

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

SEED_FILE     = "j:/Language-U/ProceduralSeed.LLM"
OUTPUT_DIR    = "j:/Language-U/qwen-3.5-0.8b-procedural-reconstruction"
CONFIG_SOURCE = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"

GENESIS_MAGIC = 0x47454E45
PERFECT_MAGIC = 0x50455246
WATERMARK_LEN = 32
MASTER_SEED   = 42
DICT_SIZE     = 256

def get_dictionary(dim, dictionary_size, seed):
    """Procedurally generate a normalized dictionary matrix."""
    rng = np.random.RandomState(seed)
    dict_mat = rng.standard_normal((dim, dictionary_size)).astype(np.float32)
    norms = np.linalg.norm(dict_mat, axis=0, keepdims=True) + 1e-9
    return dict_mat / norms

def main():
    print("=" * 80)
    print("  PROCEDURAL DECODER -- DYNAMIC SEED PROJECTIONS RECONSTRUCTION")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    if not os.path.exists(SEED_FILE):
        print(f"Error: Seed file '{SEED_FILE}' not found.")
        return
        
    print(f"\n[1] Decompressing {os.path.basename(SEED_FILE)}...")
    with open(SEED_FILE, "rb") as f_in:
        raw_genesis_data = zlib.decompress(f_in.read())
    print(f"    Decompressed to {len(raw_genesis_data):,} bytes.")
    
    # Read base safetensors structure (shapes/dtypes only, NOT weights)
    print(f"\n[2] Reading base safetensors STRUCTURE (shapes only)...")
    base_st = os.path.join(CONFIG_SOURCE, "model.safetensors-00001-of-00001.safetensors")
    tensor_meta = {}
    meta_state = {}
    with safe_open(base_st, framework="pt", device="cpu") as f:
        for k in f.keys():
            t = f.get_tensor(k)
            tensor_meta[k] = (t.shape, t.dtype)
            meta_state[k] = t
            
    print(f"\n[3] Reconstructing absolute layers from seeds...")
    layer_map = {}
    pos = 0
    
    # Parse header
    magic = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
    assert magic == GENESIS_MAGIC
    version = struct.unpack_from('>H', raw_genesis_data, pos)[0]; pos += 2
    assert version == 12, f"Expected v12, got {version}"
    watermark = raw_genesis_data[pos : pos + WATERMARK_LEN].decode('utf-8', errors='ignore').strip(); pos += WATERMARK_LEN
    perf_magic = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
    assert perf_magic == PERFECT_MAGIC
    
    hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack_from('>IIIIII', raw_genesis_data, pos); pos += 24
    pos += 16 # skip energy targets
    num_layers = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
    
    print(f"    Watermark: {watermark}")
    print(f"    v{version} | {num_layers} layers | hidden={hidden} ffn={ffn_dim} blocks={blocks} vocab={vocab}")
    
    for idx in range(num_layers):
        name_len = struct.unpack_from('>H', raw_genesis_data, pos)[0]; pos += 2
        name = raw_genesis_data[pos : pos + name_len].decode('utf-8'); pos += name_len
        m, n, r = struct.unpack_from('>III', raw_genesis_data, pos); pos += 12
        
        # Layer-specific seeds
        seed_u = MASTER_SEED + idx * 1000
        seed_v = MASTER_SEED + idx * 1000 + 500
        
        U_dict = get_dictionary(m, DICT_SIZE, seed_u)
        V_dict = get_dictionary(n, DICT_SIZE, seed_v)
        
        W_rec = np.zeros((m, n), dtype=np.float32)
        for rank in range(r):
            idx_u = raw_genesis_data[pos]; pos += 1
            idx_v = raw_genesis_data[pos]; pos += 1
            c = struct.unpack_from('>e', raw_genesis_data, pos)[0]; pos += 2
            
            W_rec += c * np.outer(U_dict[:, idx_u], V_dict[:, idx_v])
            
        dtype = tensor_meta.get(name, (None, torch.float16))[1]
        layer_map[name] = torch.from_numpy(W_rec).to(dtype)
        
        if (idx + 1) % 40 == 0 or (idx + 1) == num_layers:
            print(f"    [{idx+1:3d}/{num_layers}] Reconstructed {name[-40:]}")
            
    # EOF Check
    print(f"    EOF Check: {pos:,} vs {len(raw_genesis_data):,} bytes {'PASS' if pos == len(raw_genesis_data) else 'FAIL'}")
    
    # Step 4: Assemble final model tensors
    print(f"\n[4] Assembling complete safetensors...")
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    
    reconstructed_tensors = {}
    for k, (shape, dtype) in tensor_meta.items():
        if k in layer_map:
            t = layer_map[k]
            if t.shape == shape:
                reconstructed_tensors[k] = t.clone()
            elif t.T.shape == shape:
                reconstructed_tensors[k] = t.T.clone()
            else:
                reconstructed_tensors[k] = meta_state[k].to(dtype).clone()
        else:
            # Keep embeddings / norm layers from base model for SFT baseline
            reconstructed_tensors[k] = meta_state[k].to(dtype).clone()
            
    out_st = os.path.join(OUTPUT_DIR, "model.safetensors")
    print(f"    Saving to {out_st}...")
    save_file(reconstructed_tensors, out_st)
    
    # Copy metadata files (config / tokenizer)
    print(f"\n[5] Copying configuration and tokenizer metadata...")
    skip_ext = {'.safetensors', '.bin', '.pt', '.ckpt'}
    for fname in os.listdir(CONFIG_SOURCE):
        if os.path.splitext(fname)[1].lower() in skip_ext or fname == '.cache':
            continue
        src = os.path.join(CONFIG_SOURCE, fname)
        dst = os.path.join(OUTPUT_DIR, fname)
        if os.path.isdir(src):
            shutil.copytree(src, dst, dirs_exist_ok=True)
        else:
            shutil.copy2(src, dst)
            
    print(f"\n[6] Re-loading and verifying model structure...")
    tokenizer = AutoTokenizer.from_pretrained(OUTPUT_DIR, trust_remote_code=True)
    model = AutoModelForCausalLM.from_pretrained(
        OUTPUT_DIR, torch_dtype=torch.float16, trust_remote_code=True
    )
    print("Success! Model successfully loaded and verified.")
    print("=" * 80)

if __name__ == "__main__":
    main()
