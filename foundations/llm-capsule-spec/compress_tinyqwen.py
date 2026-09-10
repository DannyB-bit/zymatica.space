# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
import sys
import struct
import json
import hashlib
import numpy as np
import torch
from peft import PeftModel
from transformers import AutoModelForCausalLM

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

BASE_MODEL = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
ADAPTER_PATH = "j:/Language-U/TinyQwen"
OUT_DIR = "j:/Language-U/packets_tinyqwen"
MAGIC = bytes([0xA7, 0x07, 0x11])
PKT_SIZE = 256
WATERMARK = b'zymatica.space  '

LAYER_NAMES = [
    "model.layers.3.self_attn.q_proj.weight",
    "model.layers.3.self_attn.k_proj.weight",
    "model.layers.3.self_attn.v_proj.weight",
    "model.layers.3.self_attn.o_proj.weight",
    "model.layers.3.mlp.gate_proj.weight",
    "model.layers.3.mlp.up_proj.weight",
    "model.layers.3.mlp.down_proj.weight",
]

# We map module names in Peft state dict to base model param names
LORA_MODULES = [
    "base_model.model.model.layers.3.self_attn.q_proj",
    "base_model.model.model.layers.3.self_attn.k_proj",
    "base_model.model.model.layers.3.self_attn.v_proj",
    "base_model.model.model.layers.3.self_attn.o_proj",
    "base_model.model.model.layers.3.mlp.gate_proj",
    "base_model.model.model.layers.3.mlp.up_proj",
    "base_model.model.model.layers.3.mlp.down_proj",
]

QA_PAIRS = [
    ("What GPIO pin is the SX1302 reset line on Raspberry Pi 4?",
     "GPIO pin 25. The SX1302 reset is connected to GPIO 25 on gpiochip0."),
    ("What is the exact command to reset the LoRa concentrator with gpioset?",
     "gpioset -c gpiochip0 --toggle 100ms,100ms,0 25=0"),
    ("What script handles the SX1302 hardware reset?",
     "reset_lgw.sh handles the SX1302 hardware reset sequence."),
    ("On Raspberry Pi 5, which gpiochip and pin is the SX1302 reset mapped to?",
     "GPIO 17 on gpiochip4 on Raspberry Pi 5."),
    ("What frequency does the Astronaut SHE Handshake Protocol use?",
     "903.0 MHz. The Astronaut SHE Handshake Protocol operates at 903.0 MHz."),
    ("What Spreading Factor is used for the Astronaut SHE handshake?",
     "SF7. The Astronaut SHE handshake uses Spreading Factor 7 (SF7)."),
    ("What is the transmit power for the Astronaut SHE RAK Miner beacon?",
     "14 dBm. The Astronaut SHE RAK Miner beacon transmits at 14 dBm."),
    ("What does --pwid 15 represent in test_loragw_hal_tx?",
     "14 dBm power calibration index. --pwid 15 sets gain to 14 dBm."),
    ("What is the full test_loragw_hal_tx command for the Astronaut SHE handshake?",
     "./test_loragw_hal_tx -r 1250 -f 903.0 -m LORA -s 7 -b 125 -n 1 --pwid 15 -p 14 -z 32"),
    ("What is the payload size for the Astronaut SHE handshake beacon?",
     "32 bytes. The Astronaut SHE handshake beacon payload is 32 bytes."),
    ("How many dimensions does the Cuneiform-U v3.0 semantic hypercube have?",
     "6 dimensions. The Cuneiform-U v3.0 semantic hypercube is 6-dimensional."),
    ("What are the 6 axes of Cuneiform-U v3.0?",
     "DOMAIN, SUBDOMAIN, OPERATION, MODALITY, DEPTH, POLARITY"),
    ("What is the Classifier Radical R_C in Cuneiform-U v3.0?",
     "R_C packs DOMAIN in upper 4 bits and SUBDOMAIN in lower 4 bits."),
    ("What are the radical coordinates of the ACK glyph (0x807E)?",
     "R_C=0x00, R_F=0x7E, R_A=0x0B for the ACK glyph 0x807E."),
    ("What is the Shannon Orthogonality equation in Language U?",
     "H(text) = H(meaning) + H(syntax | meaning)"),
    ("What does LLD-AC stand for?",
     "LLM-Logits-Driven Range Coding. LLD-AC uses LLM probability distributions."),
    ("What is a collapse signal in LLD-AC range coding?",
     "When probability approaches 1.0, encoding cost approaches 0 bits — a collapse signal."),
    ("What frequency scale does the LLD-AC range coder use?",
     "1,000,000. The LLD-AC range coder scales frequencies to 1,000,000 integer units."),
]

