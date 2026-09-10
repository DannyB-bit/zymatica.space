# Gemma-4 Standalone Reconstructor
# WARNING: This decoder produces a LOSSY APPROXIMATION. The rank-3 seed captures only
# the top-3 dictionary pursuit projections per weight matrix. Embedding and layernorm
# parameters are initialized to defaults (zeros/ones), NOT reconstructed from the seed.
# Watermark: ip zymatica.space | astronautshe.com
import os, sys, struct, zlib, json, gc
import numpy as np, torch
from safetensors.torch import save_file

MS, DS, GM, PM = 42, 256, 0x47454E45, 0x50455246

def get_dict(dim, ds, seed):
    rng = np.random.RandomState(seed)
    m = rng.standard_normal((dim, ds)).astype(np.float32)
    return m / (np.linalg.norm(m, axis=0, keepdims=True) + 1e-9)

def get_si(name):
    if "embed_vision" in name:
        return 5, [5376, 1152], "zeros"
    elif "embed_tokens" in name:
        return 5, [262144, 5376], "embed"
    elif "language_model.norm" in name:
        return 5, [5376], "ones"
    elif "patch_embedder.input_proj" in name:
        return 5, [1152, 768], "zeros"
    elif "position_embedding_table" in name:
        return 5, [2, 10240, 1152], "embed"
    elif "std_bias" in name:
        return 5, [1152], "zeros"
    elif "std_scale" in name:
        return 5, [1152], "ones"
        
    if "language_model.layers." in name:
        b = int(name.split('.')[3])
        s_idx = min(5, b // 12 + 1)
        if any(x in name for x in ["layernorm", "layer_scalar"]):
            return s_idx, ([1] if "layer" in name else [5376]), "ones"
        elif "k_norm" in name or "q_norm" in name:
            return s_idx, ([512] if (b % 6 == 5) else [256]), "ones"
            
        is_sp = (b % 6 == 5)
        if "self_attn.q_proj" in name:
            return s_idx, ([16384, 5376] if is_sp else [8192, 5376]), "svd"
        elif "self_attn.k_proj" in name:
            return s_idx, ([2048, 5376] if is_sp else [4096, 5376]), "svd"
        elif "self_attn.v_proj" in name:
            return s_idx, [4096, 5376], "svd"
        elif "self_attn.o_proj" in name:
            return s_idx, ([5376, 16384] if is_sp else [5376, 8192]), "svd"
        elif "mlp.gate" in name or "mlp.up" in name:
            return s_idx, [21504, 5376], "svd"
        elif "mlp.down" in name:
            return s_idx, [5376, 21504], "svd"
            
    if "vision_tower.encoder.layers." in name:
        if any(x in name for x in ["layernorm"]):
            return 5, [1152], "ones"
        elif "k_norm" in name or "q_norm" in name:
            return 5, [72], "ones"
        elif "self_attn" in name:
            return 5, [1152, 1152], "svd"
        elif "mlp.gate" in name or "mlp.up" in name:
            return 5, [4304, 1152], "svd"
        elif "mlp.down" in name:
            return 5, [1152, 4304], "svd"
    return None, None, None

def gen_keys():
    keys = [
        "model.embed_vision.embedding_projection.weight", "model.language_model.embed_tokens.weight",
        "model.language_model.norm.weight", "model.vision_tower.patch_embedder.input_proj.weight",
        "model.vision_tower.patch_embedder.position_embedding_table", "model.vision_tower.std_bias", "model.vision_tower.std_scale"
    ]
    for i in range(60):
        pre = f"model.language_model.layers.{i}"
        keys.extend([
            f"{pre}.input_layernorm.weight", f"{pre}.post_attention_layernorm.weight",
            f"{pre}.pre_feedforward_layernorm.weight", f"{pre}.post_feedforward_layernorm.weight",
            f"{pre}.layer_scalar", f"{pre}.self_attn.k_norm.weight",
            f"{pre}.self_attn.q_norm.weight", f"{pre}.self_attn.q_proj.weight",
            f"{pre}.self_attn.k_proj.weight"
        ])
        if i % 6 != 5:
            keys.append(f"{pre}.self_attn.v_proj.weight")
        keys.extend([
            f"{pre}.self_attn.o_proj.weight", f"{pre}.mlp.gate_proj.weight",
            f"{pre}.mlp.up_proj.weight", f"{pre}.mlp.down_proj.weight"
        ])
    for i in range(27):
        pre = f"model.vision_tower.encoder.layers.{i}"
        keys.extend([
            f"{pre}.input_layernorm.weight", f"{pre}.post_attention_layernorm.weight",
            f"{pre}.pre_feedforward_layernorm.weight", f"{pre}.post_feedforward_layernorm.weight",
            f"{pre}.self_attn.k_norm.weight", f"{pre}.self_attn.q_norm.weight",
            f"{pre}.self_attn.q_proj.linear.weight", f"{pre}.self_attn.k_proj.linear.weight",
            f"{pre}.self_attn.v_proj.linear.weight", f"{pre}.self_attn.o_proj.linear.weight",
            f"{pre}.mlp.gate_proj.linear.weight", f"{pre}.mlp.up_proj.linear.weight",
            f"{pre}.mlp.down_proj.linear.weight"
        ])
    return keys

def reconstruct(seed_path, output_dir):
    with open(seed_path, "rb") as f_in:
        raw = zlib.decompress(f_in.read())
    pos = 0
    magic = struct.unpack_from('>I', raw, pos)[0]; pos += 4
    assert magic == GM
    version = struct.unpack_from('>H', raw, pos)[0]; pos += 2
    assert version == 12
    pos += 32 + 4
    hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack_from('>IIIIII', raw, pos); pos += 24
    pos += 16
    num_layers = struct.unpack_from('>I', raw, pos)[0]; pos += 4
    
    svd = {}
    for idx in range(num_layers):
        nl = struct.unpack_from('>H', raw, pos)[0]; pos += 2
        name = raw[pos : pos + nl].decode('utf-8'); pos += nl
        m, n, r = struct.unpack_from('>III', raw, pos); pos += 12
        svd[name] = {"idx": idx, "m": m, "n": n, "r": r, "pos": pos}
        pos += r * 4
        
    all_keys = gen_keys()
    idx_json = {"metadata": {"total_size": 0}, "weight_map": {}}
    os.makedirs(output_dir, exist_ok=True)
    
    for sh in range(1, 6):
        fn = f"model-0000{sh}-of-00005.safetensors"
        print(f"Reconstructing Shard {sh}/5...")
        tensors = {}
        for key in all_keys:
            target_sh, shape, init = get_si(key)
            if target_sh == sh:
                if init == "ones":
                    t = torch.ones(shape, dtype=torch.bfloat16)
                elif init == "embed":
                    # Embeddings are NOT stored in the seed — initialize to zeros
                    # (not random, since we cannot recover the original embedding values)
                    t = torch.zeros(shape, dtype=torch.bfloat16)
                else:
                    t = torch.zeros(shape, dtype=torch.bfloat16)
                    
                if key in svd:
                    meta = svd[key]
                    idx_val, m, n, r, p_pos = meta["idx"], meta["m"], meta["n"], meta["r"], meta["pos"]
                    U = get_dict(m, DS, MS + idx_val * 1000)
                    V = get_dict(n, DS, MS + idx_val * 1000 + 500)
                    iu, iv, cs = [], [], []
                    temp = p_pos
                    for _ in range(r):
                        iu.append(raw[temp])
                        iv.append(raw[temp+1])
                        c = struct.unpack_from('>e', raw, temp+2)[0]
                        cs.append(c)
                        temp += 4
                    t = torch.from_numpy((U[:, iu] * np.array(cs, dtype=np.float32)) @ V[:, iv].T).to(torch.bfloat16)
                tensors[key] = t
                idx_json["weight_map"][key] = fn
        save_file(tensors, os.path.join(output_dir, fn))
        del tensors
        gc.collect()
        
    with open(os.path.join(output_dir, "model.safetensors.index.json"), "w") as f:
        json.dump(idx_json, f, indent=2)
    print("RECONSTRUCTION COMPLETE (LOSSY APPROXIMATION)")
    print("WARNING: Reconstructed weights are a rank-3 approximation.")
    print("Embedding and layernorm weights are initialized to defaults.")
    print("This is NOT equivalent to the original Gemma-4 model.")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        sys.exit(1)
    reconstruct(sys.argv[1], sys.argv[2])
