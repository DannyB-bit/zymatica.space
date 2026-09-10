import os
import struct
import numpy as np

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")

def pack_2bit_array(q_vals):
    """Pack an array of 4-bit values [-7, 7] to 2-bit values [sign, mag_class], packed 4 per byte."""
    n = len(q_vals)
    # Convert to 2-bit: sign (1 bit) + mag_class (1 bit)
    # sign: 1 if >= 0 else 0
    # mag_class: 1 if abs(v) > 3 else 0
    symbols = []
    for v in q_vals:
        sign = 1 if v >= 0 else 0
        mag = 1 if abs(v) > 3 else 0
        symbols.append((sign << 1) | mag)
        
    # Pad symbols to multiple of 4
    pad_len = (4 - (len(symbols) % 4)) % 4
    if pad_len > 0:
        symbols.extend([0] * pad_len)
        
    packed = bytearray()
    for i in range(0, len(symbols), 4):
        # Pack 4 symbols (each 2 bits) into 1 byte
        b = (
            (symbols[i] & 0x03) |
            ((symbols[i+1] & 0x03) << 2) |
            ((symbols[i+2] & 0x03) << 4) |
            ((symbols[i+3] & 0x03) << 6)
        )
        packed.append(b)
    return bytes(packed)

def unpack_4bit_layer_dct(packed_bytes, K):
    """Helper to unpack 4-bit packed values from DCT byte stream."""
    q_vals = []
    n_bytes = (K + 1) // 2
    packed = packed_bytes[:n_bytes]
    for b in packed:
        lo = b & 0x0F
        hi = (b >> 4) & 0x0F
        q_vals.append(lo if lo <= 7 else lo - 16)
        q_vals.append(hi if hi <= 7 else hi - 16)
    return q_vals[:K], packed_bytes[n_bytes:]

