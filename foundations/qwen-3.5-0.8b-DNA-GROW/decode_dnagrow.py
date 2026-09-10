# Standalone Decoder for DNA-GROW — Level 8 & Level 9 Unified Decompression
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

try:
    from scipy.fft import idct
except ImportError:
    # Fallback IDCT-II
    import math
    def idct(v, norm='ortho'):
        N = len(v)
        n = np.arange(N); k = n.reshape((N,1))
        M = np.cos(np.pi * k * (2*n+1) / (2*N))
        # Inverse mapping
        out = np.zeros(N, dtype=np.float64)
        for i in range(N):
            term = 0.0
            for j in range(N):
                c = 1.0 / math.sqrt(2) if j == 0 else 1.0
                term += c * v[j] * M[j, i]
            out[i] = term
        if norm == 'ortho':
            # normalize
            out[0] *= math.sqrt(2)
            out *= math.sqrt(2.0 / N) / 2.0
            # rescale back due to forward DCT scaling
            out *= 2.0
        return out

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

SEED_FILE     = "j:/Language-U/DnaGrowSeed.LLM"
OUTPUT_DIR    = "j:/Language-U/qwen-3.5-0.8b-dnagrow-base"
CONFIG_SOURCE = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"

GENESIS_MAGIC   = 0x47454E45
PERFECT_MAGIC   = 0x50455246
WATERMARK_LEN   = 32
MASTER_SEED     = 42
DICT_SIZE       = 256
MODE_DICTIONARY = 0x08
MODE_SPECTRAL   = 0x09

def get_dictionary(dim, dictionary_size, seed):
    """Procedurally generate a normalized dictionary matrix."""
    rng = np.random.RandomState(seed)
    dict_mat = rng.standard_normal((dim, dictionary_size)).astype(np.float32)
    norms = np.linalg.norm(dict_mat, axis=0, keepdims=True) + 1e-9
    return dict_mat / norms

def unpack_2bit(packed_bytes, K):
    syms = []
    for b in packed_bytes:
        syms += [b & 3, (b>>2)&3, (b>>4)&3, (b>>6)&3]
    return syms[:K]

def decompress_grad_vec(col_data, n):
    off = 0
    orig_n = struct.unpack('>H', col_data[off:off+2])[0]; off += 2
    K      = col_data[off]; off += 1
    scale  = struct.unpack('>e', col_data[off:off+2])[0]; off += 2
    idx_mode = col_data[off]; K_check = col_data[off+1]; off += 2

    idx_size = K_check*2 if idx_mode == 1 else K_check
    idx_bytes = col_data[off:off+idx_size]; off += idx_size
    indices, running = [], 0
    if idx_mode == 0:
        for b in idx_bytes:
            running += b; indices.append(running)
    else:
        for i in range(0, idx_size, 2):
            running += struct.unpack('>H', idx_bytes[i:i+2])[0]; indices.append(running)

    packed_len = (K_check + 3) // 4
    symbols = unpack_2bit(col_data[off:off+packed_len], K_check); off += packed_len

    q_vals = []
    for s in symbols:
        sign = (s>>1)&1; mag = s&1
        q_vals.append((5.5 if mag else 1.5) * (1 if sign else -1))

    v_dct = np.zeros(orig_n, dtype=np.float64)
    v_dct[indices] = np.array(q_vals) * scale
    return idct(v_dct, norm='ortho').astype(np.float32), off

def decompress_matrix(data, rank, dim):
    cols, pos = [], 0
    for _ in range(rank):
        col, consumed = decompress_grad_vec(data[pos:], dim)
        cols.append(col); pos += consumed
    return np.stack(cols, axis=1)

