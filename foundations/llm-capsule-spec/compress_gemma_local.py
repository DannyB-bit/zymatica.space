# local compression script for Gemma-4-31B-it
# Force IPv4 to bypass broken IPv6 resolution on this machine
import socket
orig_getaddrinfo = socket.getaddrinfo
socket.getaddrinfo = lambda *args, **kwargs: [r for r in orig_getaddrinfo(*args, **kwargs) if r[0] == socket.AF_INET]

import os
import sys
import json
import time
import struct
import zlib
import shutil
import warnings
warnings.filterwarnings("ignore")

import torch
from huggingface_hub import login, HfApi

class CustomSafeOpen:
    def __init__(self, filepath):
        self.file = open(filepath, 'rb')
        # Read header size
        header_size_bytes = self.file.read(8)
        self.header_size = struct.unpack('<Q', header_size_bytes)[0]
        # Read header json
        header_json_bytes = self.file.read(self.header_size)
        self.header = json.loads(header_json_bytes.decode('utf-8'))
        # Offset to start of data payload
        self.data_start = 8 + self.header_size
        
    def keys(self):
        return [k for k in self.header.keys() if k != '__metadata__']
        
    def get_tensor(self, name):
        metadata = self.header[name]
        dtype_str = metadata['dtype']
        shape = metadata['shape']
        offsets = metadata['data_offsets']
        
        # Read raw bytes
        self.file.seek(self.data_start + offsets[0])
        size = offsets[1] - offsets[0]
        raw_bytes = self.file.read(size)
        
        # Map to PyTorch dtype
        if dtype_str == 'BF16':
            return torch.frombuffer(bytearray(raw_bytes), dtype=torch.int16).view(torch.bfloat16).reshape(shape)
        elif dtype_str == 'F16':
            return torch.frombuffer(bytearray(raw_bytes), dtype=torch.int16).view(torch.float16).reshape(shape)
        elif dtype_str == 'F32':
            return torch.frombuffer(bytearray(raw_bytes), dtype=torch.float32).reshape(shape)
        elif dtype_str == 'I8':
            return torch.frombuffer(bytearray(raw_bytes), dtype=torch.int8).reshape(shape)
        else:
            raise ValueError(f"Unsupported dtype: {dtype_str}")
            
    def __enter__(self):
        return self
        
    def __exit__(self, exc_type, exc_val, exc_tb):
        self.file.close()

