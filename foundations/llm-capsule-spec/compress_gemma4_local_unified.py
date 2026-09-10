import os
import gc
import json
import time
import torch
import numpy as np
from safetensors import safe_open
from safetensors.torch import save_file
from huggingface_hub import HfApi

MODEL_DIR = "J:/gemma-4-31B-it-local"
OUT_FILE = "J:/gemma-4-31B-it-local/gemma4_sumerian_real_weights.safetensors"
HF_TOKEN = "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"
REPO_ID = "TheAiCollectiveART/Gemma-4-31b-Sumerian"

def quantize_to_q8(W_active):
    max_val = np.max(np.abs(W_active))
    scale = max_val / 127.0 if max_val > 0 else 1e-9
    W_q = np.clip(np.round(W_active / scale), -127, 127).astype(np.int8)
    return torch.from_numpy(W_q), float(scale)

def main():
    print("=" * 80)
    print("  GEMMA-4-31B-IT LOCAL UNIFIED COMPRESSOR (CUDA-ACCELERATED)")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)
    
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"[+] Using device for SVD: {device}")
    if torch.cuda.is_available():
        print(f"    GPU: {torch.cuda.get_device_name(0)}")
        
    index_path = os.path.join(MODEL_DIR, "model.safetensors.index.json")
    if not os.path.exists(index_path):
        print(f"[-] Error: Safetensors index file not found at {index_path}")
        return
        
    with open(index_path, "r") as f:
        index_data = json.load(f)
    weight_map = index_data.get("weight_map", {})
    
    # Group keys by shard
    shards = {}
    for param_name, shard_name in weight_map.items():
        if shard_name not in shards:
            shards[shard_name] = []
        shards[shard_name].append(param_name)
        
    print(f"\n[1] Found {len(shards)} shards and {len(weight_map)} total parameters to process.")
    
    compressed_dict = {}
    processed_count = 0
    t_start = time.time()
    
    # Process shard-by-shard to minimize RAM
    for shard_name, param_names in shards.items():
        shard_path = os.path.join(MODEL_DIR, shard_name)
        print(f"\n[*] Processing shard: {shard_name} ({len(param_names)} parameters)...")
        
        with safe_open(shard_path, framework="pt", device="cpu") as f:
            for param_name in param_names:
                is_svd = any(x in param_name for x in ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"])
                
                try:
                    W = f.get_tensor(param_name)
                except Exception as e:
                    print(f"    [-] Error loading tensor {param_name}: {e}")
                    continue
                
                if is_svd and W.ndim == 2:
                    # Determine SVD rank
                    # Use rank 64 for attention projections, and 128 for MLP projections
                    r = 128 if any(x in param_name for x in ["gate_proj", "up_proj", "down_proj"]) else 64
                    
                    # Compute SVD on GPU for ultra-fast performance
                    W_dev = W.float().to(device)
                    U, S, Vt = torch.linalg.svd(W_dev, full_matrices=False)
                    
                    # Extract active components
                    U_active = U[:, :r] * S[:r]
                    V_active = Vt[:r, :].t()
                    
                    # Move back to CPU for quantization
                    U_active_cpu = U_active.cpu().numpy()
                    V_active_cpu = V_active.cpu().numpy()
                    
                    # Quantize active components to Q8 (int8)
                    U_q, scale_u = quantize_to_q8(U_active_cpu)
                    V_q, scale_v = quantize_to_q8(V_active_cpu)
                    
                    # Store in output dict
                    compressed_dict[f"{param_name}.U_q"] = U_q
                    compressed_dict[f"{param_name}.V_q"] = V_q
                    compressed_dict[f"{param_name}.scale_u"] = torch.tensor(scale_u, dtype=torch.float32)
                    compressed_dict[f"{param_name}.scale_v"] = torch.tensor(scale_v, dtype=torch.float32)
                    
                    del W_dev, U, S, Vt, U_active, V_active
                else:
                    # Save non-SVD weights as-is, converted to bfloat16 to save space
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
    
    # 4. Save compressed weights safetensors file
    print(f"\n[2] Writing compressed semantic weights to: {OUT_FILE}")
    save_file(compressed_dict, OUT_FILE)
    print(f"    [+] File written. Size: {os.path.getsize(OUT_FILE) / (1024**2):.2f} MB")
    
    # 5. Upload to Hugging Face
    print(f"\n[3] Uploading compressed weights to Hugging Face repo: {REPO_ID}...")
    api = HfApi(token=HF_TOKEN)
    try:
        api.upload_file(
            path_or_fileobj=OUT_FILE,
            path_in_repo="gemma4_sumerian_real_weights.safetensors",
            repo_id=REPO_ID,
            commit_message="Add unified compressed semantic weights (LRSM) containing SVD and non-SVD matrices for Gemma-4-31B-it"
        )
        print("\n" + "=" * 80)
        print("  SUCCESSFULLY EXTRACTED AND UPLOADED UNIFIED COMPRESSED SEMANTIC WEIGHTS!")
        print(f"  URL: https://huggingface.co/{REPO_ID}")
        print("=" * 80)
    except Exception as e:
        print(f"  [-] HF Upload failed: {e}")

if __name__ == "__main__":
    main()
