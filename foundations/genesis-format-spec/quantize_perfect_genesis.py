import os
import struct
import numpy as np

GENESIS_MAGIC   = 0x47454E45   # "GENE"
PERFECT_MAGIC   = 0x50455246   # "PERF"
WATERMARK       = b"ip zymatica.space".ljust(32, b" ")

def quantize_to_int8(X):
    """Quantize a float32 matrix to int8. Returns (X_q, scale)."""
    max_val = float(np.max(np.abs(X)))
    scale = max_val / 127.0 if max_val > 0 else 1e-9
    X_q = np.clip(np.round(X / scale), -127, 127).astype(np.int8)
    return X_q, scale

def quantize_f32_to_int8_genesis(input_path, output_path):
    print("=" * 80)
    print("  GENESIS PERFECT ENGINE CONVERTER: FLOAT32 (v3) -> INT8 (v4)")
    print("  Watermark: ip zymatica.space")
    print("=" * 80)
    print(f"Reading from: {input_path}")
    print(f"Writing to:   {output_path}\n")

    if not os.path.exists(input_path):
        print(f"Error: Input file '{input_path}' does not exist.")
        return

    with open(input_path, "rb") as fin, open(output_path, "wb") as fout:
        # --- Read Header ---
        magic = struct.unpack('>I', fin.read(4))[0]
        if magic != GENESIS_MAGIC:
            print(f"Error: Invalid magic 0x{magic:08X}, expected 0x{GENESIS_MAGIC:08X}")
            return
            
        version = struct.unpack('>H', fin.read(2))[0]
        if version != 3:
            print(f"Warning: Input format version is {version}, expected version 3 (float32 SVD).")
            # We'll continue anyway if it's version 3 or 5, but version 3 is float32.

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

        # --- Write Header (Version 4) ---
        fout.write(struct.pack('>I', GENESIS_MAGIC))
        fout.write(struct.pack('>H', 4))  # version 4
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

            # Read float32 U and V
            U_bytes = fin.read(m * r * 4)
            V_bytes = fin.read(n * r * 4)
            
            U = np.frombuffer(U_bytes, dtype=np.float32).reshape(m, r)
            V = np.frombuffer(V_bytes, dtype=np.float32).reshape(n, r)

            # Quantize primary SVD factors to INT8
            U_q, scale_u = quantize_to_int8(U)
            V_q, scale_v = quantize_to_int8(V)

            # Read has_residual
            has_residual = struct.unpack('>?', fin.read(1))[0]

            # Write Layer in Version 4 format
            name_b = name.encode('utf-8')
            fout.write(struct.pack('>H', len(name_b)))
            fout.write(name_b)
            fout.write(struct.pack('>III', m, n, r))
            fout.write(struct.pack('>ff', scale_u, scale_v))
            fout.write(U_q.tobytes())
            fout.write(V_q.tobytes())
            fout.write(struct.pack('>?', has_residual))

            if has_residual:
                res_rank = struct.unpack('>I', fin.read(4))[0]
                scale_res_u, scale_res_v = struct.unpack('>ff', fin.read(8))
                U_res = fin.read(m * res_rank)  # int8 residual
                V_res = fin.read(n * res_rank)  # int8 residual

                # Residual is already int8, so write directly
                fout.write(struct.pack('>I', res_rank))
                fout.write(struct.pack('>ff', scale_res_u, scale_res_v))
                fout.write(U_res)
                fout.write(V_res)

            if (i + 1) % 20 == 0 or (i + 1) == num_layers:
                print(f"  Processed {i+1}/{num_layers} layers...")

    input_size = os.path.getsize(input_path) / 1e9
    output_size = os.path.getsize(output_path) / 1e9
    print(f"\nSuccess!")
    print(f"  Input size (float32):  {input_size:.2f} GB")
    print(f"  Output size (int8):    {output_size:.2f} GB")
    print(f"  Compression ratio:     {input_size / output_size:.2f}x")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Convert float32 genesis to int8 genesis")
    parser.add_argument("input", help="Path to input float32 .genesis file")
    parser.add_argument("output", help="Path to output int8 .genesis file")
    args = parser.parse_args()
    
    quantize_f32_to_int8_genesis(args.input, args.output)
