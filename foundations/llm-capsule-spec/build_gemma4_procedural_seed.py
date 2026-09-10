# Level 8 Weight Compiler — LOSSY Sparse Dictionary Matching Pursuit for Gemma-4-31B-it
# WARNING: This produces a LOSSY APPROXIMATION. Rank-3 dictionary pursuit captures only
# the top-3 projections per weight matrix. This is NOT lossless compression.
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import struct
import zlib
import json
import numpy as np

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")
GENESIS_VERSION = 12           # Version 12 for Level 8 Procedural Seed

class CustomSafeOpen:
    def __init__(self, filepath):
        self.file = open(filepath, 'rb')
        header_size_bytes = self.file.read(8)
        self.header_size = struct.unpack('<Q', header_size_bytes)[0]
        header_json_bytes = self.file.read(self.header_size)
        self.header = json.loads(header_json_bytes.decode('utf-8'))
        self.data_start = 8 + self.header_size
        
    def keys(self):
        return [k for k in self.header.keys() if k != '__metadata__']
        
    def get_tensor_numpy(self, name):
        metadata = self.header[name]
        dtype_str = metadata['dtype']
        shape = metadata['shape']
        offsets = metadata['data_offsets']
        
        self.file.seek(self.data_start + offsets[0])
        size = offsets[1] - offsets[0]
        raw_bytes = self.file.read(size)
        
        if dtype_str in ('BF16', 'F16'):
            # Convert float16/bfloat16 raw bytes to numpy array
            arr = np.frombuffer(raw_bytes, dtype=np.int16)
            # Standard float16 mapping or cast
            if dtype_str == 'F16':
                arr = arr.view(np.float16)
            else:
                # Bfloat16 manual conversion to float32
                # Interpret as uint16, shift, and view as float32
                u16 = arr.view(np.uint16)
                u32 = np.zeros(len(u16), dtype=np.uint32)
                u32 = u16.astype(np.uint32) << 16
                arr = u32.view(np.float32)
            return arr.reshape(shape).astype(np.float32)
        elif dtype_str == 'F32':
            return np.frombuffer(raw_bytes, dtype=np.float32).reshape(shape)
        elif dtype_str == 'I8':
            return np.frombuffer(raw_bytes, dtype=np.int8).reshape(shape)
        else:
            raise ValueError(f"Unsupported dtype: {dtype_str}")
            
    def __enter__(self):
        return self
        
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.file.close()

def get_dictionary(dim, dictionary_size, seed):
    """Procedurally generate a normalized dictionary matrix."""
    rng = np.random.RandomState(seed)
    dict_mat = rng.standard_normal((dim, dictionary_size)).astype(np.float32)
    norms = np.linalg.norm(dict_mat, axis=0, keepdims=True) + 1e-9
    return dict_mat / norms

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Build lossy procedural seed for Gemma-4-31B")
    parser.add_argument("--model-dir", default=os.environ.get("GEMMA4_MODEL_DIR", "model"),
                        help="Path to Gemma-4-31B-it model directory")
    parser.add_argument("--output-dir", default=None,
                        help="Output directory (default: <model-dir>/working)")
    parser.add_argument("--rank", type=int, default=3, help="Projection rank (default: 3)")
    args = parser.parse_args()

    model_dir = args.model_dir
    output_dir = args.output_dir or os.path.join(model_dir, "working")
    os.makedirs(output_dir, exist_ok=True)
    
    output_genesis = os.path.join(output_dir, "gemma4_31b_procedural.genesis")
    output_seed = os.path.join(output_dir, "ProceduralSeed_gemma4.LLM")
    
    # Configuration
    R = args.rank         # Projection Rank (lossy: higher = better approximation, larger seed)
    DICT_SIZE = 256       # Candidates per dimension
    MASTER_SEED = 42      # Shared PRNG master seed
    
    print("=" * 80)
    print("  BUILDING GEMMA-4 LOSSY PROCEDURAL SEED -- SPARSE DICTIONARY MATCHING PURSUIT")
    print(f"  Projection Rank: {R} (lossy: captures top-{R} projections per matrix)")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    # Read config
    with open(os.path.join(model_dir, "config.json"), "r") as f:
        config = json.load(f)
    
    hidden = config.get("hidden_size", 5376)
    heads = config.get("num_attention_heads", 32)
    kv_heads = config.get("num_key_value_heads", 16)
    ffn_dim = config.get("intermediate_size", 21504)
    blocks = config.get("num_hidden_layers", 60)
    vocab = config.get("vocab_size", 262144)
    
    safetensor_shards = [
        "model-00001-of-00002.safetensors",
        "model-00002-of-00002.safetensors"
    ]
    
    # Scan keys and shapes across shards
    target_keys = []
    print("[*] Scanning safetensors shards for 2D weight matrices...")
    for shard_name in safetensor_shards:
        shard_path = os.path.join(model_dir, shard_name)
        if not os.path.exists(shard_path):
            print(f"Error: Shard file '{shard_path}' does not exist.")
            return
            
        with CustomSafeOpen(shard_path) as sf:
            for k in sf.keys():
                if "embed_tokens" in k:
                    continue
                metadata = sf.header[k]
                shape = metadata['shape']
                if len(shape) == 2:
                    target_keys.append((shard_name, k, shape))
                    
    num_layers = len(target_keys)
    print(f"[+] Found {num_layers} weight matrices for procedural projection compression.")
    
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
        
        for idx, (shard_name, key, shape) in enumerate(target_keys):
            shard_path = os.path.join(model_dir, shard_name)
            with CustomSafeOpen(shard_path) as sf:
                W = sf.get_tensor_numpy(key)
            
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
            
            # Report residual error (reconstruction quality metric)
            orig_norm = np.linalg.norm(W, 'fro')
            res_norm = np.linalg.norm(W_res, 'fro')
            rel_error = res_norm / (orig_norm + 1e-9)
            
            if (idx + 1) % 40 == 0 or (idx + 1) == num_layers:
                print(f"    [{idx+1:3d}/{num_layers}] {key[-40:]}  residual_error={rel_error:.4f}")
                
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
    print(f"  Lossy size ratio vs raw 62.5 GB: {62546338248 / seed_size:,.0f}x")
    print(f"  WARNING: This is a LOSSY approximation (rank-{R} dictionary pursuit).")
    print(f"  The seed does NOT contain enough information for lossless reconstruction.")
    print("=" * 80)

if __name__ == "__main__":
    main()