def quantize_dct_to_grad_genesis(input_path, output_path):
    print("=" * 80)
    print("  GENESIS LEVEL 6 CONVERTER: DCT SPECTRAL (v8) -> GRADIENT ATOM (v9)")
    print("  Watermark: ip zymatica.space")
    print("=" * 80)
    print(f"Reading from: {input_path}")
    print(f"Writing to:   {output_path}\n")

    if not os.path.exists(input_path):
        print(f"Error: Input file '{input_path}' does not exist.")
        return

    total_dct_bytes = 0
    total_grad_bytes = 0

    with open(input_path, "rb") as fin, open(output_path, "wb") as fout:
        # --- Read Header ---
        magic = struct.unpack('>I', fin.read(4))[0]
        if magic != GENESIS_MAGIC:
            print(f"Error: Invalid magic 0x{magic:08X}")
            return
            
        version = struct.unpack('>H', fin.read(2))[0]
        if version != 8:
            print(f"Error: Input format version is {version}, expected version 8 (DCT SVD).")
            return

        watermark = fin.read(32)
        perf_magic = struct.unpack('>I', fin.read(4))[0]
        if perf_magic != PERFECT_MAGIC:
            print(f"Error: Invalid perfect magic 0x{perf_magic:08X}")
            return

        hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack('>IIIIII', fin.read(24))
        e_attn, e_ffn, e_lm, e_other = struct.unpack('>ffff', fin.read(16))
        num_layers = struct.unpack('>I', fin.read(4))[0]

        # --- Write Header (Version 9: Gradient Atom DCT SVD) ---
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', 9))  # format version 9 for Gradient Atom
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
            scale_u, scale_v = struct.unpack('>ff', fin.read(8))
            
            u_len, v_len = struct.unpack('>II', fin.read(8))
            total_dct_bytes += u_len + v_len
            
            u_dct_data = fin.read(u_len)
            v_dct_data = fin.read(v_len)

            # Process U columns
            u_grad_blobs = []
            u_stream = u_dct_data
            for col in range(r):
                # Parse header
                orig_n = struct.unpack('>H', u_stream[:2])[0]
                K = u_stream[2]
                scale = struct.unpack('>e', u_stream[3:5])[0]
                idx_mode = u_stream[5]
                K_check = u_stream[6]
                
                # Seek index bytes
                idx_size = K_check * 2 if idx_mode == 1 else K_check
                idx_bytes = u_stream[7 : 7 + idx_size]
                
                # Extract 4-bit values and update stream
                packed_vals_offset = 7 + idx_size
                q_vals, remaining = unpack_4bit_layer_dct(u_stream[packed_vals_offset:], K)
                
                # Re-pack 4-bit values to 2-bit gradient atoms
                q_grad_packed = pack_2bit_array(q_vals)
                
                # Write new column format: [orig_n:2][K:1][scale:e:2][idx_mode:1][K_check:1][idx_bytes][packed_2bit_vals]
                col_header = struct.pack('>H', orig_n) + bytes([K]) + struct.pack('>e', scale) + bytes([idx_mode, K_check])
                u_grad_blobs.append(col_header + idx_bytes + q_grad_packed)
                u_stream = remaining

            # Process V columns
            v_grad_blobs = []
            v_stream = v_dct_data
            for col in range(r):
                # Parse header
                orig_n = struct.unpack('>H', v_stream[:2])[0]
                K = v_stream[2]
                scale = struct.unpack('>e', v_stream[3:5])[0]
                idx_mode = v_stream[5]
                K_check = v_stream[6]
                
                # Seek index bytes
                idx_size = K_check * 2 if idx_mode == 1 else K_check
                idx_bytes = v_stream[7 : 7 + idx_size]
                
                # Extract 4-bit values and update stream
                packed_vals_offset = 7 + idx_size
                q_vals, remaining = unpack_4bit_layer_dct(v_stream[packed_vals_offset:], K)
                
                # Re-pack 4-bit values to 2-bit gradient atoms
                q_grad_packed = pack_2bit_array(q_vals)
                
                col_header = struct.pack('>H', orig_n) + bytes([K]) + struct.pack('>e', scale) + bytes([idx_mode, K_check])
                v_grad_blobs.append(col_header + idx_bytes + q_grad_packed)
                v_stream = remaining

            u_grad_data = b''.join(u_grad_blobs)
            v_grad_data = b''.join(v_grad_blobs)
            total_grad_bytes += len(u_grad_data) + len(v_grad_data)

            # Read has_residual
            has_residual = struct.unpack('>?', fin.read(1))[0]

            # Write Layer in Version 9 format
            name_b = name.encode('utf-8')
            fout.write(struct.pack('>H', len(name_b)))
            fout.write(name_b)
            fout.write(struct.pack('>III', m, n, r))
            fout.write(struct.pack('>ff', scale_u, scale_v))
            
            # Write Gradient Atom blobs
            fout.write(struct.pack('>II', len(u_grad_data), len(v_grad_data)))
            fout.write(u_grad_data)
            fout.write(v_grad_data)
            fout.write(struct.pack('>?', has_residual))

            if has_residual:
                res_rank = struct.unpack('>I', fin.read(4))[0]
                scale_res_u, scale_res_v = struct.unpack('>ff', fin.read(8))
                U_res = fin.read(m * res_rank)  # int8 residual
                V_res = fin.read(n * res_rank)  # int8 residual

                fout.write(struct.pack('>I', res_rank))
                fout.write(struct.pack('>ff', scale_res_u, scale_res_v))
                fout.write(U_res)
                fout.write(V_res)

            if (i + 1) % 40 == 0 or (i + 1) == num_layers:
                print(f"  Processed {i+1}/{num_layers} layers...")

    input_size = os.path.getsize(input_path) / 1e9
    output_size = os.path.getsize(output_path) / 1e9
    
    print(f"\nSuccess!")
    print(f"  Input size (DCT v8):   {input_size * 1000:.1f} MB")
    print(f"  Output size (Grad v9): {output_size * 1000:.1f} MB")
    print(f"  DCT spectral bytes:    {total_dct_bytes:,} bytes")
    print(f"  Grad atom bytes:       {total_grad_bytes:,} bytes")
    print(f"  Overall SVD reduction: {total_dct_bytes / total_grad_bytes:.2f}x")
    print(f"  Overall file ratio:    {input_size / output_size:.2f}x")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Convert DCT genesis to Gradient Atom genesis")
    parser.add_argument("input", help="Path to input DCT .genesis file")
    parser.add_argument("output", help="Path to output Gradient Atom .genesis file")
    args = parser.parse_args()
    
    quantize_dct_to_grad_genesis(args.input, args.output)
