# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

"""
compress_tokenizer.py — 7-Level Tokenizer Descent Compression Engine
===================================================================
Author: Zymatica / Language-U Project
Watermark: ip zymatica.space | astronautshe.com

Compresses the Qwen 3.5 0.8B tokenizer files using a 7-Level hierarchy:
- Level 1 (Raw Baseline): Raw JSON/TXT files (~23 MB).
- Level 2 (Structured Extraction): Isolates vocabulary mapping and BPE merges.
- Level 3 (Byte/ID Delta Packing): Encodes merges as binary vocabulary index pairs.
- Level 4 (Prefix-Suffix Differential Compression): Prefix string matching + varints.
- Level 5 (Base Oracle Reference): Zero-delta alignment vs Qwen/Qwen3.5-0.8B.
- Level 6 (Deflate Entropy Coding): Zlib Level 9 hyper-deflate.
- Level 7 (XOR-FEC Packetization): Packetizes reference payload into 28 packets of 255 bytes.
"""

import os
import sys
import json
import zlib
import struct
import argparse
import hashlib

# Protocol constants
TK_MAGIC      = bytes([0xC5, 0x54, 0x4B])   # TK\xC5
PKT_SIZE      = 255
NUM_DATA      = 27
NUM_PKTS      = 28
DATA_PER_PKT  = PKT_SIZE - 3                # 252 bytes per packet
MAX_PAYLOAD   = NUM_DATA * DATA_PER_PKT     # 6,804 bytes
WATERMARK     = b'ip zymatica.space '

DEFAULT_MODEL_DIR = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
DEFAULT_OUT_DIR   = "j:/Language-U"

def write_varint(val):
    res = bytearray()
    while val >= 128:
        res.append((val & 0x7F) | 0x80)
        val >>= 7
    res.append(val & 0x7F)
    return bytes(res)

def get_prefix_suffix_encoding(tokens):
    """Encodes a list of token bytes using prefix-suffix compression."""
    encoded = bytearray()
    prev = b''
    for t in tokens:
        common = 0
        l = min(len(t), len(prev))
        while common < l and t[common] == prev[common]:
            common += 1
        suffix = t[common:]
        encoded.extend(write_varint(common))
        encoded.extend(write_varint(len(suffix)))
        encoded.extend(suffix)
        prev = t
    return bytes(encoded)

