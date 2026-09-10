# Standalone weight decoder/reconstructor for Gemma-4-31B-it SubZero
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import json
import struct
import time
import torch
from safetensors.torch import save_file

def main():
    print("=" * 80)
    print("      GEMMA-4-31B-IT SUBZERO WEIGHT DECODER / RECONSTRUCTOR")
    print("      Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    genesis_path = "J:/gemma-4-31B-it-local/working/gemma4_31b_subzero.genesis"
    output_dir = "J:/gemma-4-31B-it-local/working/reconstructed_gemma4"
    os.makedirs(output_dir, exist_ok=True)
    
    if not os.path.exists(genesis_path):
        print(f"[-] Error: Could not find genesis file at {genesis_path}")
        print("    Please download/place 'gemma4_31b_subzero.genesis' in the current folder.")
        sys.exit(1)
        
    GENESIS_MAGIC   = 0x47454E45   # "GENE"
    GENESIS_VERSION = 4            # INT8 version
    PERFECT_MAGIC   = 0x50455246   # "PERF"
    
    print(f"[*] Reading and parsing genesis file: {genesis_path}...")
    start_time = time.time()
    
    layer_map = {}
    
    with open(genesis_path, "rb") as f:
        magic = struct.unpack('>I', f.read(4))[0]
        assert magic == GENESIS_MAGIC, "Invalid genesis magic header"
        version = struct.unpack('>H', f.read(2))[0]
        assert version == GENESIS_VERSION, f"Unsupported version: {version}"
        watermark = f.read(32).decode('utf-8', errors='ignore').strip()
        perf_magic = struct.unpack('>I', f.read(4))[0]
        assert perf_magic == PERFECT_MAGIC, "Invalid perfect magic header"
        
        hidden_dim, num_heads, kv_heads, ffn_dim, num_blocks, vocab_size = struct.unpack('>IIIIII', f.read(24))
        f.read(16)  # Skip scales/energy targets
        num_layers = struct.unpack('>I', f.read(4))[0]
        
        print(f"    Watermark: {watermark}")
        print(f"    Layers: {num_layers} | blocks: {num_blocks} | hidden: {hidden_dim} | ffn: {ffn_dim}")
        
        for idx in range(num_layers):
            name_len = struct.unpack('>H', f.read(2))[0]
            name = f.read(name_len).decode('utf-8')
            m, n, rank = struct.unpack('>III', f.read(12))
            scale_u, scale_v = struct.unpack('>ff', f.read(8))
            
            # Read int8 SVD vectors
            u_bytes = f.read(m * rank)
            v_bytes = f.read(n * rank)
            
            U_q = torch.frombuffer(bytearray(u_bytes), dtype=torch.int8).reshape(m, rank).float()
            V_q = torch.frombuffer(bytearray(v_bytes), dtype=torch.int8).reshape(n, rank).float()
            
            has_residual = struct.unpack('>?', f.read(1))[0]
            if has_residual:
                # residual block (if any, skipped in default run)
                res_rank = struct.unpack('>I', f.read(4))[0]
                su_r, sv_r = struct.unpack('>ff', f.read(8))
                U_res = torch.frombuffer(bytearray(f.read(m * res_rank)), dtype=torch.int8).reshape(m, res_rank).float() * su_r
                V_res = torch.frombuffer(bytearray(f.read(n * res_rank)), dtype=torch.int8).reshape(n, res_rank).float() * sv_r
                
            # Reconstruct weight matrix: W = (U * su) @ (V * sv).T
            U = U_q * scale_u
            V = V_q * scale_v
            W_rec = U @ V.t()
            
            if has_residual:
                W_rec = W_rec + (U_res @ V_res.t())
                
            # Convert back to bfloat16
            layer_map[name] = W_rec.to(torch.bfloat16)
            
            if (idx + 1) % 50 == 0 or idx + 1 == num_layers:
                print(f"    [{idx+1:3d}/{num_layers}] Reconstructed: {name[-50:]} ({m}x{n} rank {rank})")
                
    # Save the reconstructed model weights into two shards mirroring the original model
    print("\n[*] Sharding and saving reconstructed weights as safetensors...")
    
    # We split tensors based on the original shard mapping:
    # Shard 1 contains layers 0 to 47. Shard 2 contains layers 48 to 59.
    shard_1_tensors = {}
    shard_2_tensors = {}
    
    for name, tensor in layer_map.items():
        is_shard_2 = False
        for i in range(48, 60):
            if f"layers.{i}." in name:
                is_shard_2 = True
                break
        if is_shard_2:
            shard_2_tensors[name] = tensor
        else:
            shard_1_tensors[name] = tensor
            
    # Add zero-initialized non-SVD layers (layernorms, embed_tokens, etc.) to complete weights dictionary
    # The receiver SFT healing loop will restore the values of norms/embeddings.
    print("    - Injecting placeholder non-SVD layers (layernorms, embed_tokens)...")
    
    # Embed tokens shape: [vocab_size, hidden_dim]
    embed_shape = (vocab_size, hidden_dim)
    shard_1_tensors["model.language_model.embed_tokens.weight"] = torch.zeros(embed_shape, dtype=torch.bfloat16)
    
    # Layernorm and position embeddings
    for name in ["model.embed_vision.embedding_projection.bias", "model.language_model.final_layernorm.weight"]:
        shard_2_tensors[name] = torch.zeros((hidden_dim,), dtype=torch.bfloat16)
        
    for i in range(num_blocks):
        target_shard = shard_2_tensors if i >= 48 else shard_1_tensors
        target_shard[f"model.language_model.layers.{i}.input_layernorm.weight"] = torch.zeros((hidden_dim,), dtype=torch.bfloat16)
        target_shard[f"model.language_model.layers.{i}.post_attention_layernorm.weight"] = torch.zeros((hidden_dim,), dtype=torch.bfloat16)
        target_shard[f"model.language_model.layers.{i}.pre_feedforward_layernorm.weight"] = torch.zeros((hidden_dim,), dtype=torch.bfloat16)
        target_shard[f"model.language_model.layers.{i}.post_feedforward_layernorm.weight"] = torch.zeros((hidden_dim,), dtype=torch.bfloat16)
        target_shard[f"model.language_model.layers.{i}.layer_scalar"] = torch.zeros((1,), dtype=torch.bfloat16)
        
    # Save files
    s1_path = os.path.join(output_dir, "model-00001-of-00002.safetensors")
    s2_path = os.path.join(output_dir, "model-00002-of-00002.safetensors")
    
    print(f"    - Saving shard 1 ({len(shard_1_tensors)} tensors) to {s1_path}...")
    save_file(shard_1_tensors, s1_path)
    
    print(f"    - Saving shard 2 ({len(shard_2_tensors)} tensors) to {s2_path}...")
    save_file(shard_2_tensors, s2_path)
    
    elapsed = time.time() - start_time
    print(f"\n[+] Standalone weights reconstruction successfully completed in {elapsed:.1f}s!")
    print(f"    Reconstructed model directory: {output_dir}")
    print("    (Note: run local LoRA SFT healing next to restore full coherence)")
    print("=" * 80)

if __name__ == "__main__":
    main()
