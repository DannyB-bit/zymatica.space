import os
import struct
import numpy as np
from scipy.fft import dct, idct

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")

def unpack_3bit_array(packed_bytes, original_len):
    """Unpack 3-bit packed bytes back to int8 array in range [-3, 3] vectorized."""
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

def dct_compress_vec(v, max_K=16):
    """L4: Apply DCT and keep top-K coefficients, quantized to 4-bit."""
    n = len(v)
    K = min(max_K, n)
    v_dct = dct(v.astype(np.float64), norm='ortho')
    top_idx = np.sort(np.argsort(np.abs(v_dct))[-K:])
    vals    = v_dct[top_idx]
    scale   = float(np.abs(vals).max()) / 7.0 + 1e-9
    q_vals  = np.round(vals / scale).clip(-7, 7).astype(np.int8)

    deltas = np.diff(np.concatenate([[0], top_idx])).astype(np.uint8)
    if deltas.max() > 255:
        idx_bytes = bytes([0x01, K]) + b''.join(struct.pack('>H', int(d)) for d in
                          np.diff(np.concatenate([[0], top_idx])).astype(np.uint16))
    else:
        idx_bytes = bytes([0x00, K]) + bytes(deltas)

    # Pack 4-bit values: 2 values per byte
    packed_vals = bytearray()
    for i in range(0, K, 2):
        lo = int(q_vals[i]) & 0x0F
        hi = (int(q_vals[i+1]) & 0x0F) if i+1 < K else 0
        packed_vals.append((hi << 4) | lo)

    header = struct.pack('>H', n) + bytes([K]) + struct.pack('>e', scale)
    return header + idx_bytes + bytes(packed_vals)