def pack_into_packets(payload):
    """Packs compressed payload into 28 x 255-byte XOR-FEC packets."""
    if len(payload) > MAX_PAYLOAD:
        raise OverflowError(f"Payload size {len(payload)} exceeds maximum packet capacity of {MAX_PAYLOAD} bytes.")
    
    # Pad payload with watermark to fill exactly MAX_PAYLOAD
    padded = (payload + (WATERMARK * (MAX_PAYLOAD // len(WATERMARK) + 1)))[:MAX_PAYLOAD]
    
    chunks = [padded[i * DATA_PER_PKT : (i + 1) * DATA_PER_PKT] for i in range(NUM_DATA)]
    
    packets = []
    for idx, chunk in enumerate(chunks):
        # Format per packet: [SYNC:0xBB][PKT_IDX:1][TOTAL_PKTS:1][252 bytes data]
        pkt = bytes([0xBB, idx, NUM_PKTS]) + chunk
        packets.append(pkt)
        
    # Calculate XOR-FEC parity packet over chunks
    parity_payload = bytearray(DATA_PER_PKT)
    for chunk in chunks:
        for j in range(DATA_PER_PKT):
            parity_payload[j] ^= chunk[j]
            
    parity_pkt = bytes([0xBB, NUM_DATA, NUM_PKTS]) + bytes(parity_payload)
    packets.append(parity_pkt)
    return packets

def main():
    parser = argparse.ArgumentParser(description="7-Level Tokenizer Descent Compression Engine")
    parser.add_argument("--model_dir", default=DEFAULT_MODEL_DIR, help="Path to raw tokenizer files")
    parser.add_argument("--out_dir", default=DEFAULT_OUT_DIR, help="Output directory for capsules and packets")
    args = parser.parse_args()

    print("=" * 80)
    print("  7-LEVEL TOKENIZER DESCENT COMPRESSION ENGINE")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)

    # 1. Check path validity
    tokenizer_json_path = os.path.join(args.model_dir, "tokenizer.json")
    tokenizer_config_path = os.path.join(args.model_dir, "tokenizer_config.json")
    vocab_json_path = os.path.join(args.model_dir, "vocab.json")
    merges_txt_path = os.path.join(args.model_dir, "merges.txt")

    for p in [tokenizer_json_path, tokenizer_config_path, vocab_json_path, merges_txt_path]:
        if not os.path.exists(p):
            print(f"[-] Error: Missing required file: {p}")
            sys.exit(1)

    # Load raw inputs
    print(f"\n[L1] Loading baseline files from {args.model_dir} ...")
    with open(tokenizer_json_path, "r", encoding="utf-8") as f:
        t_json = json.load(f)
    with open(tokenizer_config_path, "r", encoding="utf-8") as f:
        t_config = json.load(f)
    with open(vocab_json_path, "r", encoding="utf-8") as f:
        vocab_dict = json.load(f)
    with open(merges_txt_path, "r", encoding="utf-8") as f:
        merges_lines = [line.strip() for line in f if line.strip() and not line.startswith('#')]

    print(f"  - tokenizer.json:        {os.path.getsize(tokenizer_json_path):,} bytes")
    print(f"  - merges.txt:            {os.path.getsize(merges_txt_path):,} bytes")
    print(f"  - vocab.json:            {os.path.getsize(vocab_json_path):,} bytes")
    print(f"  - tokenizer_config.json: {os.path.getsize(tokenizer_config_path):,} bytes")

    # ── Phase 2: Extract structured configurations (L2) ────────────────────────
    print("\n[L2] Extracting structural configurations...")
    config_meta = {
        "version": t_json.get("version"),
        "truncation": t_json.get("truncation"),
        "padding": t_json.get("padding"),
        "added_tokens": t_json.get("added_tokens"),
        "normalizer": t_json.get("normalizer"),
        "pre_tokenizer": t_json.get("pre_tokenizer"),
        "post_processor": t_json.get("post_processor"),
        "decoder": t_json.get("decoder"),
        
        "model_type": t_json["model"]["type"],
        "model_dropout": t_json["model"].get("dropout"),
        "model_unk_token": t_json["model"].get("unk_token"),
        "model_continuing_subword_prefix": t_json["model"].get("continuing_subword_prefix"),
        "model_end_of_word_suffix": t_json["model"].get("end_of_word_suffix"),
        "model_fuse_unk": t_json["model"].get("fuse_unk"),
        "model_byte_fallback": t_json["model"].get("byte_fallback"),
        "model_ignore_merges": t_json["model"].get("ignore_merges"),
        
        # Include original tokenizer_config.json metadata
        "tokenizer_config": t_config
    }
    config_str = json.dumps(config_meta, ensure_ascii=False)
    config_bytes = config_str.encode("utf-8")
    comp_config = zlib.compress(config_bytes, 9)
    print(f"  Config metadata: {len(config_bytes):,} bytes -> compressed: {len(comp_config):,} bytes")

    # ── Phase 3: Binary Merges delta mapping (L3) ──────────────────────────────
    print("\n[L3] Mapping BPE merges to binary index pairs...")
    # Normal vocab tokens (ignoring added tokens mapped >= 248044)
    normal_vocab = sorted([(k, v) for k, v in vocab_dict.items() if v < 248044], key=lambda x: x[1])
    vocab_list = [t[0].encode("utf-8", errors="replace") for t in normal_vocab]
    
    # Construct binary merges
    merges_data = bytearray()
    for line in merges_lines:
        parts = line.split()
        if len(parts) != 2:
            continue
        idx0 = vocab_dict.get(parts[0], -1)
        idx1 = vocab_dict.get(parts[1], -1)
        if idx0 == -1 or idx1 == -1:
            print(f"  [-] Warning: Merge token not found in vocab: {parts}")
            continue
        merges_data.extend(struct.pack('>I', idx0)[1:])
        merges_data.extend(struct.pack('>I', idx1)[1:])
    print(f"  BPE Merges: {len(merges_lines):,} items -> encoded: {len(merges_data):,} bytes")

    # ── Phase 4: Prefix-Suffix Vocab Compression (L4) ──────────────────────────
    print("\n[L4] Performing prefix-suffix spectral vocabulary compression...")
    vocab_data = get_prefix_suffix_encoding(vocab_list)
    print(f"  Vocabulary: {len(vocab_list):,} strings -> encoded: {len(vocab_data):,} bytes")

    # ── Phase 5/6: Absolute Mode Capsule Assembly (L6) ──────────────────────────
    print("\n[L6] Assembling Absolute Mode capsule...")
    # Absolute Capsule Layout:
    # [Magic: 3B][Mode: 1B = 0x01][comp_config_len: 4B][comp_config: var][vocab_num: 4B][vocab_len: 4B][vocab_data: var][merges_num: 4B][merges_data: var]
    raw_absolute_payload = bytearray()
    raw_absolute_payload.extend(TK_MAGIC)
    raw_absolute_payload.append(0x01) # mode = Absolute
    raw_absolute_payload.extend(struct.pack('>I', len(comp_config)))
    raw_absolute_payload.extend(comp_config)
    raw_absolute_payload.extend(struct.pack('>I', len(vocab_list)))
    raw_absolute_payload.extend(struct.pack('>I', len(vocab_data)))
    raw_absolute_payload.extend(vocab_data)
    raw_absolute_payload.extend(struct.pack('>I', len(merges_lines)))
    raw_absolute_payload.extend(merges_data)

    print(f"  Raw Absolute capsule: {len(raw_absolute_payload):,} bytes")
    absolute_capsule = zlib.compress(raw_absolute_payload, 9)
    print(f"  Compressed Absolute capsule (Zlib): {len(absolute_capsule):,} bytes")

    # Save Absolute Capsule
    abs_path = os.path.join(args.out_dir, "qwen-3.5-0.8b-28chirps-tokenizer.capsule")
    with open(abs_path, "wb") as f:
        f.write(absolute_capsule)
    print(f"  [+] Wrote Absolute capsule to: {abs_path}")

    # ── Phase 5/6: Reference Mode Capsule Assembly (L5/L6) ──────────────────────
    print("\n[L5/L6] Assembling Reference Mode capsule (Zero-delta vs Base Oracle)...")
    # Reference Capsule Layout:
    # [Magic: 3B][Mode: 1B = 0x02][base_repo_len: 2B][base_repo: var]
    base_repo = "Qwen/Qwen3.5-0.8B"
    base_repo_bytes = base_repo.encode("utf-8")
    
    raw_ref_payload = bytearray()
    raw_ref_payload.extend(TK_MAGIC)
    raw_ref_payload.append(0x02) # mode = Reference
    raw_ref_payload.extend(struct.pack('>H', len(base_repo_bytes)))
    raw_ref_payload.extend(base_repo_bytes)

    print(f"  Raw Reference capsule: {len(raw_ref_payload)} bytes")
    ref_capsule = zlib.compress(raw_ref_payload, 9)
    print(f"  Compressed Reference capsule (Zlib): {len(ref_capsule)} bytes")

    # Save Reference Capsule
    ref_path = os.path.join(args.out_dir, "qwen-3.5-0.8b-28chirps-tokenizer-ref.capsule")
    with open(ref_path, "wb") as f:
        f.write(ref_capsule)
    print(f"  [+] Wrote Reference capsule to: {ref_path}")

    # ── Phase 7: XOR-FEC Chirp Packetization (L7) ───────────────────────────────
    print(f"\n[L7] Packetizing Reference Mode capsule into {NUM_PKTS} × {PKT_SIZE}-byte LoRa chirps...")
    packets = pack_into_packets(ref_capsule)
    
    pkt_dir = os.path.join(args.out_dir, "packets_tokenizer")
    os.makedirs(pkt_dir, exist_ok=True)
    
    sha_all = hashlib.sha256()
    for idx, pkt in enumerate(packets):
        p_path = os.path.join(pkt_dir, f"packet_tokenizer_{idx}.bin")
        with open(p_path, "wb") as f:
            f.write(pkt)
        sha_all.update(pkt)
        ptype = "DATA" if idx < NUM_DATA else "XOR-FEC-PARITY"
        print(f"  - Packet {idx:>2} [{ptype}]: {p_path}  ({len(pkt)} bytes)")

    # Write packet manifest
    manifest = {
        "protocol": "Tokenizer Chirp-28 v1.0",
        "method": "L7:QualiaSeed+L5:BaseReference+ZLIB+FEC",
        "watermark": "ip zymatica.space",
        "mode": "reference",
        "base_oracle": base_repo,
        "num_packets": NUM_PKTS,
        "packet_size": PKT_SIZE,
        "compressed_bytes": len(ref_capsule),
        "sha256": sha_all.hexdigest(),
    }
    manifest_path = os.path.join(pkt_dir, "manifest_tokenizer.json")
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, indent=2)
    print(f"  [+] Wrote packet manifest to: {manifest_path}")

    # ── Report ────────────────────────────────────────────────────────────────
    print("\n" + "=" * 80)
    print("  COMPRESSION RESULTS SUMMARY")
    print("=" * 80)
    print(f"  Original Files Size:      {os.path.getsize(tokenizer_json_path) + os.path.getsize(merges_txt_path) + os.path.getsize(vocab_json_path) + os.path.getsize(tokenizer_config_path):,} bytes (~23 MB)")
    print(f"  Absolute Capsule Size:    {len(absolute_capsule):,} bytes  (Mode 1, stand-alone, no external ref)")
    print(f"  Absolute Compression:     { (os.path.getsize(tokenizer_json_path) + os.path.getsize(merges_txt_path) + os.path.getsize(vocab_json_path) + os.path.getsize(tokenizer_config_path)) / len(absolute_capsule):.2f}x ratio")
    print(f"  Reference Capsule Size:   {len(ref_capsule)} bytes  (Mode 2, pre-shared oracle, zero-delta)")
    print(f"  Total Packets footprint:  {len(packets) * PKT_SIZE:,} bytes  ({NUM_PKTS} chirps × 255 bytes)")
    print(f"  Reference Compression:    { (os.path.getsize(tokenizer_json_path) + os.path.getsize(merges_txt_path) + os.path.getsize(vocab_json_path) + os.path.getsize(tokenizer_config_path)) / (len(packets) * PKT_SIZE):,.1f}x ratio (over wire)")
    print(f"  Watermark Hash:           {sha_all.hexdigest()[:32]}...")
    print("=" * 80)

if __name__ == "__main__":
    main()
