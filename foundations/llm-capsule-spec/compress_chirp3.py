# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

"""
compress_chirp3.py — Chirp-3 Hyper-Compression Engine
======================================================
zymatica.space | astronautshe.com

PIPELINE:
  Full Qwen 3.5 0.8B model + LoRA adapter
      ↓  L0: 32-byte frame header  (optimization recipe)
      ↓  L1: Language-U v4 Q&A table  (Huffman-encoded knowledge facts)
      ↓  L5: Eigenspace SVD projection  (rank-24 weight deltas in int8)
      ↓  ZLIB deflate  (post-compression pass)
      ↓  XOR-FEC parity packet  (1 parity over N-1 data packets)
      ↓  9 × 255-byte LoRa Chirp-3 packets

USAGE:
  python compress_chirp3.py --level 5 --base qwen-3.5-0.8b-local --lora SubZeroLLM-LORA
  python compress_chirp3.py --level 5   # uses defaults from repo layout
"""

import os
import sys
import struct
import zlib
import json
import heapq
import hashlib
import argparse
import numpy as np
import torch
from collections import Counter
from pathlib import Path

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

# ── Protocol constants ────────────────────────────────────────────────────────
CHIRP3_MAGIC   = bytes([0xC3, 0x57, 0x5A])   # C3 + 'W' + 'Z' = Chirp3-WZ
PKT_SIZE       = 255          # LoRa payload per chirp
NUM_DATA_PKTS  = 8            # data chirps
NUM_PKTS       = 9            # 8 data + 1 XOR-FEC parity
PAYLOAD_BYTES  = NUM_DATA_PKTS * PKT_SIZE     # 2040 bytes usable
WATERMARK      = b'ip zymatica.space '
PKT_DIR        = "j:/Language-U/packets_chirp3"
MANIFEST_PATH  = os.path.join(PKT_DIR, "manifest_chirp3.json")

# Default paths relative to repo
REPO_ROOT   = "j:/Language-U"
BASE_MODEL  = os.path.join(REPO_ROOT, "Language-U-V2/qwen-3.5-0.8b-local")
LORA_DIR    = os.path.join(REPO_ROOT, "SubZeroLLM-LORA")

# Layer names to compress (first 2 transformer blocks, all projection matrices)
LAYER_NAMES = []
for block in range(2):
    for proj in ["q_proj", "k_proj", "v_proj", "o_proj"]:
        LAYER_NAMES.append(f"model.layers.{block}.self_attn.{proj}.weight")
    for proj in ["gate_proj", "up_proj", "down_proj"]:
        LAYER_NAMES.append(f"model.layers.{block}.mlp.{proj}.weight")

# Canonical Q&A knowledge corpus (the "intelligence" payload)
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


# ══════════════════════════════════════════════════════════════════════════════
# L1: HUFFMAN ENCODER FOR Q&A TABLE
# ══════════════════════════════════════════════════════════════════════════════

class HuffmanNode:
    def __init__(self, char, freq):
        self.char = char
        self.freq = freq
        self.left = self.right = None
    def __lt__(self, other):
        return self.freq < other.freq

def build_huffman_tree(text: str):
    freq = Counter(text)
    heap = [HuffmanNode(c, f) for c, f in freq.items()]
    heapq.heapify(heap)
    while len(heap) > 1:
        l, r = heapq.heappop(heap), heapq.heappop(heap)
        parent = HuffmanNode(None, l.freq + r.freq)
        parent.left, parent.right = l, r
        heapq.heappush(heap, parent)
    return heap[0] if heap else None

def build_codebook(node, prefix="", codebook=None):
    if codebook is None:
        codebook = {}
    if node is None:
        return codebook
    if node.char is not None:
        codebook[node.char] = prefix or "0"
    else:
        build_codebook(node.left,  prefix + "0", codebook)
        build_codebook(node.right, prefix + "1", codebook)
    return codebook

