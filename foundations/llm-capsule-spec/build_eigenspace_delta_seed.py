import os
import sys
import struct
import zlib
import json
import numpy as np
import torch
from scipy.fft import dct
from safetensors.torch import safe_open

GENESIS_MAGIC   = 0x47454E45   # "GENE"
DELTA_MAGIC     = 0x44454C54   # "DELT" (Level 5 Delta Mode)
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")
GENESIS_VERSION = 5            # v5 for eigenspace delta version

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

def compress_vector(v, K=8):
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

def compress_matrix(W, rank=8, K=8):
    m, n = W.shape
    r = min(rank, min(m, n))
    
    # Run SVD
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
        u_blobs.append(compress_vector(U_norm[:, col], K=K))
        v_blobs.append(compress_vector(V_norm[:, col], K=K))
        
    return b''.join(u_blobs), b''.join(v_blobs), scale_u, scale_v, r

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Build Eigenspace Delta Seed (Level 5 Active)")
    parser.add_argument("--base-dir", default="j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local",
                        help="Path to base reference model safetensors folder")
    parser.add_argument("--tuned-dir", default="j:/Language-U/qwen-3.5-0.8b-reconstructed",
                        help="Path to fine-tuned/target model safetensors folder")
    parser.add_argument("--out-genesis", default="j:/Language-U/qwen3.5_delta.genesis",
                        help="Path to write output delta genesis file")
    parser.add_argument("--out-seed", default="j:/Language-U/EigenspaceDeltaSeed.LLM",
                        help="Path to write output Zlib compressed delta seed")
    parser.add_argument("--rank", type=int, default=8, help="SVD Rank for delta projection")
    parser.add_argument("--k-coef", type=int, default=8, help="Number of DCT coefficients to retain")
    args = parser.parse_args()

    print("=" * 80)
    print("  BUILDING EIGENSPACE DELTA SEED -- LEVEL 5 EIGENSPACE PROJECTION ACTIVE")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    print(f"Base Model Directory:  {args.base_dir}")
    print(f"Tuned Model Directory: {args.tuned_dir}")
    print("=" * 80)

    if not os.path.exists(os.path.join(args.base_dir, "config.json")):
        print(f"Error: Base directory '{args.base_dir}' does not contain config.json")
        return

    # Read config
    with open(os.path.join(args.base_dir, "config.json"), "r") as f:
        config = json.load(f)
    lang_config = config.get("text_config", config)
    
    hidden = lang_config.get("hidden_size", 1024)
    heads = lang_config.get("num_attention_heads", 8)
    kv_heads = lang_config.get("num_key_value_heads", 2)
    ffn_dim = lang_config.get("intermediate_size", 3584)
    blocks = lang_config.get("num_hidden_layers", 24)
    vocab = lang_config.get("vocab_size", 248320)
    
    # Locate safetensors
    base_st_files = sorted(f for f in os.listdir(args.base_dir) if f.endswith('.safetensors'))
    tuned_st_files = sorted(f for f in os.listdir(args.tuned_dir) if f.endswith('.safetensors'))
    
    if not base_st_files or not tuned_st_files:
        print("Error: Safetensors files not found in either base or tuned directory!")
        return
        
    base_st_path = os.path.join(args.base_dir, base_st_files[0])
    tuned_st_path = os.path.join(args.tuned_dir, tuned_st_files[0])
    
    # Scan keys
    with safe_open(base_st_path, framework="pt", device="cpu") as f:
        all_keys = list(f.keys())
    
    # Select target 2D weight matrices (excluding embeddings)
    target_keys = []
    with safe_open(base_st_path, framework="pt", device="cpu") as f:
        for k in all_keys:
            if "embed_tokens" in k:
                continue
            t = f.get_tensor(k)
            if len(t.shape) == 2:
                target_keys.append(k)
                
    num_layers = len(target_keys)
    print(f"Found {num_layers} weight matrices to compute and factorize delta layers.")
    
    print(f"\n[1] Computing delta weights (W_tuned - W_base), running SVD rank-{args.rank}...")
    
    with open(args.out_genesis, "wb") as fout:
        # Header encoding: Magic 'GENE', version, watermark, mode 'DELT'
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', GENESIS_VERSION))
        fout.write(WATERMARK)
        fout.write(struct.pack('>I', DELTA_MAGIC))
        fout.write(struct.pack('>IIIIII', hidden, heads, kv_heads, ffn_dim, blocks, vocab))
        fout.write(struct.pack('>ffff', 0.0, 0.0, 0.0, 0.0))  # reserved
        fout.write(struct.pack('>I', num_layers))
        
        for idx, key in enumerate(target_keys):
            # Load base tensor
            with safe_open(base_st_path, framework="pt", device="cpu") as sb:
                w_base = sb.get_tensor(key).float().numpy()
            # Load tuned tensor
            with safe_open(tuned_st_path, framework="pt", device="cpu") as st:
                w_tuned = st.get_tensor(key).float().numpy()
            
            # Compute eigenspace delta: ΔW = W_tuned - W_base
            w_delta = w_tuned - w_base
            
            # Factorize delta matrix
            u_data, v_data, scale_u, scale_v, r = compress_matrix(w_delta, rank=args.rank, K=args.k_coef)
            
            name_b = key.encode('utf-8')
            fout.write(struct.pack('>H', len(name_b)))
            fout.write(name_b)
            fout.write(struct.pack('>III', w_delta.shape[0], w_delta.shape[1], r))
            fout.write(struct.pack('>ff', scale_u, scale_v))
            fout.write(struct.pack('>II', len(u_data), len(v_data)))
            fout.write(u_data)
            fout.write(v_data)
            fout.write(struct.pack('>?', False))  # has_residual = False
            
            if (idx + 1) % 40 == 0 or (idx + 1) == num_layers:
                print(f"    [{idx+1:3d}/{num_layers}] Processed delta for {key[-40:]}")
                
    genesis_size = os.path.getsize(args.out_genesis)
    print(f"\n[2] Eigenspace Delta genesis file complete: {genesis_size:,} bytes ({genesis_size/1e6:.2f} MB)")
    
    # Compress with Zlib deflate level 9
    print(f"\n[3] Compressing delta genesis file with Zlib deflate level 9 to output {args.out_seed}...")
    with open(args.out_genesis, "rb") as f_in:
        raw_data = f_in.read()
        
    compressed_data = zlib.compress(raw_data, level=9)
    with open(args.out_seed, "wb") as f_out:
        f_out.write(compressed_data)
        
    seed_size = os.path.getsize(args.out_seed)
    print(f"Success! Generated: {args.out_seed}")
    print(f"  Delta Raw size:        {genesis_size:,} bytes ({genesis_size/1e6:.2f} MB)")
    print(f"  Delta Compressed size: {seed_size:,} bytes ({seed_size/1024:.1f} KB)")
    print(f"  Delta Compression:     {genesis_size / seed_size:.2f}x")
    print("=" * 80)

if __name__ == "__main__":
    main()
