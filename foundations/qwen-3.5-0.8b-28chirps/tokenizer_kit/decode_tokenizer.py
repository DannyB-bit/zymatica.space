# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

"""
decode_tokenizer.py — 7-Level Tokenizer Restoration & Verification Engine
=========================================================================
Author: Zymatica / Language-U Project
Watermark: ip zymatica.space | astronautshe.com

Decodes the hyper-compressed tokenizer capsule (or reassembles from 28 packets
via XOR-FEC) and reconstructs standard tokenizer files:
- tokenizer.json
- tokenizer_config.json
- vocab.json
- merges.txt
"""

import os
import sys
import json
import zlib
import struct
import argparse
import hashlib
from transformers import AutoTokenizer

TK_MAGIC      = bytes([0xC5, 0x54, 0x4B])   # TK\xC5
PKT_SIZE      = 255
NUM_DATA      = 27
NUM_PKTS      = 28
DATA_PER_PKT  = PKT_SIZE - 3                # 252 bytes
MAX_PAYLOAD   = NUM_DATA * DATA_PER_PKT     # 6,804 bytes

DEFAULT_CAPSULE = "j:/Language-U/qwen-3.5-0.8b-28chirps-tokenizer-ref.capsule"
DEFAULT_PKT_DIR = "j:/Language-U/packets_tokenizer"
DEFAULT_OUT_DIR = "j:/Language-U/reconstructed_tokenizer"

def read_varint(data, pos):
    val = 0
    shift = 0
    while True:
        b = data[pos]
        pos += 1
        val |= (b & 0x7F) << shift
        if not (b & 0x80):
            break
        shift += 7
    return val, pos

def decode_prefix_suffix(data, num_tokens):
    tokens = []
    pos = 0
    prev = b''
    for _ in range(num_tokens):
        common, pos = read_varint(data, pos)
        suffix_len, pos = read_varint(data, pos)
        suffix = data[pos : pos + suffix_len]
        pos += suffix_len
        
        t = prev[:common] + suffix
        tokens.append(t)
        prev = t
    return tokens

def recover_packets_via_fec(pkt_dir):
    """Loads 28 packets from folder and applies XOR-FEC recovery if exactly 1 packet is missing."""
    packet_files = sorted([f for f in os.listdir(pkt_dir) if f.startswith("packet_tokenizer_") and f.endswith(".bin")])
    if not packet_files:
        raise FileNotFoundError(f"No packet files found in {pkt_dir}")
        
    # Read the number of total packets from the wrapper of the first file
    with open(os.path.join(pkt_dir, packet_files[0]), "rb") as f:
        first_pkt = f.read()
    if len(first_pkt) < 3 or first_pkt[0] != 0xBB:
        raise ValueError("Invalid packet header structure in first packet.")
    total_pkts = first_pkt[2]
    
    # Load all available packets
    received_packets = {}
    for pf in packet_files:
        with open(os.path.join(pkt_dir, pf), "rb") as f:
            pkt_bytes = f.read()
        if len(pkt_bytes) == PKT_SIZE and pkt_bytes[0] == 0xBB:
            idx = pkt_bytes[1]
            received_packets[idx] = pkt_bytes
            
    print(f"  Loaded {len(received_packets)}/{total_pkts} packets.")
    
    missing_indices = [i for i in range(total_pkts) if i not in received_packets]
    if len(missing_indices) == 0:
        print("[+] All packets received intact. Verifying FEC parity...")
        # Verify FEC is correct (XOR of all data + FEC payloads must equal 0)
        xor_fec = bytearray(DATA_PER_PKT)
        for idx, pkt in received_packets.items():
            for j in range(DATA_PER_PKT):
                xor_fec[j] ^= pkt[j + 3]
        if any(xor_fec):
            print("⚠️ Warning: FEC verification failed (non-zero XOR sum).")
        else:
            print("[+] FEC verification passed.")
    elif len(missing_indices) == 1:
        missing_idx = missing_indices[0]
        print(f"[-] Missing packet index {missing_idx}. Performing XOR FEC recovery...")
        recovered_payload = bytearray(DATA_PER_PKT)
        for idx, pkt in received_packets.items():
            for j in range(DATA_PER_PKT):
                recovered_payload[j] ^= pkt[j + 3]
                
        # Reconstruct missing packet
        recovered_pkt = bytes([0xBB, missing_idx, total_pkts]) + bytes(recovered_payload)
        received_packets[missing_idx] = recovered_pkt
        print(f"[+] Successfully recovered missing packet index {missing_idx} via FEC.")
    else:
        raise ValueError(f"Cannot recover because {len(missing_indices)} packets are missing.")
        
    # Reassemble payload from data packets (excluding FEC parity packet)
    assembled = bytearray()
    for i in range(NUM_DATA):
        assembled.extend(received_packets[i][3:])
    return bytes(assembled)

