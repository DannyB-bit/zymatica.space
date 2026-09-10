import os
import struct
import numpy as np

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")

def pack_3bit_array(arr):
    """Pack a flat array of int8 values in range [-3, 3] into 3-bit packed bytes vectorized."""
    arr_shifted = np.clip(arr + 3, 0, 7).astype(np.uint8)
    n = len(arr_shifted)
    
    # Pad to multiple of 8
    pad_len = (8 - (n % 8)) % 8
    if pad_len > 0:
        arr_shifted = np.concatenate([arr_shifted, np.zeros(pad_len, dtype=np.uint8)])
    
    # Reshape to (N/8, 8)
    arr_reshaped = arr_shifted.reshape(-1, 8).astype(np.uint32)
    
    # shift powers: [2^0, 2^3, 2^6, 2^9, 2^12, 2^15, 2^18, 2^21]
    shifts = np.array([1, 8, 64, 512, 4096, 32768, 262144, 2097152], dtype=np.uint32)
    
    # Multiply and sum across axis 1
    vals = np.sum(arr_reshaped * shifts, axis=1)
    
    # Split each uint32 into 3 bytes
    b0 = (vals & 0xFF).astype(np.uint8)
    b1 = ((vals >> 8) & 0xFF).astype(np.uint8)
    b2 = ((vals >> 16) & 0xFF).astype(np.uint8)
    
    # Stack and convert to bytes
    packed_bytes = np.stack([b0, b1, b2], axis=1).flatten().tobytes()
    return packed_bytes

def unpack_3bit_array(packed_bytes, original_len):
    """Unpack 3-bit packed bytes back to int8 array in range [-3, 3] vectorized."""
    # Reshape byte array into (N/8, 3)
    bytes_arr = np.frombuffer(packed_bytes, dtype=np.uint8).reshape(-1, 3).astype(np.uint32)
    
    # Reconstruct 24-bit values
    vals = bytes_arr[:, 0] | (bytes_arr[:, 1] << 8) | (bytes_arr[:, 2] << 16)
    
    # Extract 8 components
    v0 = (vals & 0x07).astype(np.int8) - 3
    v1 = ((vals >> 3) & 0x07).astype(np.int8) - 3
    v2 = ((vals >> 6) & 0x07).astype(np.int8) - 3
    v3 = ((vals >> 9) & 0x07).astype(np.int8) - 3
    v4 = ((vals >> 12) & 0x07).astype(np.int8) - 3
    v5 = ((vals >> 15) & 0x07).astype(np.int8) - 3
    v6 = ((vals >> 18) & 0x07).astype(np.int8) - 3
    v7 = ((vals >> 21) & 0x07).astype(np.int8) - 3
    
    # Stack and flatten
    arr = np.stack([v0, v1, v2, v3, v4, v5, v6, v7], axis=1).flatten()
    return arr[:original_len]