def eigenspace_compress(W_delta: np.ndarray, W_base: np.ndarray, K: int = 16) -> bytes:
    U_b, S_b, Vh_b = np.linalg.svd(W_base.astype(np.float64), full_matrices=False)
    R = min(K, len(S_b))
    delta_s = np.array([
        float(U_b[:, i].T @ W_delta.astype(np.float64) @ Vh_b[i, :])
        for i in range(R)
    ])
    scale = float(np.abs(delta_s).max()) / 127.0 + 1e-9
    q = np.round(delta_s[:R] / scale).clip(-127, 127).astype(np.int8)
    return bytes([R]) + struct.pack('>e', scale) + bytes(q)

def gradient_atom_compress(W_delta: np.ndarray, W_base: np.ndarray, K: int = 8) -> bytes:
    U_b, S_b, Vh_b = np.linalg.svd(W_base.astype(np.float64), full_matrices=False)
    R = min(K, len(S_b))
    delta_s = np.array([
        float(U_b[:, i].T @ W_delta.astype(np.float64) @ Vh_b[i, :])
        for i in range(R)
    ])
    base_scale = float(np.abs(delta_s).max()) + 1e-9

    nibbles = []
    for i in range(R):
        s    = delta_s[i] / base_scale
        sign = 1 if s >= 0 else 0
        mag  = min(3, int(abs(s) * 4))
        nibbles.append((sign << 2) | mag)

    packed = bytearray()
    for i in range(0, len(nibbles), 2):
        lo = nibbles[i]
        hi = nibbles[i+1] if i+1 < len(nibbles) else 0
        packed.append((hi << 4) | lo)

    return bytes([R]) + struct.pack('>e', base_scale) + bytes(packed)

