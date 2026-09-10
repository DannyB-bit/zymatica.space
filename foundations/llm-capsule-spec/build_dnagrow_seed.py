# Unified Weight Compiler for DNA-GROW — Level 8 (Dictionary) & Level 9 (Geometric Spectral)
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import struct
import zlib
import json
import numpy as np
import torch
import argparse
from safetensors.torch import safe_open

try:
    from scipy.fft import dct
except ImportError:
    import math
    def dct(v, norm='ortho'):
        N = len(v)
        n = np.arange(N); k = n.reshape((N,1))
        M = np.cos(np.pi * k * (2*n+1) / (2*N))
        out = 2 * np.dot(M, v.astype(np.float64))
        out[0] /= math.sqrt(2)
        return (out / math.sqrt(2*N))

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")
VERSION_DNA_GROW = 15          # Unified DNA-GROW Version

# Mode Magic numbers
MODE_DICTIONARY = 0x08         # Level 8 matching pursuit mode
MODE_SPECTRAL   = 0x09         # Level 9 SVD-DCT geometric mode

def get_dictionary(dim, dictionary_size, seed):
    """Procedurally generate a normalized dictionary matrix."""
    rng = np.random.RandomState(seed)
    dict_mat = rng.standard_normal((dim, dictionary_size)).astype(np.float32)
    norms = np.linalg.norm(dict_mat, axis=0, keepdims=True) + 1e-9
    return dict_mat / norms

def pack_2bit_array(symbols):
    pad_len = (4 - (len(symbols) % 4)) % 4
    if pad_len > 0:
        symbols.extend([0] * pad_len)
    packed = bytearray()
    for i in range(0, len(symbols), 4):
        b = (
            (symbols[i] & 0x03) |
            ((symbols[i+1] & 0x03) << 2) |
            ((symbols[i+2] & 0x03) << 4) |
            ((symbols[i+3] & 0x03) << 6)
        )
        packed.append(b)
    return bytes(packed)

def compress_vector_dct(v, K=8):
    n = len(v)
    K = min(K, n)
    v_dct = dct(v.astype(np.float64), norm='ortho')
    top_idx = np.sort(np.argsort(np.abs(v_dct))[-K:])
    vals = v_dct[top_idx]
    
    scale = float(np.abs(vals).max()) / 5.5 + 1e-9
    
    symbols = []
    for x in vals:
        sign = 1 if x >= 0 else 0
        mag = 1 if abs(x) > 3.5 * scale else 0
        symbols.append((sign << 1) | mag)
        
    deltas = np.diff(np.concatenate([[0], top_idx]))
    if deltas.max() > 255:
        idx_mode = 1
        idx_bytes = b''.join(struct.pack('>H', int(d)) for d in deltas)
    else:
        idx_mode = 0
        idx_bytes = bytes(deltas.astype(np.uint8).tolist())
        
    packed_vals = pack_2bit_array(symbols)
    
    col_header = struct.pack('>H', n) + bytes([K]) + struct.pack('>e', scale) + bytes([idx_mode, K])
    return col_header + idx_bytes + packed_vals

def compress_matrix_spectral(W, rank=8, K=8):
    m, n = W.shape
    r = min(rank, min(m, n))
    
    # Run randomized SVD
    W_t = torch.from_numpy(W).float()
    U, S, V = torch.svd_lowrank(W_t, q=r, niter=3)
    
    # Absorb square root of S
    sqrt_S = torch.sqrt(S.clamp(min=0))
    U_scaled = (U * sqrt_S.unsqueeze(0)).numpy().astype(np.float32)
    V_scaled = (V * sqrt_S.unsqueeze(0)).numpy().astype(np.float32)
    
    scale_u = float(np.abs(U_scaled).max()) / 1.0 + 1e-9
    scale_v = float(np.abs(V_scaled).max()) / 1.0 + 1e-9
    
    U_norm = U_scaled / scale_u
    V_norm = V_scaled / scale_v
    
    u_blobs = []
    v_blobs = []
    for col in range(r):
        u_blobs.append(compress_vector_dct(U_norm[:, col], K=K))
        v_blobs.append(compress_vector_dct(V_norm[:, col], K=K))
        
    return b''.join(u_blobs), b''.join(v_blobs), scale_u, scale_v, r

