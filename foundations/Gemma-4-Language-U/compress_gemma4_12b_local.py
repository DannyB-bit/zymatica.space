# Gemma-4-12B-it Local SVD/Q8 Unified Compressor
# Watermark: ip zymatica.space | astronautshe.com | Gemma-4-Language-U

import os
import gc
import json
import time
import argparse
import torch
import numpy as np
from safetensors import safe_open
from safetensors.torch import save_file

def quantize_to_q8(W_active):
    """Quantize weight matrix to 8-bit integers with a float scale factor."""
    max_val = np.max(np.abs(W_active))
    scale = max_val / 127.0 if max_val > 0 else 1e-9
    W_q = np.clip(np.round(W_active / scale), -127, 127).astype(np.int8)
    return torch.from_numpy(W_q), float(scale)

def main():
    parser = argparse.ArgumentParser(description="Gemma-4-12B-it Local SVD Compressor")
    parser.add_argument("--model-dir", type=str, required=True, help="Directory containing google/gemma-4-12b-it weights")
    parser.add_argument("--out-file", type=str, required=True, help="Path to output gemma4_12b_sumerian_real_weights.safetensors")
    parser.add_argument("--attn-rank", type=int, default=64, help="SVD Rank for attention projections (q, k, v, o)")
    parser.add_argument("--mlp-rank", type=int, default=128, help="SVD Rank for MLP projections (gate, up, down)")
    args = parser.parse_args()

    print("=" * 80)
    # Human-readable model name for prompt context
    print("  GEMMA-4-12B-IT LOCAL UNIFIED COMPRESSOR (Gemma-4-Language-U)")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"[*] SVD Execution Device: {device}")
    if torch.cuda.is_available():
        print(f"    - GPU: {torch.cuda.get_device_name(0)}")
        
    index_path = os.path.join(args.model_dir, "model.safetensors.index.json")
    if not os.path.exists(index_path):
        print(f"[-] Error: Safetensors index file not found at {index_path}")
        return
        
    with open(index_path, "r") as f:
        index_data = json.load(f)
    weight_map = index_data.get("weight_map", {})
    
    # Group keys by shard file
    shards = {}
    for param_name, shard_name in weight_map.items():
        if shard_name not in shards:
            shards[shard_name] = []
        shards[shard_name].append(param_name)
        
    print(f"\n[1] Identified {len(shards)} shards and {len(weight_map)} total parameters to process.")
    
    compressed_dict = {}
    processed_count = 0
    t_start = time.time()
    
    for shard_name, param_names in shards.items():
        shard_path = os.path.join(args.model_dir, shard_name)
        print(f"\n[*] Processing shard: {shard_name} ({len(param_names)} parameters)...")
        
        with safe_open(shard_path, framework="pt", device="cpu") as f:
            for param_name in param_names:
                is_svd = any(proj in param_name for proj in ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"])
                
                try:
                    W = f.get_tensor(param_name)
                except Exception as e:
                    print(f"    [-] Error loading tensor {param_name}: {e}")
                    continue
                
                if is_svd and W.ndim == 2:
                    # Select SVD rank based on projection type
                    r = args.mlp_rank if any(proj in param_name for proj in ["gate_proj", "up_proj", "down_proj"]) else args.attn_rank
                    
                    # Bound rank to the smaller matrix dimension
                    r = min(r, W.shape[0], W.shape[1])
                    
                    # Run GPU-accelerated SVD
                    W_dev = W.float().to(device)
                    U, S, Vt = torch.linalg.svd(W_dev, full_matrices=False)
                    
                    # Scale components to absorb singular values
                    U_active = U[:, :r] * torch.sqrt(S[:r])
                    V_active = Vt[:r, :].t() * torch.sqrt(S[:r])
                    
                    U_active_cpu = U_active.cpu().numpy()
                    V_active_cpu = V_active.cpu().numpy()
                    
                    # Quantize singular vectors to Q8
                    U_q, scale_u = quantize_to_q8(U_active_cpu)
                    V_q, scale_v = quantize_to_q8(V_active_cpu)
                    
                    # Save components to output dictionary
                    compressed_dict[f"{param_name}.U_q"] = U_q
                    compressed_dict[f"{param_name}.V_q"] = V_q
                    compressed_dict[f"{param_name}.scale_u"] = torch.tensor(scale_u, dtype=torch.float32)
                    compressed_dict[f"{param_name}.scale_v"] = torch.tensor(scale_v, dtype=torch.float32)
                    
                    del W_dev, U, S, Vt, U_active, V_active
                else:
                    # Keep non-projection parameters as-is in bfloat16
                    compressed_dict[param_name] = W.to(torch.bfloat16)
                    
                del W
                gc.collect()
                if device.type == "cuda":
                    torch.cuda.empty_cache()
                
                processed_count += 1
                if processed_count % 100 == 0:
                    print(f"    [Progress] Processed {processed_count}/{len(weight_map)} parameters...")
                    
        gc.collect()
        
    print(f"\n[+] Successfully compressed/extracted {processed_count} layers in {time.time() - t_start:.2f}s.")
    
    # Save the output file
    os.makedirs(os.path.dirname(args.out_file), exist_ok=True)
    print(f"\n[2] Writing compressed weights to: {args.out_file}")
    save_file(compressed_dict, args.out_file)
    print(f"    [+] Saved successfully. File size: {os.path.getsize(args.out_file) / (1024**2):.2f} MB")

if __name__ == "__main__":
    main()
