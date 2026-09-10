# Gemma-4-12B-it Direct Weight Reconstructor from SVD/Q8 safetensors
# Watermark: ip zymatica.space | astronautshe.com | Gemma-4-Language-U

import os
import gc
import json
import torch
from safetensors import safe_open
from safetensors.torch import save_file

def get_shard_id(key, layers_count=48):
    if "language_model.layers." not in key:
        return 10
    parts = key.split('.')
    try:
        idx = parts.index("layers") + 1
        layer_num = int(parts[idx])
    except:
        layer_num = 0
    return min(10, layer_num // (layers_count // 10) + 1)

def reconstruct_model(svd_weights_path, output_dir, text_config):
    print("=" * 80)
    print("  GEMMA-4-12B DIRECT RECONSTRUCTION ENGINE")
    print("  Rebuilding dense weights from SVD factors...")
    print("=" * 80)
    
    os.makedirs(output_dir, exist_ok=True)
    
    # 1. Open SVD weights safetensors
    ref_file = safe_open(svd_weights_path, framework="pt", device="cpu")
    ref_keys = set(ref_file.keys())
    
    # Identify unique base keys (strip suffix like .U_q, .V_q, .scale_u, .scale_v)
    base_keys = set()
    for k in ref_keys:
        for suffix in [".U_q", ".V_q", ".scale_u", ".scale_v"]:
            if k.endswith(suffix):
                base_keys.add(k[:-len(suffix)])
                break
        else:
            base_keys.add(k)
            
    print(f"[+] Found {len(base_keys)} base parameters.")
    
    # 2. Reconstruct parameters and group by shard
    shards = {sh: {} for sh in range(1, 11)}
    
    for base_key in sorted(base_keys):
        shard_id = get_shard_id(base_key, layers_count=48)
        
        # Check if SVD components are present
        if f"{base_key}.U_q" in ref_keys:
            U_q = ref_file.get_tensor(f"{base_key}.U_q")
            V_q = ref_file.get_tensor(f"{base_key}.V_q")
            scale_u = ref_file.get_tensor(f"{base_key}.scale_u").item()
            scale_v = ref_file.get_tensor(f"{base_key}.scale_v").item()
            
            # Reconstruct: W = (U_q * scale_u) @ (V_q * scale_v).t()
            U = U_q.float() * scale_u
            V = V_q.float() * scale_v
            W = U @ V.t()
            shards[shard_id][base_key] = W.to(torch.bfloat16)
        else:
            shards[shard_id][base_key] = ref_file.get_tensor(base_key).to(torch.bfloat16)
            
    # 3. Save shards and write index mapping
    idx_json = {"metadata": {"total_size": 0}, "weight_map": {}}
    
    for sh in range(1, 11):
        fn = f"model-0000{sh:02d}-of-00010.safetensors"
        shard_path = os.path.join(output_dir, fn)
        print(f"    - Saving Shard {sh}/10 to {shard_path}...")
        save_file(shards[sh], shard_path)
        
        for k in shards[sh].keys():
            idx_json["weight_map"][k] = fn
            
        del shards[sh]
        gc.collect()
        
    with open(os.path.join(output_dir, "model.safetensors.index.json"), "w") as f_out:
        json.dump(idx_json, f_out, indent=2)
        
    print("[SUCCESS] All shards reconstructed and index map generated.")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Direct reconstruct model shards")
    parser.add_argument("--svd-weights", required=True, help="Path to SVD safetensors weights")
    parser.add_argument("--output-dir", required=True, help="Output directory")
    args = parser.parse_args()
    reconstruct_model(args.svd_weights, args.output_dir, None)
