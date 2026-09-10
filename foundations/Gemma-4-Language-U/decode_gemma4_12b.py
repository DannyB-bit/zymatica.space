# Gemma-4-12B Weight Reconstructor & Decoder
# Watermark: ip zymatica.space | astronautshe.com | Gemma-4-Language-U

import os
import sys
import gc
import struct
import zlib
import json
import argparse
import time
import numpy as np
import torch
from scipy.fft import idct
from safetensors.torch import save_file
from safetensors import safe_open

MS, DS, GM, PM = 42, 256, 0x47454E45, 0x50455246

def get_dict(dim, ds, seed):
    """Generate pseudo-random dictionary mapping matrix."""
    rng = np.random.RandomState(seed)
    m = rng.standard_normal((dim, ds)).astype(np.float32)
    return m / (np.linalg.norm(m, axis=0, keepdims=True) + 1e-9)

def get_si(name, hidden, ffn_dim, vocab, layers_count):
    """Determine layer configuration parameters dynamically."""
    if "embed_vision" in name:
        return 5, [hidden, 1152], "zeros"
    elif "embed_tokens" in name:
        return 5, [vocab, hidden], "embed"
    elif "language_model.norm" in name:
        return 5, [hidden], "ones"
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
        s_idx = min(5, b // (layers_count // 5) + 1) if (layers_count >= 5) else 1
        if any(x in name for x in ["layernorm", "layer_scalar"]):
            return s_idx, ([1] if "layer" in name else [hidden]), "ones"
        elif "k_norm" in name or "q_norm" in name:
            # Detect standard vs split layers based on layer idx
            return s_idx, ([512] if (b % 6 == 5) else [256]), "ones"
            
        is_sp = (b % 6 == 5)
        if "self_attn.q_proj" in name:
            return s_idx, ([16384, hidden] if is_sp else [8192, hidden]), "svd"
        elif "self_attn.k_proj" in name:
            return s_idx, ([2048, hidden] if is_sp else [4096, hidden]), "svd"
        elif "self_attn.v_proj" in name:
            return s_idx, [4096, hidden], "svd"
        elif "self_attn.o_proj" in name:
            return s_idx, ([hidden, 16384] if is_sp else [hidden, 8192]), "svd"
        elif "mlp.gate" in name or "mlp.up" in name:
            return s_idx, [ffn_dim, hidden], "svd"
        elif "mlp.down" in name:
            return s_idx, [hidden, ffn_dim], "svd"
            
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

def get_si_10(name, hidden, ffn_dim, vocab, layers_count):
    """Sharding mapping helper for 10-shard division."""
    if "language_model.layers." not in name:
        target_sh = 10
    else:
        parts = name.split('.')
        try:
            b_idx = parts.index("layers") + 1
            b = int(parts[b_idx])
        except:
            b = 0
        target_sh = min(10, b // (layers_count // 10) + 1) if (layers_count >= 10) else 1
        
    _, shape, init = get_si(name, hidden, ffn_dim, vocab, layers_count)
    return target_sh, shape, init

def gen_keys(layers_count):
    """Generate dynamic lists of model weights based on configuration."""
    keys = [
        "model.embed_vision.embedding_projection.weight", "model.language_model.embed_tokens.weight",
        "model.language_model.norm.weight", "model.vision_tower.patch_embedder.input_proj.weight",
        "model.vision_tower.patch_embedder.position_embedding_table", "model.vision_tower.std_bias", "model.vision_tower.std_scale"
    ]
    for i in range(layers_count):
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
    return keys

# Vectorized array unpacking subroutines
def unpack_3bit_array(packed_bytes, original_len):
    bytes_arr = np.frombuffer(packed_bytes, dtype=np.uint8).reshape(-1, 3).astype(np.uint32)
    vals = bytes_arr[:, 0] | (bytes_arr[:, 1] << 8) | (bytes_arr[:, 2] << 16)
    v0 = (vals & 0x07).astype(np.int8) - 3
    v1 = ((vals >> 3) & 0x07).astype(np.int8) - 3
    v2 = ((vals >> 6) & 0x07).astype(np.int8) - 3
    v3 = ((vals >> 9) & 0x07).astype(np.int8) - 3
    v4 = ((vals >> 12) & 0x07).astype(np.int8) - 3
    v5 = ((vals >> 15) & 0x07).astype(np.int8) - 3
    v6 = ((vals >> 18) & 0x07).astype(np.int8) - 3
    v7 = ((vals >> 21) & 0x07).astype(np.int8) - 3
    arr = np.stack([v0, v1, v2, v3, v4, v5, v6, v7], axis=1).flatten()
    return arr[:original_len]

def dct_decompress_vec(data, pos):
    n     = struct.unpack_from('>H', data, pos)[0]; pos += 2
    K     = data[pos]; pos += 1
    scale = struct.unpack_from('>e', data, pos)[0]; pos += 2
    idx_mode = data[pos]; K_check = data[pos+1]; pos += 2

    indices = []
    running = 0
    for _ in range(K_check):
        if idx_mode == 0:
            delta = data[pos]; pos += 1
        else:
            delta = struct.unpack_from('>H', data, pos)[0]; pos += 2
        running += delta
        indices.append(running)

    n_bytes = (K_check + 1) // 2
    packed  = data[pos : pos + n_bytes]; pos += n_bytes
    q_vals  = []
    for b in packed:
        lo = b & 0x0F; hi = (b >> 4) & 0x0F
        q_vals.append(lo if lo <= 7 else lo - 16)
        q_vals.append(hi if hi <= 7 else hi - 16)
    q_vals = q_vals[:K_check]

    v_dct = np.zeros(n, dtype=np.float64)
    v_dct[indices] = np.array(q_vals, dtype=np.float64) * scale
    return idct(v_dct, norm='ortho').astype(np.float32), pos

def decompress_and_reconstruct(input_path, output_dir, ref_weights_path=None):
    print("=" * 80)
    print(f"[*] Starting Reconstruction for Gemma-4-Language-U Pipeline...")
    print(f"    Source File:   {input_path}")
    print(f"    Output Folder: {output_dir}")
    if ref_weights_path:
        print(f"    Ref Weights:   {ref_weights_path}")
    print("=" * 80)
    
    ref_keys = set()
    ref_file = None
    if ref_weights_path and os.path.exists(ref_weights_path):
        print("[*] Opening reference weights file for on-demand loading...")
        ref_file = safe_open(ref_weights_path, framework="pt", device="cpu")
        ref_keys = set(ref_file.keys())
        print(f"    - Found {len(ref_keys)} tensors in reference file.")
    
    with open(input_path, "rb") as fh:
        raw_bytes = fh.read()
        
    try:
        raw = zlib.decompress(raw_bytes)
        print("[+] Detected zlib deflated capsule (Level 9 Seed).")
    except Exception:
        raw = raw_bytes
        print("[*] Detected raw binary genesis format.")
        
    pos = 0
    magic = struct.unpack_from('>I', raw, pos)[0]; pos += 4
    assert magic == GM, f"Invalid magic header: {hex(magic)}"
    version = struct.unpack_from('>H', raw, pos)[0]; pos += 2
    watermark = raw[pos : pos + 32].decode('utf-8', errors='ignore').strip(); pos += 32
    perf_magic = struct.unpack_from('>I', raw, pos)[0]; pos += 4
    assert perf_magic == PM, f"Invalid perfect magic: {hex(perf_magic)}"
    
    hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack_from('>IIIIII', raw, pos); pos += 24
    pos += 16  # skip energy/scale
    num_layers = struct.unpack_from('>I', raw, pos)[0]; pos += 4
    
    print(f"[+] Version: {version} (Watermark: '{watermark}')")
    print(f"[+] Arch:    hidden={hidden}, heads={heads}, kv_heads={kv_heads}, ffn_dim={ffn_dim}, blocks={blocks}, vocab={vocab}")
    print(f"[+] Layers:  {num_layers} parameters inside seed")
    
    # 4. Index weights offsets
    layer_offsets = {}
    for idx in range(num_layers):
        nl = struct.unpack_from('>H', raw, pos)[0]; pos += 2
        name = raw[pos : pos + nl].decode('utf-8'); pos += nl
        m, n, r = struct.unpack_from('>III', raw, pos); pos += 12
        start_offset = pos
        
        if version == 4:
            pos += 8 # scale_u, scale_v
            pos += m * r + n * r
            has_residual = struct.unpack_from('>?', raw, pos)[0]; pos += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw, pos)[0]; pos += 4
                pos += 8 # su_r, sv_r
                pos += m * res_rank + n * res_rank
        elif version == 6:
            pos += 8 # scale_u, scale_v
            u_len_padded = (m * r + (8 - (m * r % 8)) % 8)
            v_len_padded = (n * r + (8 - (n * r % 8)) % 8)
            pos += (u_len_padded * 3) // 8 + (v_len_padded * 3) // 8
            has_residual = struct.unpack_from('>?', raw, pos)[0]; pos += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw, pos)[0]; pos += 4
                pos += 8 # su_r, sv_r
                pos += m * res_rank + n * res_rank
        elif version == 8:
            pos += 8 # scale_u, scale_v
            u_dct_len, v_dct_len = struct.unpack_from('>II', raw, pos); pos += 8
            pos += u_dct_len + v_dct_len
            has_residual = struct.unpack_from('>?', raw, pos)[0]; pos += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw, pos)[0]; pos += 4
                pos += 8 # su_r, sv_r
                pos += m * res_rank + n * res_rank
        elif version == 12:
            pos += r * 4
        else:
            raise ValueError(f"Unsupported version: {version}")
            
        layer_offsets[name] = (idx, m, n, r, start_offset, pos)
        
    print(f"[+] Indexed {len(layer_offsets)} layers successfully.")

    def decompress_layer(raw_bytes, ver, l_name, l_idx, lm, ln, lr, start_pos, end_pos):
        off = start_pos
        if ver == 4:
            scale_u, scale_v = struct.unpack_from('>ff', raw_bytes, off); off += 8
            u_bytes = raw_bytes[off : off + lm * lr]; off += lm * lr
            v_bytes = raw_bytes[off : off + ln * lr]; off += ln * lr
            U = np.frombuffer(u_bytes, dtype=np.int8).reshape(lm, lr).astype(np.float32) * scale_u
            V = np.frombuffer(v_bytes, dtype=np.int8).reshape(ln, lr).astype(np.float32) * scale_v
            W_rec = U @ V.T
            
            has_residual = struct.unpack_from('>?', raw_bytes, off)[0]; off += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw_bytes, off)[0]; off += 4
                su_r, sv_r = struct.unpack_from('>ff', raw_bytes, off); off += 8
                U_res = np.frombuffer(raw_bytes[off : off + lm * res_rank], dtype=np.int8).reshape(lm, res_rank).astype(np.float32) * su_r; off += lm * res_rank
                V_res = np.frombuffer(raw_bytes[off : off + ln * res_rank], dtype=np.int8).reshape(ln, res_rank).astype(np.float32) * sv_r; off += ln * res_rank
                W_rec += U_res @ V_res.T
                
        elif ver == 6:
            scale_u, scale_v = struct.unpack_from('>ff', raw_bytes, off); off += 8
            u_len_padded = (lm * lr + (8 - (lm * lr % 8)) % 8)
            v_len_padded = (ln * lr + (8 - (ln * lr % 8)) % 8)
            u_bytes = raw_bytes[off : off + (u_len_padded * 3) // 8]; off += (u_len_padded * 3) // 8
            v_bytes = raw_bytes[off : off + (v_len_padded * 3) // 8]; off += (v_len_padded * 3) // 8
            U = unpack_3bit_array(u_bytes, lm * lr).reshape(lm, lr).astype(np.float32) * scale_u
            V = unpack_3bit_array(v_bytes, ln * lr).reshape(ln, lr).astype(np.float32) * scale_v
            W_rec = U @ V.T
            
            has_residual = struct.unpack_from('>?', raw_bytes, off)[0]; off += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw_bytes, off)[0]; off += 4
                su_r, sv_r = struct.unpack_from('>ff', raw_bytes, off); off += 8
                U_res = np.frombuffer(raw_bytes[off : off + lm * res_rank], dtype=np.int8).reshape(lm, res_rank).astype(np.float32) * su_r; off += lm * res_rank
                V_res = np.frombuffer(raw_bytes[off : off + ln * res_rank], dtype=np.int8).reshape(ln, res_rank).astype(np.float32) * sv_r; off += ln * res_rank
                W_rec += U_res @ V_res.T
                
        elif ver == 8:
            scale_u, scale_v = struct.unpack_from('>ff', raw_bytes, off); off += 8
            u_dct_len, v_dct_len = struct.unpack_from('>II', raw_bytes, off); off += 8
            u_dct_data = raw_bytes[off : off + u_dct_len]; off += u_dct_len
            v_dct_data = raw_bytes[off : off + v_dct_len]; off += v_dct_len
            
            u_cols = []
            u_pos = 0
            for _ in range(lr):
                col, consumed = dct_decompress_vec(u_dct_data, u_pos)
                u_pos = consumed
                u_cols.append(col)
            U = np.stack(u_cols, axis=1)
            
            v_cols = []
            v_pos = 0
            for _ in range(lr):
                col, consumed = dct_decompress_vec(v_dct_data, v_pos)
                v_pos = consumed
                v_cols.append(col)
            V = np.stack(v_cols, axis=1)
            W_rec = U @ V.T
            
            has_residual = struct.unpack_from('>?', raw_bytes, off)[0]; off += 1
            if has_residual:
                res_rank = struct.unpack_from('>I', raw_bytes, off)[0]; off += 4
                su_r, sv_r = struct.unpack_from('>ff', raw_bytes, off); off += 8
                U_res = np.frombuffer(raw_bytes[off : off + lm * res_rank], dtype=np.int8).reshape(lm, res_rank).astype(np.float32) * su_r; off += lm * res_rank
                V_res = np.frombuffer(raw_bytes[off : off + ln * res_rank], dtype=np.int8).reshape(ln, res_rank).astype(np.float32) * sv_r; off += ln * res_rank
                W_rec += U_res @ V_res.T
                
        elif ver == 12:
            U_dict = get_dict(lm, DS, MS + l_idx * 1000)
            V_dict = get_dict(ln, DS, MS + l_idx * 1000 + 500)
            iu, iv, cs = [], [], []
            for _ in range(lr):
                iu.append(raw_bytes[off]); off += 1
                iv.append(raw_bytes[off]); off += 1
                c = struct.unpack_from('>e', raw_bytes, off)[0]; off += 2
                cs.append(c)
            W_rec = (U_dict[:, iu] * np.array(cs, dtype=np.float32)) @ V_dict[:, iv].T
        else:
            raise ValueError(f"Unsupported genesis version: {ver}")
            
        return torch.from_numpy(W_rec).to(torch.bfloat16)

    # 5. Output sharding execution
    all_keys = gen_keys(blocks)
    idx_json = {"metadata": {"total_size": 0}, "weight_map": {}}
    os.makedirs(output_dir, exist_ok=True)
    
    for sh in range(1, 11):
        fn = f"model-0000{sh:02d}-of-00010.safetensors"
        print(f"    - Reconstructing Shard {sh}/10...")
        tensors = {}
        for key in all_keys:
            target_sh, shape, init = get_si_10(key, hidden, ffn_dim, vocab, blocks)
            if target_sh == sh:
                matched = False
                for dk, (l_idx, lm, ln, lr, s_off, e_off) in layer_offsets.items():
                    if key.endswith(dk) or dk.endswith(key.replace("model.", "")):
                        tensors[key] = decompress_layer(
                            raw, version, dk, l_idx, lm, ln, lr, s_off, e_off
                        )
                        matched = True
                        break
                        
                if not matched:
                    if ref_file and key in ref_keys:
                        tensors[key] = ref_file.get_tensor(key).clone()
                    else:
                        if init == "ones":
                            tensors[key] = torch.ones(shape, dtype=torch.bfloat16)
                        else:
                            tensors[key] = torch.zeros(shape, dtype=torch.bfloat16)
                            
                if not tensors[key].is_contiguous():
                    tensors[key] = tensors[key].contiguous()
                idx_json["weight_map"][key] = fn
                
        save_file(tensors, os.path.join(output_dir, fn))
        del tensors
        gc.collect()
        
    if ref_file is not None:
        del ref_file
    gc.collect()
    
    with open(os.path.join(output_dir, "model.safetensors.index.json"), "w") as f_out:
        json.dump(idx_json, f_out, indent=2)
        
    print(f"\n[+] Reconstruction complete. Shards saved to: {output_dir}")

def main():
    parser = argparse.ArgumentParser(description="Gemma-4-12B Reconstruction Engine")
    parser.add_argument("--input", type=str, required=True, help="Path to input .genesis or .LLM capsule file")
    parser.add_argument("--output-dir", type=str, required=True, help="Folder to write output safetensors shards to")
    parser.add_argument("--ref-weights", type=str, default=None, help="Optional path to reference base weights safetensors")
    args = parser.parse_args()
    
    decompress_and_reconstruct(args.input, args.output_dir, args.ref_weights)

if __name__ == "__main__":
    main()