def quantize_3bit_to_dct_genesis(input_path, output_path, K_u=16, K_v=16):
    print("=" * 80)
    print("  GENESIS LEVEL 4 CONVERTER: 3-BIT SVD (v6) -> DCT SPECTRAL SVD (v8)")
    print("  Watermark: ip zymatica.space")
    print("=" * 80)
    print(f"Reading from: {input_path}")
    print(f"Writing to:   {output_path}\n")

    if not os.path.exists(input_path):
        print(f"Error: Input file '{input_path}' does not exist.")
        return

    total_3bit_bytes = 0
    total_dct_bytes = 0

    with open(input_path, "rb") as fin, open(output_path, "wb") as fout:
        # --- Read Header ---
        magic = struct.unpack('>I', fin.read(4))[0]
        if magic != GENESIS_MAGIC:
            print(f"Error: Invalid magic 0x{magic:08X}")
            return
            
        version = struct.unpack('>H', fin.read(2))[0]
        if version != 6:
            print(f"Error: Input format version is {version}, expected version 6 (3-bit SVD).")
            return

        watermark = fin.read(32)
        perf_magic = struct.unpack('>I', fin.read(4))[0]
        if perf_magic != PERFECT_MAGIC:
            print(f"Error: Invalid perfect magic 0x{perf_magic:08X}")
            return

        hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack('>IIIIII', fin.read(24))
        e_attn, e_ffn, e_lm, e_other = struct.unpack('>ffff', fin.read(16))
        num_layers = struct.unpack('>I', fin.read(4))[0]

        # --- Write Header (Version 8: DCT spectral SVD) ---
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', 8))  # format version 8 for DCT spectral
        fout.write(WATERMARK)
        fout.write(struct.pack('>I', PERFECT_MAGIC))
        fout.write(struct.pack('>IIIIII', hidden, heads, kv_heads, ffn_dim, blocks, vocab))
        fout.write(struct.pack('>ffff', e_attn, e_ffn, e_lm, e_other))
        
        # Placeholder for layer count
        layer_count_pos = fout.tell()
        fout.write(struct.pack('>I', num_layers))

        # --- Process Layers ---
        for i in range(num_layers):
            name_len = struct.unpack('>H', fin.read(2))[0]
            name = fin.read(name_len).decode('utf-8')
            m, n, r = struct.unpack('>III', fin.read(12))

            # Read version 6 details: scales + 3-bit packed U and V
            scale_u, scale_v = struct.unpack('>ff', fin.read(8))
            
            # 3-bit packed array sizes: ceil(len * 3 / 8) -> padded to multiple of 8
            u_len_padded = (m * r + (8 - (m * r % 8)) % 8)
            v_len_padded = (n * r + (8 - (n * r % 8)) % 8)
            u_packed_bytes = fin.read((u_len_padded * 3) // 8)
            v_packed_bytes = fin.read((v_len_padded * 3) // 8)
            
            total_3bit_bytes += len(u_packed_bytes) + len(v_packed_bytes)

            # Unpack 3-bit arrays back to [-3, 3] integers
            U_3bit = unpack_3bit_array(u_packed_bytes, m * r).reshape(m, r)
            V_3bit = unpack_3bit_array(v_packed_bytes, n * r).reshape(n, r)

            # De-quantize back to floating-point vectors
            U_float = U_3bit.astype(np.float32) * scale_u
            V_float = V_3bit.astype(np.float32) * scale_v

            # Compress each column vector of U and V via DCT Spectral (Level 4)
            u_dct_blobs = []
            v_dct_blobs = []
            for col in range(r):
                u_dct_blobs.append(dct_compress_vec(U_float[:, col], max_K=K_u))
                v_dct_blobs.append(dct_compress_vec(V_float[:, col], max_K=K_v))

            u_dct_data = b''.join(u_dct_blobs)
            v_dct_data = b''.join(v_dct_blobs)
            
            total_dct_bytes += len(u_dct_data) + len(v_dct_data)

            # Read has_residual
            has_residual = struct.unpack('>?', fin.read(1))[0]

            # Write Layer in Version 8 format
            name_b = name.encode('utf-8')
            fout.write(struct.pack('>H', len(name_b)))
            fout.write(name_b)
            fout.write(struct.pack('>III', m, n, r))
            fout.write(struct.pack('>ff', scale_u, scale_v)) # store reference scales
            
            # Write DCT binary blobs
            fout.write(struct.pack('>II', len(u_dct_data), len(v_dct_data)))
            fout.write(u_dct_data)
            fout.write(v_dct_data)
            
            fout.write(struct.pack('>?', has_residual))

            if has_residual:
                res_rank = struct.unpack('>I', fin.read(4))[0]
                scale_res_u, scale_res_v = struct.unpack('>ff', fin.read(8))
                U_res = fin.read(m * res_rank)  # int8 residual
                V_res = fin.read(n * res_rank)  # int8 residual

                # Residual is left as int8, write directly
                fout.write(struct.pack('>I', res_rank))
                fout.write(struct.pack('>ff', scale_res_u, scale_res_v))
                fout.write(U_res)
                fout.write(V_res)

            if (i + 1) % 40 == 0 or (i + 1) == num_layers:
                print(f"  Processed {i+1}/{num_layers} layers...")

    input_size = os.path.getsize(input_path) / 1e9
    output_size = os.path.getsize(output_path) / 1e9
    
    print(f"\nSuccess!")
    print(f"  Input size (3-bit):    {input_size * 1000:.1f} MB")
    print(f"  Output size (DCT v8):  {output_size * 1000:.1f} MB")
    print(f"  3-bit parameter bytes: {total_3bit_bytes:,} bytes")
    print(f"  DCT spectral bytes:    {total_dct_bytes:,} bytes")
    print(f"  Overall SVD reduction: {total_3bit_bytes / total_dct_bytes:.2f}x")
    print(f"  Overall file ratio:    {input_size / output_size:.2f}x")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Convert 3-bit genesis to DCT spectral genesis")
    parser.add_argument("input", help="Path to input 3-bit .genesis file")
    parser.add_argument("output", help="Path to output DCT .genesis file")
    parser.add_argument("--k-u", type=int, default=16, help="Top-K DCT coefficients for U")
    parser.add_argument("--k-v", type=int, default=16, help="Top-K DCT coefficients for V")
    args = parser.parse_args()
    
    quantize_3bit_to_dct_genesis(args.input, args.output, K_u=args.k_u, K_v=args.k_v)