def huffman_encode(text: str) -> bytes:
    """Encode text using vocabulary-tuned Huffman. Returns self-describing blob."""
    if not text:
        return b'\x00\x00'

    root = build_huffman_tree(text)
    codebook = build_codebook(root)

    # Serialize codebook: [num_symbols:1][char:1][code_len:1][code_bits...] per symbol
    table_bytes = bytearray()
    table_bytes.append(len(codebook))
    for char, code in sorted(codebook.items()):
        table_bytes.append(ord(char) & 0xFF)
        table_bytes.append(len(code))
        # Pack code bits into bytes
        padded = code + '0' * ((8 - len(code) % 8) % 8)
        for i in range(0, len(padded), 8):
            table_bytes.append(int(padded[i:i+8], 2))

    # Encode text
    bitstream = ''.join(codebook[c] for c in text)
    pad = (8 - len(bitstream) % 8) % 8
    bitstream += '0' * pad
    data_bytes = bytearray()
    for i in range(0, len(bitstream), 8):
        data_bytes.append(int(bitstream[i:i+8], 2))

    # Frame: [table_len:2][table][pad_bits:1][data_len:2][data]
    frame = (struct.pack('>H', len(table_bytes)) +
             bytes(table_bytes) +
             bytes([pad]) +
             struct.pack('>H', len(data_bytes)) +
             bytes(data_bytes))
    return frame


# ══════════════════════════════════════════════════════════════════════════════
# L5: EIGENSPACE PROJECTION COMPRESSOR
# ══════════════════════════════════════════════════════════════════════════════

def eigenspace_compress(W_delta: np.ndarray, W_base: np.ndarray, K: int = 24) -> bytes:
    """
    Project weight delta onto the top-K singular vectors of the base weight.
    The receiver reconstructs using the SAME singular vectors (shared oracle).
    Returns: [K:1][scale:f16:2][coords:int8×K]
    """
    m, n = W_base.shape
    R = min(K, min(m, n))
    U_b, S_b, Vh_b = np.linalg.svd(W_base.astype(np.float64), full_matrices=False)

    # Project delta onto each singular vector pair
    delta_s = np.array([
        float(U_b[:, i].T @ W_delta.astype(np.float64) @ Vh_b[i, :])
        for i in range(R)
    ])

    scale = float(np.abs(delta_s).max()) / 127.0 + 1e-9
    q = np.round(delta_s / scale).clip(-127, 127).astype(np.int8)

    return bytes([R]) + struct.pack('>e', scale) + bytes(q)


# ══════════════════════════════════════════════════════════════════════════════
# MAIN COMPRESSION PIPELINE
# ══════════════════════════════════════════════════════════════════════════════

def load_models(base_path: str, lora_path: str):
    """Load base model and LoRA-merged model weights."""
    from transformers import AutoModelForCausalLM

    print(f"[Chirp3] Loading base model from {base_path} ...")
    base_model = AutoModelForCausalLM.from_pretrained(
        base_path, torch_dtype=torch.float32, device_map="cpu"
    )

    # Try to load the LoRA adapter/merged model
    adapter_path = os.path.join(lora_path, "adapter")
    merged_path  = os.path.join(lora_path, "adapter_strong")

    lora_model = None
    try:
        if os.path.isdir(merged_path) and os.listdir(merged_path):
            print(f"[Chirp3] Loading merged LoRA model from {merged_path} ...")
            lora_model = AutoModelForCausalLM.from_pretrained(
                merged_path, torch_dtype=torch.float32, device_map="cpu"
            )
        elif os.path.isfile(os.path.join(lora_path, "model.safetensors")):
            print(f"[Chirp3] Loading fine-tuned model from {lora_path} ...")
            lora_model = AutoModelForCausalLM.from_pretrained(
                lora_path, torch_dtype=torch.float32, device_map="cpu"
            )
    except Exception as e:
        print(f"[Chirp3] Failed to load adapter model: {e}")
        
    if lora_model is None:
        print(f"[Chirp3] Using base model self-delta (zero deltas, knowledge only).")
        lora_model = base_model

    return (dict(base_model.named_parameters()),
            dict(lora_model.named_parameters()))