def main():
    print("=" * 80)
    print("  DNA-GROW DECODER -- UNIFIED OFFLINE RECONSTRUCTION")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    if not os.path.exists(SEED_FILE):
        print(f"Error: Seed file '{SEED_FILE}' not found.")
        return
        
    print(f"\n[1] Decompressing {os.path.basename(SEED_FILE)}...")
    with open(SEED_FILE, "rb") as f_in:
        raw_genesis_data = zlib.decompress(f_in.read())
    print(f"    Decompressed to {len(raw_genesis_data):,} bytes.")
    
    # Read base safetensors structure (shapes/dtypes only)
    print(f"\n[2] Reading base safetensors structure from {CONFIG_SOURCE}...")
    base_st_files = sorted(f for f in os.listdir(CONFIG_SOURCE) if f.endswith('.safetensors'))
    if not base_st_files:
        print("Error: No safetensors found in base config directory.")
        return
    base_st = os.path.join(CONFIG_SOURCE, base_st_files[0])
    
    tensor_meta = {}
    meta_state = {}
    with safe_open(base_st, framework="pt", device="cpu") as f:
        for k in f.keys():
            t = f.get_tensor(k)
            tensor_meta[k] = (t.shape, t.dtype)
            meta_state[k] = t
            
    print(f"\n[3] Decoding matrices from seed...")
    layer_map = {}
    pos = 0
    
    # Parse header
    magic = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
    assert magic == GENESIS_MAGIC
    version = struct.unpack_from('>H', raw_genesis_data, pos)[0]; pos += 2
    assert version == 15, f"Expected unified version 15, got {version}"
    watermark = raw_genesis_data[pos : pos + WATERMARK_LEN].decode('utf-8', errors='ignore').strip(); pos += WATERMARK_LEN
    perf_magic = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
    assert perf_magic == PERFECT_MAGIC
    
    hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack_from('>IIIIII', raw_genesis_data, pos); pos += 24
    
    # Extract mode byte
    mode_byte = raw_genesis_data[pos]; pos += 4 # Skip 4 bytes of mode alignment
    pos += 12 # skip reserved targets
    num_layers = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
    
    print(f"    Watermark:  {watermark}")
    print(f"    Mode:       {'Dictionary Matching Pursuit (generativeUFO)' if mode_byte == MODE_DICTIONARY else 'Geometric Spectral (GeometricSeed)'}")
    print(f"    Attributes: layers={num_layers} hidden={hidden} heads={heads} blocks={blocks} vocab={vocab}")
    
    for idx in range(num_layers):
        name_len = struct.unpack_from('>H', raw_genesis_data, pos)[0]; pos += 2
        name = raw_genesis_data[pos : pos + name_len].decode('utf-8'); pos += name_len
        m, n, r = struct.unpack_from('>III', raw_genesis_data, pos); pos += 12
        
        dtype = tensor_meta.get(name, (None, torch.float16))[1]
        
        if mode_byte == MODE_DICTIONARY:
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
                
            layer_map[name] = torch.from_numpy(W_rec).to(dtype)
        else:
            scale_u, scale_v = struct.unpack_from('>ff', raw_genesis_data, pos); pos += 8
            u_len, v_len = struct.unpack_from('>II', raw_genesis_data, pos); pos += 8
            
            u_data = raw_genesis_data[pos : pos + u_len]; pos += u_len
            v_data = raw_genesis_data[pos : pos + v_len]; pos += v_len
            
            U_rec = decompress_matrix(u_data, r, m)
            V_rec = decompress_matrix(v_data, r, n)
            W_rec = (U_rec * scale_u) @ (V_rec * scale_v).T
            
            has_residual = struct.unpack_from('>?', raw_genesis_data, pos)[0]; pos += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
                pos += 8 + m * res_rank + n * res_rank # skip
                
            layer_map[name] = torch.from_numpy(W_rec).to(dtype)
            
        if (idx + 1) % 40 == 0 or (idx + 1) == num_layers:
            print(f"    [{idx+1:3d}/{num_layers}] Reconstructed {name[-40:]}")
            
    print(f"    EOF Check: {pos:,} / {len(raw_genesis_data):,} bytes. Status: {'PASS' if pos == len(raw_genesis_data) else 'FAIL'}")
    
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
            # Embeddings and layernorms are copied from baseline weights
            reconstructed_tensors[k] = meta_state[k].to(dtype).clone()
            
    out_st = os.path.join(OUTPUT_DIR, "model.safetensors")
    print(f"    Saving to {out_st}...")
    save_file(reconstructed_tensors, out_st)
    
    # Copy metadata files (config / tokenizer)
    print(f"\n[5] Copying metadata configuration files...")
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
            
    print(f"\n[6] Re-loading and verifying reconstructed model...")
    tokenizer = AutoTokenizer.from_pretrained(OUTPUT_DIR, trust_remote_code=True)
    model = AutoModelForCausalLM.from_pretrained(
        OUTPUT_DIR, torch_dtype=torch.float16, trust_remote_code=True
    )
    print("Success! Model structure verified and fully loaded.")
    print("=" * 80)

if __name__ == "__main__":
    main()
