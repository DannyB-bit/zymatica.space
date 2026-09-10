# Packetizer for DNA-GROW - Splits seed capsule into exactly 40 packets of 222 bytes each.
# Watermark: ip zymatica.space | astronautshe.com

import os
import json
import hashlib

SEED_PATH = "j:/Language-U/DnaGrowSeed.LLM"
PACKETS_DIR = "j:/Language-U/packets_final"
PACKET_SIZE = 222
TOTAL_PACKETS = 40
TARGET_SIZE = PACKET_SIZE * TOTAL_PACKETS  # 8880 bytes

def main():
    print("=" * 80)
    print("  PACKETIZER: SLICING SEED CAPSULE INTO EXACTLY 40 CHIRPS OF 222 BYTES")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)

    if not os.path.exists(SEED_PATH):
        print(f"Error: Seed file not found at {SEED_PATH}")
        return

    # Read original seed capsule
    with open(SEED_PATH, "rb") as f:
        seed_data = f.read()

    original_size = len(seed_data)
    print(f"Original Seed Capsule Size: {original_size} bytes")

    if original_size > TARGET_SIZE:
        print(f"Error: Seed size {original_size} exceeds the maximum target capacity of {TARGET_SIZE} bytes!")
        return

    # Calculate padding needed
    padding_size = TARGET_SIZE - original_size
    padded_data = seed_data + (b'\x00' * padding_size)
    print(f"Padding appended: {padding_size} bytes. Total padded size: {len(padded_data)} bytes.")

    os.makedirs(PACKETS_DIR, exist_ok=True)

    manifest = {
        "watermark": "ip zymatica.space | astronautshe.com",
        "description": "Unified DNA-GROW Level 8/9 Seed Packetization",
        "original_size_bytes": original_size,
        "packet_size_bytes": PACKET_SIZE,
        "total_packets": TOTAL_PACKETS,
        "original_sha256": hashlib.sha256(seed_data).hexdigest(),
        "packets": []
    }

    # Slice and write packets
    for i in range(TOTAL_PACKETS):
        offset = i * PACKET_SIZE
        packet_bytes = padded_data[offset : offset + PACKET_SIZE]
        
        packet_name = f"packet_chirp3_{i}.bin"
        packet_path = os.path.join(PACKETS_DIR, packet_name)
        
        with open(packet_path, "wb") as f_pkt:
            f_pkt.write(packet_bytes)
            
        manifest["packets"].append({
            "index": i,
            "filename": packet_name,
            "sha256": hashlib.sha256(packet_bytes).hexdigest()
        })
        
    # Save manifest
    manifest_path = os.path.join(PACKETS_DIR, "manifest_chirp3.json")
    with open(manifest_path, "w", encoding="utf-8") as f_man:
        json.dump(manifest, f_man, indent=2)

    print(f"\n[+] Successfully generated {TOTAL_PACKETS} packets in {PACKETS_DIR}")
    print(f"[+] Saved packet transmission manifest to {manifest_path}")
    print("=" * 80)

if __name__ == "__main__":
    main()