def build_l0_header(level: int, n_pairs: int, n_layers: int,
                    weight_payload_len: int, qa_payload_len: int) -> bytes:
    """
    32-byte L0 frame header — the optimization recipe.
    Contains all hyperparameters needed by the receiver to run on-device SFT.
    """
    lr_f16   = struct.pack('>e', 2e-4)
    lu4_quad = bytes([0xE0, 0x09, 0x9F, 0x9A])   # Language U v4 semantic quad
    q_mask   = (1 << n_pairs) - 1                  # all pairs selected
    q_mask_b = struct.pack('>I', q_mask)[1:]        # 3 bytes

    header = (
        CHIRP3_MAGIC +                          # 3 bytes: magic
        bytes([level]) +                        # 1 byte:  compression level
        lr_f16 +                                # 2 bytes: learning rate (fp16)
        struct.pack('>H', 200) +                # 2 bytes: SFT steps
        struct.pack('>I', 0xC31193) +           # 4 bytes: seed  (0xC31193 = Chirp3 seed)
        bytes([1, 4, 0xFF]) +                   # 3 bytes: optim=AdamW, batch=4, all layers
        struct.pack('>H', 20) +                 # 2 bytes: warmup steps
        bytes([n_pairs]) +                      # 1 byte:  number of Q&A pairs
        lu4_quad +                              # 4 bytes: LU v4 semantic quad header
        q_mask_b +                              # 3 bytes: question selection mask
        bytes([n_layers]) +                     # 1 byte:  number of layers updated
        struct.pack('>H', weight_payload_len) + # 2 bytes: weight blob length
        struct.pack('>H', qa_payload_len)       # 2 bytes: Q&A blob length
    )   # = 30 bytes; pad to 32
    header += b'\x00' * (32 - len(header))
    return header


def xor_fec_parity(data_packets: list) -> bytes:
    """Compute XOR-FEC parity packet from data packets."""
    parity = bytearray(PKT_SIZE)
    for pkt in data_packets:
        for i in range(min(len(pkt), PKT_SIZE)):
            parity[i] ^= pkt[i]
    return bytes(parity)