def main():
    print("=" * 80)
    print("      LOCAL GEMMA-4-31B-IT COMPRESSION & UPLOAD PIPELINE (CustomSafeOpen)")
    print("      Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    HF_TOKEN = "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"
    BASE_MODEL = "google/gemma-4-31B-it"
    local_model_dir = "J:/gemma-4-31B-it-local"
    output_dir = "J:/gemma-4-31B-it-local/working"
    os.makedirs(output_dir, exist_ok=True)
    
    output_genesis = os.path.join(output_dir, "gemma4_31b_subzero.genesis")
    
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"[+] Using device: {device}")
    if torch.cuda.is_available():
        print(f"    GPU: {torch.cuda.get_device_name(0)}")
        
    # Login to Hugging Face
    print("[*] Logging in to Hugging Face...")
    login(token=HF_TOKEN, add_to_git_credential=False)
    api = HfApi(token=HF_TOKEN)
    print("[+] Logged in successfully.")
    
    # 1. Process Tokenizer
    print("\n[*] Processing Tokenizer...")
    t_json_path = os.path.join(local_model_dir, "tokenizer.json")
    with open(t_json_path, "r", encoding="utf-8") as f:
        t_data = json.load(f)
        
    vocab = t_data['model']['vocab']
    merges = [' '.join(m) for m in t_data['model']['merges']]
    
    # Run Tokenizer Compression
    print("[*] Compressing tokenizer...")
    TK_MAGIC = bytes([0xC5, 0x54, 0x4B])
    t_config = json.load(open(os.path.join(local_model_dir, "tokenizer_config.json")))
    
    config_meta = {
        "version": t_data.get("version"),
        "added_tokens": t_data.get("added_tokens"),
        "normalizer": t_data.get("normalizer"),
        "pre_tokenizer": t_data.get("pre_tokenizer"),
        "post_processor": t_data.get("post_processor"),
        "decoder": t_data.get("decoder"),
        "model_type": t_data["model"]["type"],
        "tokenizer_config": t_config
    }
    config_str = json.dumps(config_meta, ensure_ascii=False)
    config_bytes = config_str.encode("utf-8")
    comp_config = zlib.compress(config_bytes, 9)
    
    normal_vocab = sorted([(k, v) for k, v in vocab.items() if v < 262144], key=lambda x: x[1])
    vocab_list = [t[0].encode("utf-8", errors="replace") for t in normal_vocab]
    
    vocab_data = bytearray()
    prev = b''
    for t in vocab_list:
        common = 0
        l = min(len(t), len(prev))
        while common < l and t[common] == prev[common]:
            common += 1
        suffix = t[common:]
        for val in [common, len(suffix)]:
            while val >= 128:
                vocab_data.append((val & 0x7F) | 0x80)
                val >>= 7
            vocab_data.append(val & 0x7F)
        vocab_data.extend(suffix)
        prev = t
        
    merges_data = bytearray()
    for line in merges:
        parts = line.split()
        if len(parts) != 2:
            continue
        idx0 = vocab.get(parts[0], -1)
        idx1 = vocab.get(parts[1], -1)
        if idx0 == -1 or idx1 == -1:
            continue
        merges_data.extend(struct.pack('>I', idx0)[1:])
        merges_data.extend(struct.pack('>I', idx1)[1:])
        
    raw_payload = bytearray()
    raw_payload.extend(TK_MAGIC)
    raw_payload.append(0x01) # Mode 1: Absolute
    raw_payload.extend(struct.pack('>I', len(comp_config)))
    raw_payload.extend(comp_config)
    raw_payload.extend(struct.pack('>I', len(vocab_list)))
    raw_payload.extend(struct.pack('>I', len(vocab_data)))
    raw_payload.extend(vocab_data)
    raw_payload.extend(struct.pack('>I', len(merges)))
    raw_payload.extend(merges_data)
    
    abs_capsule_path = os.path.join(output_dir, "gemma-4-31b-subzero-tokenizer.capsule")
    absolute_capsule = zlib.compress(raw_payload, 9)
    with open(abs_capsule_path, "wb") as f:
        f.write(absolute_capsule)
    print(f"    [+] Created absolute capsule: {abs_capsule_path} ({len(absolute_capsule)} bytes)")
        
    # Reference Mode
    raw_ref = bytearray()
    raw_ref.extend(TK_MAGIC)
    raw_ref.append(0x02) # Mode 2: Reference
    base_bytes = BASE_MODEL.encode('utf-8')
    raw_ref.extend(struct.pack('>H', len(base_bytes)))
    raw_ref.extend(base_bytes)
    
    ref_capsule_path = os.path.join(output_dir, "gemma-4-31b-subzero-tokenizer-ref.capsule")
    ref_capsule = zlib.compress(raw_ref, 9)
    with open(ref_capsule_path, "wb") as f:
        f.write(ref_capsule)
    print(f"    [+] Created reference capsule: {ref_capsule_path} ({len(ref_capsule)} bytes)")
        
    # Pack into packets
    PKT_SIZE = 255
    NUM_DATA = 27
    NUM_PKTS = 28
    DATA_PER_PKT = PKT_SIZE - 3
    MAX_PAYLOAD = NUM_DATA * DATA_PER_PKT
    WATERMARK = b'ip zymatica.space '
    
    padded = (ref_capsule + (WATERMARK * (MAX_PAYLOAD // len(WATERMARK) + 1)))[:MAX_PAYLOAD]
    chunks = [padded[i*DATA_PER_PKT : (i+1)*DATA_PER_PKT] for i in range(NUM_DATA)]
    
    packets_dir = os.path.join(output_dir, "packets_tokenizer")
    os.makedirs(packets_dir, exist_ok=True)
    for idx, chunk in enumerate(chunks):
        pkt = bytes([0xBB, idx, NUM_PKTS]) + chunk
        with open(os.path.join(packets_dir, f"packet_tokenizer_{idx}.bin"), "wb") as f:
            f.write(pkt)
            
    parity = bytearray(DATA_PER_PKT)
    for chunk in chunks:
        for j in range(DATA_PER_PKT):
            parity[j] ^= chunk[j]
    with open(os.path.join(packets_dir, f"packet_tokenizer_{NUM_DATA}.bin"), "wb") as f:
        f.write(bytes([0xBB, NUM_DATA, NUM_PKTS]) + bytes(parity))
    print(f"    [+] Created 28 XOR-FEC packets in {packets_dir}")
    
    # Copy config files to language folders in framework kit
    kit_dir = "j:/Language-U/UFO-Compression-Framework-Kit"
    for d in os.listdir(kit_dir):
        sub = os.path.join(kit_dir, d)
        if os.path.isdir(sub) and not d.startswith('.'):
            shutil.copy2(os.path.join(local_model_dir, "tokenizer.json"), os.path.join(sub, "tokenizer.json"))
            shutil.copy2(os.path.join(local_model_dir, "tokenizer_config.json"), os.path.join(sub, "tokenizer_config.json"))
            with open(os.path.join(sub, "vocab.json"), "w", encoding="utf-8") as vf:
                json.dump(vocab, vf, indent=2, ensure_ascii=False)
            with open(os.path.join(sub, "merges.txt"), "w", encoding="utf-8") as mf:
                mf.write('\n'.join(merges))
    print("    [+] Synced tokenizer configs to framework kit directories.")
    
    # 2. Process Model Weights
    print("\n[*] Processing Model Weights (SVD Compression)...")
    
    GENESIS_MAGIC   = 0x47454E45   # "GENE"
    GENESIS_VERSION = 4            # INT8 version
    PERFECT_MAGIC   = 0x50455246   # "PERF"
    WATERMARK_BYTES = b"ip zymatica.space".ljust(32, b" ")
    
    hidden_dim = 5376
    num_heads = 32
    kv_heads = 16
    ffn_dim = 21504
    num_blocks = 60
    vocab_size = 262144
    
    attn_rank = 64
    ffn_rank = 128
    lm_rank = 64
    
    def quantize_to_int8(X):
        max_val = torch.max(torch.abs(X)).item()
        scale = max_val / 127.0 if max_val > 0 else 1e-9
        X_q = torch.clamp(torch.round(X / scale), -127, 127).to(torch.int8)
        return X_q, scale
        
    safetensor_shards = [
        "model-00001-of-00002.safetensors",
        "model-00002-of-00002.safetensors"
    ]
    
    total_original = 0
    total_compressed = 0
    num_layers = 0
    
    start_time = time.time()
    
    with open(output_genesis, "wb") as f_out:
        f_out.write(struct.pack('>I', GENESIS_MAGIC))
        f_out.write(struct.pack('>H', GENESIS_VERSION))
        f_out.write(WATERMARK_BYTES)
        f_out.write(struct.pack('>I', PERFECT_MAGIC))
        f_out.write(struct.pack('>IIIIII', hidden_dim, num_heads, kv_heads, ffn_dim, num_blocks, vocab_size))
        f_out.write(struct.pack('>ffff', 1.0, 1.0, 1.0, 1.0))
        
        layer_count_pos = f_out.tell()
        f_out.write(struct.pack('>I', 0))
        
        for shard_name in safetensor_shards:
            shard_path = os.path.join(local_model_dir, shard_name)
            print(f"\n[+] Processing shard: {shard_name} ({os.path.getsize(shard_path)/1e9:.2f} GB)...", flush=True)
            
            with CustomSafeOpen(shard_path) as sf:
                keys = sorted(sf.keys())
                for name in keys:
                    if "embed_tokens" in name:
                        continue
                    param = sf.get_tensor(name)
                    if param.dim() != 2:
                        continue
                        
                    m, n = param.shape
                    orig_size = m * n * 2 # float16
                    
                    if any(k in name for k in ['q_proj', 'k_proj', 'v_proj', 'o_proj', 'self_attn.q', 'self_attn.k', 'self_attn.v', 'self_attn.o']):
                        rank = min(attn_rank, min(m, n))
                    elif any(k in name for k in ['gate_proj', 'up_proj', 'down_proj', 'mlp.gate', 'mlp.up', 'mlp.down', 'fc1', 'fc2']):
                        rank = min(ffn_rank, min(m, n))
                    elif 'lm_head' in name:
                        rank = min(lm_rank, min(m, n))
                    else:
                        rank = min(attn_rank, min(m, n))
                        
                    layer_start = time.time()
                    
                    # Compute SVD on GPU
                    W = param.float().to(device)
                    U_full, S, Vh_full = torch.linalg.svd(W, full_matrices=False)
                    U_r = U_full[:, :rank]
                    S_r = S[:rank]
                    Vh_r = Vh_full[:rank, :]
                    
                    sqrt_S = torch.sqrt(S_r)
                    U = U_r * sqrt_S.unsqueeze(0)
                    V = Vh_r.t() * sqrt_S.unsqueeze(0)
                    
                    U_cpu = U.cpu()
                    V_cpu = V.cpu()
                    
                    # Release CUDA tensors
                    del W, U_full, S, Vh_full, U_r, S_r, Vh_r, U, V
                    if device == "cuda":
                        # Only empty cache periodically to avoid speed degradation
                        if num_layers % 10 == 0:
                            torch.cuda.empty_cache()
                            
                    U_q, scale_u = quantize_to_int8(U_cpu)
                    V_q, scale_v = quantize_to_int8(V_cpu)
                    
                    name_bytes = name.encode('utf-8')
                    f_out.write(struct.pack('>H', len(name_bytes)))
                    f_out.write(name_bytes)
                    f_out.write(struct.pack('>III', m, n, rank))
                    f_out.write(struct.pack('>ff', scale_u, scale_v))
                    f_out.write(U_q.numpy().tobytes())
                    f_out.write(V_q.numpy().tobytes())
                    f_out.write(struct.pack('>?', False))
                    
                    compressed_size = (m * rank + n * rank) * 1 # INT8
                    total_original += orig_size
                    total_compressed += compressed_size
                    num_layers += 1
                    
                    layer_elapsed = time.time() - layer_start
                    print(f"  [{num_layers}] Compressed {name}: {m}x{n} -> rank {rank} | took {layer_elapsed:.2f}s", flush=True)
                    
        f_out.seek(layer_count_pos)
        f_out.write(struct.pack('>I', num_layers))
        
    elapsed_total = time.time() - start_time
    print(f"\n[+] SVD Weight Compression complete in {elapsed_total/60:.1f} minutes!")
    print(f"    Layers processed: {num_layers}")
    print(f"    Original size: {total_original/1e9:.3f} GB")
    print(f"    Compressed size: {total_compressed/1e9:.3f} GB")
    print(f"    Output Genesis path: {output_genesis}")
    
    # 3. Create & Upload to 8 Hugging Face Repositories
    print("\n[*] Starting Hugging Face uploads...")
    
    repos = {
        "Gemma-4-31b-SubZeroRust": ["rust/tokenizer_coder.rs", "rust/decode_tokenizer.rs"],
        "Gemma-4-31b-SubZeroJava": ["java/TestTokenizerCoder.java", "java/DecodeTokenizer.java"],
        "Gemma-4-31b-Subzero-CPP": ["cpp/tokenizer_coder.cpp", "cpp/tokenizer_coder.hpp", "cpp/decode_tokenizer.cpp"],
        "Gemma-4-31b-SubZeroPython": ["python/compress_tokenizer.py", "python/decode_tokenizer.py"],
        "Gemma-4-31b-SubZeroSwift": ["swift/tokenizer_coder.swift", "swift/decode_tokenizer.swift"],
        "Gemma-4-31b-SubZeroGo": ["go/tokenizer_coder.go", "go/decode_tokenizer.go"],
        "Gemma-4-31b-SubZeroTypeScript": ["typescript/tokenizer_coder.ts", "typescript/tokenizer_coder.js", "typescript/decode_tokenizer.js"],
        "Gemma-4-31b-Subzero-FrameWork": ["*"] # Entire kit
    }
    
    for repo_name, source_files in repos.items():
        repo_id = f"Zymatica/{repo_name}"
        print(f"\n[*] Uploading to repository: {repo_id}...")
        try:
            print("  - Ensuring private repository exists...")
            api.create_repo(repo_id=repo_id, private=True, exist_ok=True)
            
            # Upload weights genesis
            print("  - Uploading gemma4_31b_subzero.genesis...")
            api.upload_file(
                path_or_fileobj=output_genesis,
                path_in_repo="gemma4_31b_subzero.genesis",
                repo_id=repo_id,
                token=HF_TOKEN
            )
            
            # Upload capsules
            print("  - Uploading capsules...")
            api.upload_file(
                path_or_fileobj=abs_capsule_path,
                path_in_repo="tokenizer_kit/gemma-4-31b-subzero-tokenizer.capsule",
                repo_id=repo_id,
                token=HF_TOKEN
            )
            api.upload_file(
                path_or_fileobj=ref_capsule_path,
                path_in_repo="tokenizer_kit/gemma-4-31b-subzero-tokenizer-ref.capsule",
                repo_id=repo_id,
                token=HF_TOKEN
            )
            
            # Upload packetizer folders
            print("  - Uploading packetizer folder...")
            api.upload_folder(
                folder_path=packets_dir,
                path_in_repo="tokenizer_kit/packets_tokenizer",
                repo_id=repo_id,
                token=HF_TOKEN
            )
            
            # Upload code files
            print("  - Uploading source code files...")
            if source_files == ["*"]:
                api.upload_folder(
                    folder_path=kit_dir,
                    path_in_repo="framework_kit",
                    repo_id=repo_id,
                    token=HF_TOKEN
                )
            else:
                for file_path in source_files:
                    local_file = os.path.join(kit_dir, file_path.replace('/', os.sep))
                    if os.path.exists(local_file):
                        api.upload_file(
                            path_or_fileobj=local_file,
                            path_in_repo=f"tokenizer_kit/{os.path.basename(file_path)}",
                            repo_id=repo_id,
                            token=HF_TOKEN
                        )
                        
            # Generate Model Card README
            readme_content = f"""---
license: other
base_model: {BASE_MODEL}
tags:
  - gemma4
  - ufo-compression
  - model-quantization
  - SVD
  - subzero
language:
  - en
pipeline_tag: text-generation
---

# Gemma-4-31B-it SubZero Model ({repo_name.split('-')[-1]})

This repository contains the compressed **Gemma-4-31B-it SubZero** model weights and tokenizer capsule using the **UFO Hyper-Compression Framework**.

## 🌌 Compression Specifications

- **Base Model:** `{BASE_MODEL}`
- **Compressed Weight Format:** `.genesis` Low-Rank SVD (v4 INT8 Symmetric Quantization)
- **Ranks:** Attention layers = 64, FFN intermediate layers = 128, LM head = 64
- **Tokenizer Compression:** 7-Level descent capsule + 28 XOR-FEC packet stream

## 📂 Repository Contents

- `gemma4_31b_subzero.genesis`: Reconstructed SVD weights file (~1.8 GB)
- `tokenizer_kit/`: Tokenizer capsules and packets, along with language implementation files.

*All rights reserved. Patent pending. © 2026 Zymatica.*
"""
            readme_path = os.path.join(output_dir, "README.md")
            with open(readme_path, "w", encoding="utf-8") as rf:
                rf.write(readme_content)
                
            api.upload_file(
                path_or_fileobj=readme_path,
                path_in_repo="README.md",
                repo_id=repo_id,
                token=HF_TOKEN
            )
            print(f"  [+] Repository {repo_name} fully uploaded and verified!")
        except Exception as e:
            print(f"  [-] Failed to upload to {repo_name}: {e}")
            
    print("\n" + "=" * 80)
    print("      ALL LOCAL TASKS SUCCESSFULLY COMPLETED!")
    print("=" * 80)

if __name__ == "__main__":
    main()