def quantize_int8_to_3bit_genesis(input_path, output_path):
    print("=" * 80)
    print("  GENESIS LEVEL 3 CONVERTER: INT8 SVD (v4) -> ZIG 3-BIT SVD (v6)")
    print("  Watermark: ip zymatica.space")
    print("=" * 80)
    print(f"Reading from: {input_path}")
    print(f"Writing to:   {output_path}\n")

    if not os.path.exists(input_path):
        print(f"Error: Input file '{input_path}' does not exist.")
        return

    total_int8_weight_bytes = 0
    total_packed_3bit_bytes = 0
    errors = []

    with open(input_path, "rb") as fin, open(output_path, "wb") as fout:
        # --- Read Header ---
        magic = struct.unpack('>I', fin.read(4))[0]
        if magic != GENESIS_MAGIC:
            print(f"Error: Invalid magic 0x{magic:08X}, expected 0x{GENESIS_MAGIC:08X}")
            return
            
        version = struct.unpack('>H', fin.read(2))[0]
        if version != 4:
            print(f"Error: Input format version is {version}, expected version 4 (INT8 SVD).")
            return

        watermark = fin.read(32)
        perf_magic = struct.unpack('>I', fin.read(4))[0]
        if perf_magic != PERFECT_MAGIC:
            print(f"Error: Invalid perfect magic 0x{perf_magic:08X}, expected 0x{PERFECT_MAGIC:08X}")
            return

        # Model architecture
        hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack('>IIIIII', fin.read(24))
        print(f"Model Configuration:")
        print(f"  Hidden size: {hidden}, Heads: {heads}, KV Heads: {kv_heads}")
        print(f"  FFN Dim: {ffn_dim}, Blocks: {blocks}, Vocab: {vocab}")

        # Energy targets
        e_attn, e_ffn, e_lm, e_other = struct.unpack('>ffff', fin.read(16))
        print(f"Energy Targets: ATTN={e_attn:.2f}, FFN={e_ffn:.2f}, LM={e_lm:.2f}, OTHER={e_other:.2f}")

        # Layer count
        num_layers = struct.unpack('>I', fin.read(4))[0]
        print(f"Number of layers: {num_layers}\n")

        # --- Write Header (Version 6: 3-bit packed) ---
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', 6))  # format version 6 for 3-bit packed SVD
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

            # Read version 4 details: scales + int8 U_q and V_q
            scale_u, scale_v = struct.unpack('>ff', fin.read(8))
            U_q_bytes = fin.read(m * r)
            V_q_bytes = fin.read(n * r)
            
            U_q = np.frombuffer(U_q_bytes, dtype=np.int8).reshape(m, r)
            V_q = np.frombuffer(V_q_bytes, dtype=np.int8).reshape(n, r)

            # Convert from INT8 range [-127, 127] to 3-bit range [-3, 3]
            U_3bit = np.clip(np.round(U_q * (3.0 / 127.0)), -3, 3).astype(np.int8)
            V_3bit = np.clip(np.round(V_q * (3.0 / 127.0)), -3, 3).astype(np.int8)

            # Re-scale to preserve magnitude (compensates for 127 -> 3 scaling)
            new_scale_u = scale_u * (127.0 / 3.0)
            new_scale_v = scale_v * (127.0 / 3.0)

            # Pack 3-bit factors into binary bytes
            U_packed = pack_3bit_array(U_3bit.flatten())
            V_packed = pack_3bit_array(V_3bit.flatten())

            total_int8_weight_bytes += (m * r + n * r)
            total_packed_3bit_bytes += (len(U_packed) + len(V_packed))

            # Reconstruction sanity check (error of 3-bit vs original INT8)
            U_rec = (unpack_3bit_array(U_packed, m * r).reshape(m, r) * new_scale_u)
            V_rec = (unpack_3bit_array(V_packed, n * r).reshape(n, r) * new_scale_v)
            W_orig = (U_q.astype(np.float32) * scale_u) @ (V_q.astype(np.float32) * scale_v).T
            W_3bit = U_rec @ V_rec.T
            norm_diff = np.linalg.norm(W_orig - W_3bit)
            norm_orig = np.linalg.norm(W_orig) + 1e-9
            errors.append(norm_diff / norm_orig)

            # Read has_residual
            has_residual = struct.unpack('>?', fin.read(1))[0]

            # Write Layer in Version 6 format
            name_b = name.encode('utf-8')
            fout.write(struct.pack('>H', len(name_b)))
            fout.write(name_b)
            fout.write(struct.pack('>III', m, n, r))
            fout.write(struct.pack('>ff', new_scale_u, new_scale_v))
            
            # Write 3-bit packed arrays
            fout.write(U_packed)
            fout.write(V_packed)
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
    print(f"  Input size (INT8):     {input_size * 1000:.1f} MB")
    print(f"  Output size (3-bit):   {output_size * 1000:.1f} MB")
    print(f"  SVD parameters raw:    {total_int8_weight_bytes:,} bytes (INT8)")
    print(f"  SVD parameters packed: {total_packed_3bit_bytes:,} bytes (3-bit)")
    print(f"  Parameter reduction:   {total_int8_weight_bytes / total_packed_3bit_bytes:.2f}x")
    print(f"  Overall file ratio:    {input_size / output_size:.2f}x")
    print(f"  Mean SVD Quant Error:  {np.mean(errors):.4f}")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Convert INT8 genesis to 3-bit genesis")
    parser.add_argument("input", help="Path to input INT8 .genesis file")
    parser.add_argument("output", help="Path to output 3-bit .genesis file")
    args = parser.parse_args()
    
    quantize_int8_to_3bit_genesis(args.input, args.output)