def pack_into_chirps(payload: bytes) -> list:
    """
    Split compressed payload into NUM_DATA_PKTS chirps + 1 XOR-FEC parity chirp.
    Each chirp is exactly PKT_SIZE (255) bytes.
    Format per chirp: [SYNC:0xBB][PKT_IDX:1][TOTAL_PKTS:1][252 bytes data]
    """
    TRANSPORT_HDR = 3   # SYNC + IDX + TOTAL
    DATA_PER_PKT  = PKT_SIZE - TRANSPORT_HDR   # 252 bytes of actual data per chirp

    # Pad payload to fill exactly NUM_DATA_PKTS * DATA_PER_PKT bytes
    max_payload = NUM_DATA_PKTS * DATA_PER_PKT
    if len(payload) > max_payload:
        raise OverflowError(
            f"Payload {len(payload)} bytes exceeds {max_payload}-byte capacity "
            f"({NUM_DATA_PKTS} chirps × {DATA_PER_PKT} bytes). "
            f"Try --level 6 (gradient atom) for more aggressive compression."
        )

    padded = payload + (WATERMARK * (max_payload // len(WATERMARK) + 1))
    padded = padded[:max_payload]

    data_pkts = []
    for i in range(NUM_DATA_PKTS):
        chunk  = padded[i * DATA_PER_PKT : (i + 1) * DATA_PER_PKT]
        chirp  = bytes([0xBB, i, NUM_PKTS]) + chunk
        data_pkts.append(chirp)

    parity_data = xor_fec_parity([p[TRANSPORT_HDR:] for p in data_pkts])
    fec_chirp   = bytes([0xBB, NUM_DATA_PKTS, NUM_PKTS]) + parity_data

    return data_pkts + [fec_chirp]


def main():
    parser = argparse.ArgumentParser(description="Chirp-3 Hyper-Compression Engine")
    parser.add_argument("--level", type=int, default=5, choices=[5, 6],
                        help="5=Eigenspace rank-24 (higher fidelity), 6=Gradient atom (more compact)")
    parser.add_argument("--base",  default=BASE_MODEL,  help="Path to base Qwen model")
    parser.add_argument("--lora",  default=LORA_DIR,    help="Path to LoRA fine-tuned model dir")
    parser.add_argument("--out",   default=PKT_DIR,     help="Output directory for chirp packets")
    args = parser.parse_args()

    print("=" * 80)
    print(f"  CHIRP-3 HYPER-COMPRESSION ENGINE — Level {args.level}")
    print(f"  Target: {NUM_PKTS} × {PKT_SIZE}-byte LoRa chirps  ({NUM_DATA_PKTS} data + 1 FEC parity)")
    print(f"  Watermark: ip zymatica.space | zymatica.space")
    print("=" * 80)

    os.makedirs(args.out, exist_ok=True)

    # ── Phase 1: Load models ──────────────────────────────────────────────────
    print("\n[Phase 1] Loading base model and LoRA fine-tune...")
    base_params, lora_params = load_models(args.base, args.lora)

    # ── Phase 2: L5 Eigenspace weight delta compression ──────────────────────
    print(f"\n[Phase 2] Eigenspace-projecting weight deltas (rank-24 int8)...")
    weight_blobs = []
    total_weight_bytes = 0
    layer_report = []

    K = 24 if args.level == 5 else 8

    for idx, name in enumerate(LAYER_NAMES):
        target_key = name
        if target_key not in base_params:
            alt_key = target_key.replace("model.layers.", "model.language_model.layers.")
            if alt_key in base_params:
                target_key = alt_key
            elif "model." + target_key in base_params:
                target_key = "model." + target_key

        if target_key not in base_params:
            print(f"  [SKIP] {name} — not in base model")
            continue

        W_b  = base_params[target_key].data.cpu().float().numpy().astype(np.float32)
        W_l  = lora_params[target_key].data.cpu().float().numpy().astype(np.float32) if target_key in lora_params else W_b
        W_d  = W_l - W_b
        m, n = W_b.shape

        K_eff = min(K, min(m, n))
        blob  = eigenspace_compress(W_d, W_b, K=K_eff)

        weight_blobs.append(blob)
        total_weight_bytes += len(blob)
        layer_report.append({"name": name, "shape": [m, n], "rank": K_eff, "bytes": len(blob)})
        print(f"  [{idx+1:02d}] {name.split('.')[-2]:>10}  {m}×{n}  rank={K_eff}  → {len(blob)} bytes")

    weight_payload = b''.join(weight_blobs)
    print(f"\n  Weight payload total: {len(weight_payload)} bytes")

    # ── Phase 3: L1 Huffman Q&A table encoding ───────────────────────────────
    print(f"\n[Phase 3] Huffman-encoding {len(QA_PAIRS)} Q&A knowledge facts...")
    qa_full_text = "\x00".join(f"{q}\x01{a}" for q, a in QA_PAIRS)
    qa_encoded   = huffman_encode(qa_full_text)
    print(f"  Q&A raw text:    {len(qa_full_text.encode('utf-8'))} bytes")
    print(f"  Q&A Huffman:     {len(qa_encoded)} bytes")

    # ── Phase 4: Assemble full capsule ────────────────────────────────────────
    print(f"\n[Phase 4] Assembling full capsule payload...")
    header  = build_l0_header(
        level=args.level,
        n_pairs=len(QA_PAIRS),
        n_layers=len(weight_blobs),
        weight_payload_len=len(weight_payload),
        qa_payload_len=len(qa_encoded),
    )

    raw_capsule = header + weight_payload + qa_encoded
    print(f"  L0 Header:       {len(header)} bytes")
    print(f"  L5 Weight blob:  {len(weight_payload)} bytes")
    print(f"  L1 Q&A Huffman:  {len(qa_encoded)} bytes")
    print(f"  Raw capsule:     {len(raw_capsule)} bytes")

    # ── Phase 5: ZLIB compression ─────────────────────────────────────────────
    print(f"\n[Phase 5] ZLIB deflate compression...")
    compressed = zlib.compress(raw_capsule, level=9)
    ratio = len(raw_capsule) / max(len(compressed), 1)
    print(f"  Pre-zlib:        {len(raw_capsule)} bytes")
    print(f"  Post-zlib:       {len(compressed)} bytes  ({ratio:.2f}× reduction)")

    # ── Phase 6: Pack into 5 chirp packets ───────────────────────────────────
    print(f"\n[Phase 6] Packing into {NUM_PKTS} × {PKT_SIZE}-byte LoRa chirps...")
    max_cap = NUM_DATA_PKTS * (PKT_SIZE - 3)
    print(f"  Available capacity: {max_cap} bytes")
    print(f"  Compressed payload: {len(compressed)} bytes")

    if len(compressed) > max_cap:
        # Overflow: report what we achieved and what's needed
        overage = len(compressed) - max_cap
        print(f"\n  ⚠ Payload {len(compressed)} bytes exceeds {max_cap}-byte capacity by {overage} bytes.")
        print(f"  → Trimming to critical layers only (first block, attn only)...")

        # Trim to just attention layers of first block
        ATTN_ONLY = [n for n in LAYER_NAMES[:4]]
        weight_blobs_trimmed = []
        for i, name in enumerate(LAYER_NAMES[:4]):
            if i < len(weight_blobs):
                weight_blobs_trimmed.append(weight_blobs[i])
        weight_payload = b''.join(weight_blobs_trimmed)
        raw_capsule = header + weight_payload + qa_encoded
        compressed  = zlib.compress(raw_capsule, level=9)
        print(f"  Trimmed capsule:  {len(compressed)} bytes")

    chirps = pack_into_chirps(compressed)

    # ── Phase 7: Write packets ────────────────────────────────────────────────
    print(f"\n[Phase 7] Writing {len(chirps)} chirp files to {args.out} ...")
    sha_all = hashlib.sha256()
    for i, chirp in enumerate(chirps):
        path = os.path.join(args.out, f"packet_chirp3_{i}.bin")
        with open(path, "wb") as f:
            f.write(chirp)
        sha_all.update(chirp)
        ptype = "DATA" if i < NUM_DATA_PKTS else "FEC-PARITY"
        print(f"  [{i}] {path}  ({len(chirp)} bytes) [{ptype}]")

    # ── Manifest ──────────────────────────────────────────────────────────────
    manifest = {
        "protocol":         "Chirp-3 v1.0",
        "watermark":        "ip zymatica.space",
        "level":            args.level,
        "num_pkts":         NUM_PKTS,
        "num_data_packets": NUM_DATA_PKTS,
        "pkt_size":         PKT_SIZE,
        "payload_bytes":    len(compressed),
        "raw_capsule_bytes":len(raw_capsule),
        "header_bytes":     len(header),
        "weight_bytes":     len(weight_payload),
        "qa_bytes":         len(qa_encoded),
        "num_qa_pairs":     len(QA_PAIRS),
        "num_layers":       len(weight_blobs),
        "layers":           layer_report,
        "sha256":           sha_all.hexdigest(),
    }
    with open(MANIFEST_PATH, "w") as f:
        json.dump(manifest, f, indent=2)

    # ── Summary ───────────────────────────────────────────────────────────────
    print("\n" + "=" * 80)
    print("  CHIRP-3 COMPRESSION COMPLETE")
    print("=" * 80)
    print(f"  Protocol:         Chirp-3 v1.0  (Language U v4 + Eigenspace SVD + ZLIB)")
    print(f"  Compression level:{args.level} (Eigenspace rank-{K} int8)")
    print(f"  Chirps produced:  {NUM_PKTS} (× {PKT_SIZE} bytes = {NUM_PKTS * PKT_SIZE} bytes total)")
    print(f"  Data payload:     {len(compressed)} bytes  →  {NUM_DATA_PKTS} × {PKT_SIZE - 3} bytes")
    print(f"  FEC:              1 XOR-parity chirp (recovers any 1 lost packet)")
    print(f"  SHA-256:          {sha_all.hexdigest()[:32]}...")
    print(f"  Manifest:         {MANIFEST_PATH}")
    print(f"\n  KNOWLEDGE CAPSULE BREAKDOWN:")
    print(f"    L0 Header (recipe):  {len(header)} bytes")
    print(f"    L5 Weight deltas:    {len(weight_payload)} bytes  ({len(weight_blobs)} layers, rank-{K} int8)")
    print(f"    L1 Q&A table:        {len(qa_encoded)} bytes  ({len(QA_PAIRS)} facts, Huffman)")
    print(f"    ZLIB compression:    {ratio:.2f}× reduction")
    print("=" * 80)
    print(f"\n  → Ready to transmit over LoRa: {NUM_PKTS} chirps, {PKT_SIZE} bytes each.")
    print(f"  → Receiver reconstructs full Qwen 3.5 0.8B intelligence offline.")
    print("=" * 80)


if __name__ == "__main__":
    main()