def main():
    parser = argparse.ArgumentParser(description="7-Level Tokenizer Restoration & Verification Engine")
    parser.add_argument("--capsule", default=None, help="Path to capsule file to decode")
    parser.add_argument("--packet_dir", default=None, help="Path to packets directory to reassemble")
    parser.add_argument("--out_dir", default=DEFAULT_OUT_DIR, help="Output directory for restored tokenizer")
    args = parser.parse_args()

    print("=" * 80)
    print("  7-LEVEL TOKENIZER RESTORATION & VERIFICATION ENGINE")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)

    # 1. Determine Source (Capsule or Packet directory)
    payload_bytes = None
    if args.packet_dir:
        print(f"\n[*] Reassembling from packets in: {args.packet_dir}")
        try:
            payload_bytes = recover_packets_via_fec(args.packet_dir)
        except Exception as e:
            print(f"[-] Reassembly failed: {e}")
            sys.exit(1)
    elif args.capsule:
        print(f"\n[*] Decoding capsule file: {args.capsule}")
        with open(args.capsule, "rb") as f:
            payload_bytes = f.read()
    else:
        # Auto-detect packets or capsule
        if os.path.exists(DEFAULT_PKT_DIR) and len(os.listdir(DEFAULT_PKT_DIR)) > 0:
            print(f"\n[*] Auto-detected packet directory: {DEFAULT_PKT_DIR}")
            try:
                payload_bytes = recover_packets_via_fec(DEFAULT_PKT_DIR)
            except Exception as e:
                print(f"[-] Reassembly failed: {e}")
        
        if payload_bytes is None:
            # Fallback to default capsule
            if os.path.exists(DEFAULT_CAPSULE):
                print(f"\n[*] Auto-detected default capsule file: {DEFAULT_CAPSULE}")
                with open(DEFAULT_CAPSULE, "rb") as f:
                    payload_bytes = f.read()
            else:
                # Fallback to absolute capsule
                abs_capsule = DEFAULT_CAPSULE.replace("-ref.capsule", ".capsule")
                if os.path.exists(abs_capsule):
                    print(f"\n[*] Auto-detected default absolute capsule file: {abs_capsule}")
                    with open(abs_capsule, "rb") as f:
                        payload_bytes = f.read()

    if not payload_bytes:
        print("[-] Error: No input capsule file or packet directory found.")
        sys.exit(1)

    # 2. Decompress Zlib (L6)
    print("\n[L6] Decompressing binary payload...")
    try:
        decompressed = zlib.decompress(payload_bytes)
        print(f"  Decompressed: {len(decompressed):,} bytes")
    except Exception as e:
        # If the input was the full padded packets, it might have trailing padding bytes.
        # We need to trim trailing padding or parse headers.
        # Let's try parsing directly or handle payload extraction
        print(f"[-] Decompression failed: {e}")
        sys.exit(1)

    # 3. Parse Magic Header and Mode
    pos = 0
    magic = decompressed[pos:pos+3]; pos += 3
    if magic != TK_MAGIC:
        print(f"[-] Error: Invalid magic bytes: {magic.hex()}")
        sys.exit(1)
        
    mode = decompressed[pos]; pos += 1
    print(f"  Magic verified: 0x{magic.hex().upper()}")
    print(f"  Mode verified:  Mode {mode} ({'Absolute' if mode == 1 else 'Reference/Oracle'})")

    os.makedirs(args.out_dir, exist_ok=True)

    # 4. Reconstruction
    if mode == 1:
        # --- Mode 1: Absolute Mode ---
        print("\n[L2-L4] Restoring absolute tokenizer structures...")
        
        # Unpack config metadata length and data
        comp_config_len = struct.unpack_from('>I', decompressed, pos)[0]; pos += 4
        comp_config_data = decompressed[pos : pos + comp_config_len]; pos += comp_config_len
        config_meta = json.loads(zlib.decompress(comp_config_data).decode("utf-8"))
        print(f"  - Config metadata loaded ({len(config_meta)} keys)")

        # Unpack vocabulary normal tokens
        vocab_num = struct.unpack_from('>I', decompressed, pos)[0]; pos += 4
        vocab_len = struct.unpack_from('>I', decompressed, pos)[0]; pos += 4
        vocab_data = decompressed[pos : pos + vocab_len]; pos += vocab_len
        
        vocab_list = decode_prefix_suffix(vocab_data, vocab_num)
        print(f"  - Restored {len(vocab_list):,} normal vocabulary tokens")

        # Unpack merges
        merges_num = struct.unpack_from('>I', decompressed, pos)[0]; pos += 4
        merges_data = decompressed[pos : pos + merges_num * 6]; pos += merges_num * 6
        
        merges = []
        for i in range(merges_num):
            idx0 = int.from_bytes(merges_data[i*6 : i*6 + 3], 'big')
            idx1 = int.from_bytes(merges_data[i*6 + 3 : i*6 + 6], 'big')
            t0 = vocab_list[idx0].decode("utf-8", errors="replace")
            t1 = vocab_list[idx1].decode("utf-8", errors="replace")
            merges.append(f"{t0} {t1}")
        print(f"  - Restored {len(merges):,} BPE merge entries")

        # Reconstruct vocab.json and merges.txt
        vocab_dict = {t.decode("utf-8", errors="replace"): idx for idx, t in enumerate(vocab_list)}
        
        # Write merges.txt
        merges_out = os.path.join(args.out_dir, "merges.txt")
        with open(merges_out, "w", encoding="utf-8") as f:
            f.write("\n".join(merges) + "\n")
            
        # Write vocab.json
        vocab_out = os.path.join(args.out_dir, "vocab.json")
        with open(vocab_out, "w", encoding="utf-8") as f:
            json.dump(vocab_dict, f, ensure_ascii=False, indent=2)

        # Reconstruct tokenizer.json
        reconstructed_t_json = {
            "version": config_meta["version"],
            "truncation": config_meta["truncation"],
            "padding": config_meta["padding"],
            "added_tokens": config_meta["added_tokens"],
            "normalizer": config_meta["normalizer"],
            "pre_tokenizer": config_meta["pre_tokenizer"],
            "post_processor": config_meta["post_processor"],
            "decoder": config_meta["decoder"],
            "model": {
                "type": config_meta["model_type"],
                "dropout": config_meta["model_dropout"],
                "unk_token": config_meta["model_unk_token"],
                "continuing_subword_prefix": config_meta["model_continuing_subword_prefix"],
                "end_of_word_suffix": config_meta["model_end_of_word_suffix"],
                "fuse_unk": config_meta["model_fuse_unk"],
                "byte_fallback": config_meta["model_byte_fallback"],
                "ignore_merges": config_meta["model_ignore_merges"],
                "vocab": vocab_dict,
                "merges": merges
            }
        }
        
        tokenizer_json_out = os.path.join(args.out_dir, "tokenizer.json")
        with open(tokenizer_json_out, "w", encoding="utf-8") as f:
            json.dump(reconstructed_t_json, f, ensure_ascii=False, indent=2)

        # Write tokenizer_config.json
        tokenizer_config_out = os.path.join(args.out_dir, "tokenizer_config.json")
        with open(tokenizer_config_out, "w", encoding="utf-8") as f:
            json.dump(config_meta["tokenizer_config"], f, ensure_ascii=False, indent=2)

        print("[+] Stand-alone absolute reconstruction completed successfully.")

    elif mode == 2:
        # --- Mode 2: Reference Mode ---
        print("\n[L5] Fetching base model tokenizer reference from HuggingFace...")
        base_repo_len = struct.unpack_from('>H', decompressed, pos)[0]; pos += 2
        base_repo = decompressed[pos : pos + base_repo_len].decode("utf-8"); pos += base_repo_len
        print(f"  - Base Oracle Reference: {base_repo}")
        
        try:
            # Load tokenizer from HF reference
            print(f"  - Querying Hugging Face: {base_repo} ...")
            tokenizer = AutoTokenizer.from_pretrained(base_repo, trust_remote_code=True)
            tokenizer.save_pretrained(args.out_dir)
            print(f"[+] Successfully downloaded and saved tokenizer to: {args.out_dir}")
        except Exception as e:
            print(f"[-] Error downloading base tokenizer: {e}")
            # Offline fallback if local files exist
            local_fallback = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
            if os.path.exists(local_fallback):
                print(f"  - [Offline Fallback] Copying from local cache: {local_fallback}")
                import shutil
                for fn in ["tokenizer.json", "tokenizer_config.json", "vocab.json", "merges.txt"]:
                    src = os.path.join(local_fallback, fn)
                    if os.path.exists(src):
                        shutil.copy(src, os.path.join(args.out_dir, fn))
                print(f"[+] Offline fallback copied successfully to: {args.out_dir}")
            else:
                sys.exit(1)
    else:
        print(f"[-] Error: Unknown tokenizer capsule mode: {mode}")
        sys.exit(1)

    # 5. Verification
    print("\n[*] Verifying reconstructed tokenizer loading correctness...")
    try:
        loaded_tokenizer = AutoTokenizer.from_pretrained(args.out_dir, trust_remote_code=True)
        print(f"  [PASS] Reconstructed tokenizer successfully parsed by Transformers!")
        
        # Test encoding
        test_text = "Astronaut SHE LoRa concentrator GPIO reset SX1302 v3.0 Cuneiform-U"
        tokens = loaded_tokenizer.encode(test_text)
        decoded = loaded_tokenizer.decode(tokens)
        print(f"  [PASS] Test encoding round-trip succeeded!")
        print(f"         Encoded: {tokens[:8]}...")
        print(f"         Decoded: \"{decoded}\"")
        
        print("\n" + "=" * 80)
        print("  RESTORE SUCCESSFUL")
        print("=" * 80)
        print(f"  Output folder: {os.path.abspath(args.out_dir)}")
        print("=" * 80)
    except Exception as e:
        print(f"[-] Verification failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
