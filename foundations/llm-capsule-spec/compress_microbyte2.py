# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
import struct
import zlib
import numpy as np

CAPSULE_PATH = "j:/Language-U/qwen-3.5-0.8b-microbyte-2.capsule"
MAGIC = bytes([0xA7, 0x07, 0xC3])

def main():
    print("=" * 72)
    print("  QWEN-3.5-0.8B-MICROBYTE-2 CAPSULE GENERATOR")
    print("  Watermark: ip zymatica.space")
    print("=" * 72)

    # 1. Build L0 Header / SFT Recipe (12 bytes)
    seed = 0xA11E4
    lr = 2e-4
    steps = 150
    qualia_seed = 0b_01_11_10_00
    
    header = struct.pack('>I', seed)          # 4 bytes
    header += struct.pack('>e', lr)           # 2 bytes
    header += struct.pack('>H', steps)        # 2 bytes
    header += bytes([qualia_seed])             # 1 byte
    
    # 2. Build L1 Semantic Variables (19 bytes)
    variables = bytes([
        25,                       # Pi 4 reset pin (1 byte)
        17,                       # Pi 5 reset pin (1 byte)
        4,                        # Pi 5 gpiochip (1 byte)
        7,                        # SF (1 byte)
        14,                       # Astronaut SHE tx power (1 byte)
        15,                       # pwid index (1 byte)
        14,                       # pwid power (1 byte)
        32,                       # payload size (1 byte)
        6,                        # hypercube dimensions (1 byte)
        0x00, 0x7E, 0x0B,         # ACK coordinates (3 bytes)
        1,                        # collapse signal probability limit (1 byte: 1.0 represented as 1)
        0,                        # collapse signal encoding cost (1 byte)
    ])
    variables += struct.pack('>e', 903.0)     # Astronaut SHE freq (2 bytes)
    variables += struct.pack('>I', 1000000)   # LLD-AC frequency scale (4 bytes)

    # 3. Build L6 Gradient Atom Weight updates (mocked/quantized rank-8, 7 layers = 49 bytes)
    # Storing 7 layers * 7 bytes = 49 bytes
    # This represents the quantized singular value coefficients
    np.random.seed(seed)
    weights = np.random.randint(-3, 3, size=(7, 7), dtype=np.int8)
    
    weight_payload = bytearray()
    for layer in range(7):
        # 1 byte rank + 2 bytes scale + 4 bytes packed nibbles = 7 bytes
        r = 8
        scale = 0.35
        # Packing 8 nibbles (2-bit magnitude, 1-bit sign)
        # For mock, we pack 8 values into 4 bytes
        packed_nibbles = bytes([0x12, 0x34, 0x56, 0x78])
        weight_payload.append(r)
        weight_payload.extend(struct.pack('>e', scale))
        weight_payload.extend(packed_nibbles)

    # Combine all parts
    payload = MAGIC + header + variables + bytes(weight_payload)
    print(f"Raw binary payload: {len(payload)} bytes")

    # Apply the final train conductor: Zlib Level 9
    compressed = zlib.compress(payload, 9)
    print(f"Compressed capsule size (after Zlib): {len(compressed)} bytes")

    # Ensure it's under 255 bytes!
    assert len(compressed) <= 255, f"Capsule exceeded 255 bytes! Got {len(compressed)} bytes"

    # Save to disk
    with open(CAPSULE_PATH, "wb") as f:
        f.write(compressed)
        
    print(f"[+] Successfully wrote capsule to {CAPSULE_PATH}")
    print("=" * 72)

if __name__ == "__main__":
    main()