def main():
    parser = argparse.ArgumentParser(description="Unified Weight Compiler for DNA-GROW")
    parser.add_argument("--model-dir", default="j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local",
                        help="Path to base model directory")
    parser.add_argument("--out-seed", default="j:/Language-U/DnaGrowSeed.LLM",
                        help="Path to write output DNA-GROW seed LLM capsule")
    parser.add_argument("--mode", choices=["dictionary", "spectral"], default="dictionary",
                        help="Compression mode: dictionary (generativeUFO) or spectral (GeometricSeed)")
    parser.add_argument("--rank", type=int, default=8, help="SVD/projection rank")
    parser.add_argument("--dict-size", type=int, default=256, help="Candidates per dimension (dictionary mode)")
    parser.add_argument("--master-seed", type=int, default=42, help="Shared PRNG master seed")
    parser.add_argument("--k-coef", type=int, default=8, help="DCT coefficients to retain (spectral mode)")
    
    args = parser.parse_args()
    
    print("=" * 80)
    print(f"  BUILDING DNA-GROW SEED CAPSULE -- MODE: {args.mode.upper()}")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    if not os.path.exists(os.path.join(args.model_dir, "config.json")):
        print(f"Error: Model directory '{args.model_dir}' does not contain config.json")
        return

    # Read config
    with open(os.path.join(args.model_dir, "config.json"), "r") as f:
        config = json.load(f)
    lang_config = config.get("text_config", config)
    
    hidden = lang_config.get("hidden_size", 1024)
    heads = lang_config.get("num_attention_heads", 8)
    kv_heads = lang_config.get("num_key_value_heads", 2)
    ffn_dim = lang_config.get("intermediate_size", 3584)
    blocks = lang_config.get("num_hidden_layers", 24)
    vocab = lang_config.get("vocab_size", 248320)
    
    # Locate safetensors
    st_files = sorted(f for f in os.listdir(args.model_dir) if f.endswith('.safetensors'))
    if not st_files:
        print("Error: No safetensors found!")
        return
    st_path = os.path.join(args.model_dir, st_files[0])
    
    # Scan keys
    with safe_open(st_path, framework="pt", device="cpu") as f:
        all_keys = list(f.keys())
    
    # Process only 2D weight matrices (excluding embeddings)
    target_keys = []
    with safe_open(st_path, framework="pt", device="cpu") as f:
        for k in all_keys:
            if "embed_tokens" in k or "lm_head" in k:
                continue
            t = f.get_tensor(k)
            if len(t.shape) == 2:
                target_keys.append(k)
                
    num_layers = len(target_keys)
    print(f"Found {num_layers} weight matrices for projection compression.")
    
    temp_genesis = args.out_seed + ".tmp.genesis"
    mode_byte = MODE_DICTIONARY if args.mode == "dictionary" else MODE_SPECTRAL
    
    with open(temp_genesis, "wb") as fout:
        # Header formatting
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', VERSION_DNA_GROW))
        fout.write(WATERMARK)
        fout.write(struct.pack('>I', PERFECT_MAGIC))
        fout.write(struct.pack('>IIIIII', hidden, heads, kv_heads, ffn_dim, blocks, vocab))
        fout.write(bytes([mode_byte, 0, 0, 0])) # Inject mode details into reserved bytes
        fout.write(b'\x00' * 12 # reserved targets
        )
        fout.write(struct.pack('>I', num_layers))
        
        for idx, key in enumerate(target_keys):
            with safe_open(st_path, framework="pt", device="cpu") as sf:
                W = sf.get_tensor(key).float().numpy()
            
            m, n = W.shape
            name_bytes = key.encode('utf-8')
            
            fout.write(struct.pack('>H', len(name_bytes)))
            fout.write(name_bytes)
            fout.write(struct.pack('>III', m, n, args.rank))
            
            if args.mode == "dictionary":
                # Layer-specific seeds
                seed_u = args.master_seed + idx * 1000
                seed_v = args.master_seed + idx * 1000 + 500
                
                U_dict = get_dictionary(m, args.dict_size, seed_u)
                V_dict = get_dictionary(n, args.dict_size, seed_v)
                
                layer_payload = bytearray()
                W_res = W.copy()
                
                for r in range(args.rank):
                    scores = U_dict.T @ W_res @ V_dict
                    abs_scores = np.abs(scores)
                    best_idx = np.argmax(abs_scores)
                    idx_u, idx_v = np.unravel_index(best_idx, abs_scores.shape)
                    c = scores[idx_u, idx_v]
                    
                    # Subtract projection
                    W_res -= c * np.outer(U_dict[:, idx_u], V_dict[:, idx_v])
                    
                    layer_payload.append(idx_u)
                    layer_payload.append(idx_v)
                    layer_payload.extend(struct.pack('>e', float(c)))
                
                fout.write(layer_payload)
            else:
                # Spectral mode
                u_data, v_data, scale_u, scale_v, r_actual = compress_matrix_spectral(W, rank=args.rank, K=args.k_coef)
                fout.write(struct.pack('>ff', scale_u, scale_v))
                fout.write(struct.pack('>II', len(u_data), len(v_data)))
                fout.write(u_data)
                fout.write(v_data)
                fout.write(struct.pack('>?', False)) # has_residual = False
                
            if (idx + 1) % 40 == 0 or (idx + 1) == num_layers:
                print(f"    [{idx+1:3d}/{num_layers}] Compressed {key[-40:]}")
                
    # Zlib compress
    print(f"\n[3] Deflating temporary genesis format with Zlib level 9...")
    with open(temp_genesis, "rb") as f_in:
        raw_data = f_in.read()
        
    compressed_data = zlib.compress(raw_data, level=9)
    with open(args.out_seed, "wb") as f_out:
        f_out.write(compressed_data)
        
    # Clean up temp file
    if os.path.exists(temp_genesis):
        os.remove(temp_genesis)
        
    raw_size = len(raw_data)
    seed_size = os.path.getsize(args.out_seed)
    
    print("=" * 80)
    print(f"Success! Generated: {args.out_seed}")
    print(f"  Raw weight structure representation: {raw_size:,} bytes")
    print(f"  Compressed seed capsule size:        {seed_size:,} bytes ({seed_size/1024:.2f} KB)")
    print(f"  Compression ratio vs raw format:      {raw_size / seed_size:.2f}x")
    print(f"  Overall compression ratio vs raw model: {1746942600 / seed_size:,.0f}x")
    print("=" * 80)

if __name__ == "__main__":
    main()