def main():
    level = 5
    if len(sys.argv) > 1:
        try:
            level = int(sys.argv[1])
        except ValueError:
            pass
            
    print("=" * 72)
    print(f"  TINYQWEN 1-PAUP COMPRESSOR — Level {level}")
    print("  Watermark: ip zymatica.space")
    print("=" * 72)

    # 1. Load models
    print("Loading base model...")
    base_model = AutoModelForCausalLM.from_pretrained(BASE_MODEL, torch_dtype=torch.float32)
    base_p = dict(base_model.named_parameters())
    
    print("Loading LoRA adapter model...")
    peft_model = PeftModel.from_pretrained(base_model, ADAPTER_PATH)
    peft_state = peft_model.state_dict()

    print("\nCompressing weight updates using SVD...")
    weight_blobs = []
    total_enc = 0

    for idx, (name, base_name) in enumerate(zip(LORA_MODULES, LAYER_NAMES), 1):
        # Extract LoRA A and B weights
        a_key = f"{name}.lora_A.default.weight"
        b_key = f"{name}.lora_B.default.weight"
        
        A = peft_state[a_key].cpu().numpy() # Shape: [r, in_dim]
        B = peft_state[b_key].cpu().numpy() # Shape: [out_dim, r]
        
        # Qwen LoRA configs: r=8, alpha=16
        r = 8
        alpha = 16
        W_d = (B @ A) * (alpha / r)
        
        W_b = base_p[base_name].data.to(torch.float32).cpu().numpy()
        m, n = W_b.shape

        if level == 5:
            # We want to fit exactly within 224 bytes.
            # 7 layers * 27 bytes = 189 bytes. 24 coefficients works perfectly.
            K5 = min(24, min(m, n))
            blob = eigenspace_compress(W_d, W_b, K=K5)
        else: # level 6
            # 7 layers * 7 bytes = 49 bytes.
            K6 = min(8, min(m, n))
            blob = gradient_atom_compress(W_d, W_b, K=K6)

        weight_blobs.append(blob)
        total_enc += len(blob)
        print(f"  [{idx:02X}] {base_name.split('.')[-2]} -> {len(blob)} bytes")

    weight_data = b''.join(weight_blobs)
    print(f"Total weight payload: {total_enc} bytes")

    # 18-bit question selection mask representing all 18 canonical questions selected
    q_mask = 0b111111111111111111
    q_mask_bytes = struct.pack('>I', q_mask)[1:] # 3 bytes

    # 32-byte header
    lr_f16 = struct.pack('>e', 2e-4)
    lu4_quad = bytes([0xE0, 0x09, 0x9F, 0x9A])
    header = (
        MAGIC +                 # 3 bytes
        bytes([level]) +        # 1 byte
        lr_f16 +                # 2 bytes
        struct.pack('>H', 150) + # 2 bytes (steps)
        struct.pack('>I', 0xA11E4) + # 4 bytes (seed)
        bytes([1, 4, 0xFF]) +   # 3 bytes (optim, batch, layer_flags)
        struct.pack('>H', 10) +  # 2 bytes (warmup)
        bytes([len(QA_PAIRS)]) + # 1 byte
        lu4_quad +              # 4 bytes
        q_mask_bytes +          # 3 bytes
        bytes([len(LAYER_NAMES)]) + # 1 byte
        struct.pack('>H', len(weight_data)) # 2 bytes
    )
    header += b'\x00' * (32 - len(header))

    full_packet_data = header + weight_data
    if len(full_packet_data) > 253:
        print(f"⚠️ Error: Payload size {len(full_packet_data)} bytes exceeds single packet capacity!")
        sys.exit(1)

    print(f"\n[+] Fits in a single packet! Payload size: {len(full_packet_data)} bytes")

    spare = 253 - len(full_packet_data)
    padded_payload = full_packet_data + (WATERMARK * (spare // len(WATERMARK) + 1))[:spare]

    # [SYNC=0xBB] [PKT_IDX=0] [TOTAL_PKTS=1] [253 bytes payload]
    packet = bytes([0xBB, 0, 1]) + padded_payload
    
    os.makedirs(OUT_DIR, exist_ok=True)
    out_path = os.path.join(OUT_DIR, "packet_1paup.bin")
    with open(out_path, "wb") as f:
        f.write(packet)
    print(f"Packet successfully written to: {out_path}")

    # Compress the packet using zlib to enable faster RAK miner transmission
    import zlib
    compressed_packet = zlib.compress(packet)
    compressed_out_path = os.path.join(OUT_DIR, "packet_1paup.bin.zlib")
    with open(compressed_out_path, "wb") as f:
        f.write(compressed_packet)
    print(f"Zlib-compressed packet successfully written to: {compressed_out_path} ({len(compressed_packet)} bytes)")

    # Save manifest
    manifest = {
        "protocol": "1-PAUP v1.0",
        "watermark": "ip zymatica.space",
        "level": level,
        "payload_bytes": len(full_packet_data),
        "header_bytes": len(header),
        "weight_bytes": len(weight_data),
        "question_mask": bin(q_mask),
        "sha256": hashlib.sha256(packet).hexdigest(),
    }
    manifest_path = os.path.join(OUT_DIR, "manifest_1paup.json")
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)
    print(f"Manifest written to: {manifest_path}")

if __name__ == '__main__':
    main()
